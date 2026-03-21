use std::sync::LazyLock;

use derive_setters::Setters;
use merge::Merge;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::temperature::Temperature;
use crate::update::Update;
use crate::{Compact, MaxTokens, ReasoningEffortLevel, ServiceTier, TopK, TopP};

/// Configuration for a workflow that contains all settings
/// required to initialize a workflow.
#[derive(Debug, Clone, Serialize, Deserialize, Merge, Setters, JsonSchema)]
#[setters(strip_option, into)]
pub struct Workflow {
    /// Path pattern for custom template files (supports glob patterns)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub templates: Option<String>,

    /// configurations that can be used to update forge
    #[merge(strategy = crate::merge::option)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updates: Option<Update>,

    /// Commands that can be used to interact with the workflow
    #[merge(strategy = merge::vec::append)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<Command>,

    /// A set of custom rules that all agents should follow
    /// These rules will be applied in addition to each agent's individual rules
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub custom_rules: Option<String>,

    /// Temperature used for all agents
    ///
    /// Temperature controls the randomness in the model's output.
    /// - Lower values (e.g., 0.1) make responses more focused, deterministic,
    ///   and coherent
    /// - Higher values (e.g., 0.8) make responses more creative, diverse, and
    ///   exploratory
    /// - Valid range is 0.0 to 2.0
    /// - If not specified, each agent's individual setting or the model
    ///   provider's default will be used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub temperature: Option<Temperature>,

    /// Reasoning effort level used for all agents
    ///
    /// Controls the reasoning effort for models that support variable thinking
    /// levels (e.g., GPT-5.x). Sent as `reasoning_effort` in the API request.
    /// - `none` — no reasoning, raw completion
    /// - `minimal` — very light reasoning
    /// - `low` — light reasoning, fast responses
    /// - `medium` — balanced reasoning effort (default for most models)
    /// - `high` — thorough reasoning
    /// - `xhigh` — maximum reasoning, most thorough responses
    /// - If not specified, each agent's individual setting or the model
    ///   provider's default will be used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub reasoning_effort: Option<ReasoningEffortLevel>,

    /// Service tier for API requests
    ///
    /// Controls the processing priority for all agents:
    /// - `fast` — priority processing at 2x cost (fastest inference)
    /// - `flex` — flexible processing at reduced cost
    /// - `auto` — let the API choose the appropriate tier
    /// - If not specified, each agent's individual setting or the model
    ///   provider's default will be used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub service_tier: Option<ServiceTier>,

    /// Top-p (nucleus sampling) used for all agents
    ///
    /// Controls the diversity of the model's output by considering only the
    /// most probable tokens up to a cumulative probability threshold.
    /// - Lower values (e.g., 0.1) make responses more focused
    /// - Higher values (e.g., 0.9) make responses more diverse
    /// - Valid range is 0.0 to 1.0
    /// - If not specified, each agent's individual setting or the model
    ///   provider's default will be used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub top_p: Option<TopP>,

    /// Top-k used for all agents
    ///
    /// Controls the number of highest probability vocabulary tokens to keep.
    /// - Lower values (e.g., 10) make responses more focused
    /// - Higher values (e.g., 100) make responses more diverse
    /// - Valid range is 1 to 1000
    /// - If not specified, each agent's individual setting or the model
    ///   provider's default will be used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub top_k: Option<TopK>,

    /// Maximum number of tokens the model can generate for all agents
    ///
    /// Controls the maximum length of the model's response.
    /// - Lower values (e.g., 100) limit response length for concise outputs
    /// - Higher values (e.g., 4000) allow for longer, more detailed responses
    /// - Valid range is 1 to 100,000
    /// - If not specified, each agent's individual setting or the model
    ///   provider's default will be used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub max_tokens: Option<MaxTokens>,

    /// Flag to enable/disable tool support for all agents in this workflow.
    /// If not specified, each agent's individual setting will be used.
    /// Default is false (tools disabled) when not specified.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub tool_supported: Option<bool>,

    /// Maximum number of times a tool can fail before the orchestrator
    /// forces the completion.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub max_tool_failure_per_turn: Option<usize>,

    /// Maximum number of requests that can be made in a single turn
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub max_requests_per_turn: Option<usize>,
    /// Configuration for automatic context compaction for all agents
    /// If specified, this will be applied to all agents in the workflow
    /// If not specified, each agent's individual setting will be used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub compact: Option<Compact>,
}

static DEFAULT_WORKFLOW: LazyLock<Workflow> =
    LazyLock::new(|| serde_yml::from_str(include_str!("../../../forge.default.yaml")).unwrap());

impl Default for Workflow {
    fn default() -> Self {
        DEFAULT_WORKFLOW.clone()
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, Merge, Setters, JsonSchema)]
#[setters(strip_option, into)]
pub struct Command {
    #[merge(strategy = crate::merge::std::overwrite)]
    pub name: String,

    #[merge(strategy = crate::merge::std::overwrite)]
    pub description: String,

    #[merge(strategy = crate::merge::option)]
    pub prompt: Option<String>,
}

impl Workflow {
    /// Creates a new empty workflow with all fields set to their empty state.
    /// This is useful for testing where you want to build a workflow from
    /// scratch.
    pub fn new() -> Self {
        Self {
            custom_rules: None,
            temperature: None,
            reasoning_effort: None,
            service_tier: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            tool_supported: None,
            updates: None,
            templates: None,
            max_tool_failure_per_turn: None,
            max_requests_per_turn: None,
            compact: None,
            commands: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ModelId;

    #[test]
    fn test_workflow_new_creates_empty_workflow() {
        // Arrange

        // Act
        let actual = Workflow::new();

        // Assert
        assert_eq!(actual.custom_rules, None);
        assert_eq!(actual.temperature, None);
        assert_eq!(actual.reasoning_effort, None);
        assert_eq!(actual.service_tier, None);
        assert_eq!(actual.top_p, None);
        assert_eq!(actual.top_k, None);
        assert_eq!(actual.max_tokens, None);
        assert_eq!(actual.tool_supported, None);
        assert_eq!(actual.compact, None);
    }

    #[test]
    fn test_workflow_with_tool_supported() {
        // Arrange
        let fixture = r#"
        {
            "tool_supported": true,
            "agents": [
                {
                    "id": "test-agent",
                    "description": "Test agent"
                }
            ]
        }
        "#;

        // Act
        let actual: Workflow = serde_json::from_str(fixture).unwrap();

        // Assert
        assert_eq!(actual.tool_supported, Some(true));
    }

    #[test]
    fn test_workflow_merge_tool_supported() {
        // Fixture
        let mut base = Workflow::new();

        let other = Workflow::new().tool_supported(true);

        // Act
        base.merge(other);

        // Assert
        assert_eq!(base.tool_supported, Some(true));
    }

    #[test]
    fn test_workflow_merge_tool_supported_with_existing() {
        // Fixture
        let mut base = Workflow::new().tool_supported(false);

        let other = Workflow::new().tool_supported(true);

        // Act
        base.merge(other);

        // Assert
        assert_eq!(base.tool_supported, Some(true));
    }
    #[test]
    fn test_workflow_merge_compact() {
        // Fixture
        let mut base = Workflow::new();

        let compact = Compact::new()
            .model(ModelId::new("test-model"))
            .token_threshold(1000_usize)
            .turn_threshold(5_usize);
        let other = Workflow::new().compact(compact.clone());

        // Act
        base.merge(other);

        // Assert
        assert_eq!(base.compact, Some(compact));
    }

    #[test]
    fn test_workflow_merge_compact_with_existing() {
        // Fixture
        let existing_compact = Compact::new()
            .model(ModelId::new("existing-model"))
            .token_threshold(500_usize);
        let mut base = Workflow::new().compact(existing_compact);

        let new_compact = Compact::new()
            .model(ModelId::new("new-model"))
            .token_threshold(1000_usize)
            .turn_threshold(5_usize);
        let other = Workflow::new().compact(new_compact.clone());

        // Act
        base.merge(other);

        // Assert
        assert_eq!(base.compact, Some(new_compact));
    }
}
