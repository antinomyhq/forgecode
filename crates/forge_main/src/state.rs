use std::path::PathBuf;

use derive_setters::Setters;
use forge_api::{ConversationId, Environment};
use forge_domain::{ReasoningEffortLevel, ServiceTier};

//TODO: UIState and ForgePrompt seem like the same thing and can be merged
/// State information for the UI
#[derive(Debug, Default, Clone, Setters)]
#[setters(strip_option)]
pub struct UIState {
    pub cwd: PathBuf,
    pub conversation_id: Option<ConversationId>,
    pub fast_mode: bool,
    pub service_tier: Option<ServiceTier>,
    pub reasoning_effort: Option<ReasoningEffortLevel>,
}

impl UIState {
    pub fn new(env: Environment) -> Self {
        Self { cwd: env.cwd, conversation_id: Default::default(), fast_mode: false, service_tier: None, reasoning_effort: None }
    }
}
