use std::sync::Arc;

use async_trait::async_trait;
use forge_domain::{
    ContextMessage, Conversation, EventData, EventHandle, Role, StartPayload, TextMessage,
};
use forge_template::Element;
use tracing::warn;

use crate::{TemplateEngine, WorkspaceService};

/// Hook handler that injects skill recommendations as a droppable user message
/// at the start of each conversation turn.
///
/// When the `Start` lifecycle event fires the handler:
/// 1. Extracts the raw user query from the most recent user message in the
///    conversation context.
/// 2. Calls [`WorkspaceService::recommend_skills`] which sends the query and
///    all available skills to the remote ranking service and returns only the
///    relevant skills with their relevance scores.
/// 3. Injects a droppable `User` message listing the recommended skills wrapped
///    in `<recommended_skills>` XML so the LLM can decide which to invoke.
///
/// The injected message is marked droppable so it is automatically removed
/// during context compaction.
#[derive(Clone)]
pub struct SkillRecommendationHandler<S> {
    services: Arc<S>,
}

impl<S> SkillRecommendationHandler<S> {
    /// Creates a new skill recommendation handler.
    pub fn new(services: Arc<S>) -> Self {
        Self { services }
    }
}

#[async_trait]
impl<S: WorkspaceService> EventHandle<EventData<StartPayload>> for SkillRecommendationHandler<S> {
    async fn handle(
        &self,
        event: &EventData<StartPayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        // Extract the user query from the most-recent user message.
        // Prefer the raw_content (original event value before template rendering);
        // fall back to the rendered content string when raw_content is absent.
        let user_query = conversation
            .context
            .as_ref()
            .and_then(|c| c.messages.iter().rev().find(|m| m.has_role(Role::User)))
            .and_then(|entry| {
                entry
                    .message
                    .as_value()
                    .and_then(|v| v.as_user_prompt())
                    .map(|p| p.as_str().to_owned())
                    .or_else(|| entry.message.content().map(str::to_owned))
            });

        let Some(user_query) = user_query else {
            return Ok(());
        };

        // Call the remote ranking service to get relevant skills for this query.
        let selected = match self.services.recommend_skills(user_query.clone()).await {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    agent_id = %event.agent.id,
                    error = ?e,
                    query = %user_query,
                    "Failed to recommend skills, skipping"
                );
                return Ok(());
            }
        };

        if selected.is_empty() {
            return Ok(());
        }

        // Inject as a droppable user message so it can be removed during compaction.
        let instruction = TemplateEngine::default().render(
            "forge-skill-recommendation-message.md",
            &serde_json::json!({}),
        )?;

        let message = TextMessage::new(
            Role::User,
            Element::new("recommended_skills")
                .text(instruction)
                .append(selected.iter().map(Element::from))
                .render(),
        )
        .model(event.agent.model.clone())
        .droppable(true);

        let ctx = conversation
            .context
            .take()
            .unwrap_or_default()
            .add_message(ContextMessage::Text(message));
        conversation.context = Some(ctx);

        tracing::debug!(
            agent_id = %event.agent.id,
            user_query = %user_query,
            skills = ?selected.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            "Injected skill recommendations"
        );

        Ok(())
    }
}
