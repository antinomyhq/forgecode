use forge_domain::{Agent, Conversation, ToolDefinition};

/// Applies tunable parameters from agent to conversation context
#[derive(Debug, Clone)]
pub struct ApplyTunableParameters {
    agent: Agent,
    tool_definitions: Vec<ToolDefinition>,
}

impl ApplyTunableParameters {
    pub const fn new(agent: Agent, tool_definitions: Vec<ToolDefinition>) -> Self {
        Self { agent, tool_definitions }
    }

    pub fn apply(self, mut conversation: Conversation) -> Conversation {
        let mut ctx = conversation.context.take().unwrap_or_default();

        if let Some(temperature) = self.agent.temperature {
            ctx = ctx.temperature(temperature);
        }
        if let Some(reasoning_effort) = self.agent.reasoning_effort {
            ctx = ctx.reasoning_effort(reasoning_effort);
        }
        if let Some(service_tier) = self.agent.service_tier {
            ctx = ctx.service_tier(service_tier);
        }
        if let Some(top_p) = self.agent.top_p {
            ctx = ctx.top_p(top_p);
        }
        if let Some(top_k) = self.agent.top_k {
            ctx = ctx.top_k(top_k);
        }
        if let Some(max_tokens) = self.agent.max_tokens {
            ctx = ctx.max_tokens(max_tokens.value() as usize);
        }
        if let Some(ref reasoning) = self.agent.reasoning {
            ctx = ctx.reasoning(reasoning.clone());
        }

        conversation.context(ctx.tools(self.tool_definitions))
    }
}

#[cfg(test)]
mod tests {
    use forge_domain::{
        AgentId, Context, ConversationId, MaxTokens, ModelId, ProviderId, ReasoningConfig,
        Temperature, ToolDefinition, TopK, TopP,
    };
    use pretty_assertions::assert_eq;

    use super::*;

    #[derive(schemars::JsonSchema)]
    struct TestToolInput;

    #[test]
    fn test_apply_sets_parameters() {
        let reasoning = ReasoningConfig::default().max_tokens(2000);

        let agent = Agent::new(
            AgentId::new("test"),
            ProviderId::ANTHROPIC,
            ModelId::new("claude-3-5-sonnet-20241022"),
        )
        .temperature(Temperature::new(0.7).unwrap())
        .max_tokens(MaxTokens::new(1000).unwrap())
        .top_k(TopK::new(50).unwrap())
        .top_p(TopP::new(0.9).unwrap())
        .reasoning(reasoning.clone());

        let tool_def = ToolDefinition::new("test_tool")
            .description("A test tool")
            .input_schema(schemars::schema_for!(TestToolInput));

        let conversation =
            Conversation::new(ConversationId::generate()).context(Context::default());

        let actual = ApplyTunableParameters::new(agent, vec![tool_def.clone()]).apply(conversation);

        let ctx = actual.context.unwrap();
        assert_eq!(ctx.temperature, Some(Temperature::new(0.7).unwrap()));
        assert_eq!(ctx.max_tokens, Some(1000));
        assert_eq!(ctx.top_k, Some(TopK::new(50).unwrap()));
        assert_eq!(ctx.top_p, Some(TopP::new(0.9).unwrap()));
        assert_eq!(ctx.reasoning, Some(reasoning));
        assert_eq!(ctx.tools, vec![tool_def]);
    }
}
