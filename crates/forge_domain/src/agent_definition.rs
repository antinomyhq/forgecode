use std::borrow::Cow;

use derive_more::derive::Display;
use derive_setters::Setters;
use merge::Merge;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::Display as StrumDisplay;

use crate::compact::Compact;
use crate::temperature::Temperature;
use crate::template::Template;
use crate::{
    EventContext, MaxTokens, ModelId, ProviderId, ReasoningEffortLevel, ServiceTier, SystemContext, ToolName,
    TopK, TopP,
};

// Unique identifier for an agent
#[derive(Debug, Display, Eq, PartialEq, Hash, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct AgentId(Cow<'static, str>);

impl From<&str> for AgentId {
    fn from(value: &str) -> Self {
        AgentId(Cow::Owned(value.to_string()))
    }
}

impl AgentId {
    // Creates a new agent ID from a string-like value
    pub fn new(id: impl ToString) -> Self {
        Self(Cow::Owned(id.to_string()))
    }

    // Returns the agent ID as a string reference
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    pub const FORGE: AgentId = AgentId(Cow::Borrowed("forge"));
    pub const MUSE: AgentId = AgentId(Cow::Borrowed("muse"));
    pub const SAGE: AgentId = AgentId(Cow::Borrowed("sage"));
}

impl Default for AgentId {
    fn default() -> Self {
        AgentId::FORGE
    }
}

/// Agent definition - used for deserialization from configuration files
/// Fields like model and provider are optional to support defaults
#[derive(Debug, Clone, Serialize, Deserialize, Merge, Setters, JsonSchema)]
#[setters(strip_option, into)]
#[serde(rename = "Agent")]
pub struct AgentDefinition {
    /// Flag to enable/disable tool support for this agent.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub tool_supported: Option<bool>,

    // Unique identifier for the agent
    #[merge(strategy = crate::merge::std::overwrite)]
    pub id: AgentId,

    /// Path to the agent definition file, if loaded from a file
    #[serde(skip)]
    #[merge(strategy = crate::merge::std::overwrite)]
    pub path: Option<String>,

    /// Human-readable title for the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub provider: Option<ProviderId>,

    // The language model ID to be used by this agent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub model: Option<ModelId>,

    // Human-readable description of the agent's purpose
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub description: Option<String>,

    // Template for the system prompt provided to the agent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub system_prompt: Option<Template<SystemContext>>,

    // Template for the user prompt provided to the agent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub user_prompt: Option<Template<EventContext>>,

    /// Tools that the agent can use
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = merge_opt_vec)]
    pub tools: Option<Vec<ToolName>>,

    /// Maximum number of turns the agent can take
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub max_turns: Option<u64>,

    /// Configuration for automatic context compaction
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub compact: Option<Compact>,

    /// A set of custom rules that the agent should follow
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub custom_rules: Option<String>,

    /// Temperature used for agent
    ///
    /// Temperature controls the randomness in the model's output.
    /// - Lower values (e.g., 0.1) make responses more focused, deterministic,
    ///   and coherent
    /// - Higher values (e.g., 0.8) make responses more creative, diverse, and
    ///   exploratory
    /// - Valid range is 0.0 to 2.0
    /// - If not specified, the model provider's default temperature will be
    ///   used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub temperature: Option<Temperature>,

    /// Reasoning effort level for this agent
    ///
    /// Controls the reasoning effort for models that support variable thinking
    /// levels (e.g., GPT-5.x). Sent as `reasoning_effort` in the API request.
    /// - `none` — no reasoning, raw completion
    /// - `minimal` — very light reasoning
    /// - `low` — light reasoning, fast responses
    /// - `medium` — balanced reasoning effort (default for most models)
    /// - `high` — thorough reasoning
    /// - `xhigh` — maximum reasoning, most thorough responses
    /// - If not specified, the model provider's default will be used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub reasoning_effort: Option<ReasoningEffortLevel>,

    /// Service tier for API requests
    ///
    /// Controls the processing priority for models that support service tiers:
    /// - `fast` — priority processing at 2x cost (fastest inference)
    /// - `flex` — flexible processing at reduced cost
    /// - `auto` — let the API choose the appropriate tier
    /// - If not specified, the model provider's default will be used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub service_tier: Option<ServiceTier>,

    /// Top-p (nucleus sampling) used for agent
    ///
    /// Controls the diversity of the model's output by considering only the
    /// most probable tokens up to a cumulative probability threshold.
    /// - Lower values (e.g., 0.1) make responses more focused
    /// - Higher values (e.g., 0.9) make responses more diverse
    /// - Valid range is 0.0 to 1.0
    /// - If not specified, the model provider's default will be used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub top_p: Option<TopP>,

    /// Top-k used for agent
    ///
    /// Controls the number of highest probability vocabulary tokens to keep.
    /// - Lower values (e.g., 10) make responses more focused
    /// - Higher values (e.g., 100) make responses more diverse
    /// - Valid range is 1 to 1000
    /// - If not specified, the model provider's default will be used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub top_k: Option<TopK>,

    /// Maximum number of tokens the model can generate
    ///
    /// Controls the maximum length of the model's response.
    /// - Lower values (e.g., 100) limit response length for concise outputs
    /// - Higher values (e.g., 4000) allow for longer, more detailed responses
    /// - Valid range is 1 to 100,000
    /// - If not specified, the model provider's default will be used
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub max_tokens: Option<MaxTokens>,

    /// Reasoning configuration for the agent.
    /// Controls the reasoning capabilities of the agent
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub reasoning: Option<ReasoningConfig>,
    /// Maximum number of times a tool can fail before sending the response back
    /// to the LLM forces the completion.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub max_tool_failure_per_turn: Option<usize>,

    /// Maximum number of requests that can be made in a single turn
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub max_requests_per_turn: Option<usize>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, Merge, Setters, JsonSchema, PartialEq)]
#[setters(strip_option)]
#[merge(strategy = merge::option::overwrite_none)]
pub struct ReasoningConfig {
    /// Controls the effort level of the agent's reasoning
    /// supported by openrouter and forge provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,

    /// Controls how many tokens the model can spend thinking.
    /// supported by openrouter, anthropic and forge provider
    /// should be greater then 1024 but less than overall max_tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,

    /// Model thinks deeply, but the reasoning is hidden from you.
    /// supported by openrouter and forge provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<bool>,

    /// Enables reasoning at the “medium” effort level with no exclusions.
    /// supported by openrouter, anthropic and forge provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, StrumDisplay)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Effort {
    High,
    Medium,
    Low,
}

/// Converts a thinking budget (max_tokens) to Effort
/// - 0-1024 → Low
/// - 1025-8192 → Medium
/// - 8193+ → High
impl From<usize> for Effort {
    fn from(budget: usize) -> Self {
        if budget <= 1024 {
            Effort::Low
        } else if budget <= 8192 {
            Effort::Medium
        } else {
            Effort::High
        }
    }
}

fn merge_opt_vec<T>(base: &mut Option<Vec<T>>, other: Option<Vec<T>>) {
    if let Some(other) = other {
        if let Some(base) = base {
            base.extend(other);
        } else {
            *base = Some(other);
        }
    }
}

impl AgentDefinition {
    /// Creates a new AgentDefinition with the given ID
    pub fn new(id: impl Into<AgentId>) -> Self {
        Self {
            id: id.into(),
            title: Default::default(),
            tool_supported: Default::default(),
            model: Default::default(),
            description: Default::default(),
            system_prompt: Default::default(),
            user_prompt: Default::default(),
            tools: Default::default(),
            max_turns: Default::default(),
            compact: Default::default(),
            custom_rules: Default::default(),
            temperature: Default::default(),
            reasoning_effort: Default::default(),
            service_tier: Default::default(),
            top_p: Default::default(),
            top_k: Default::default(),
            max_tokens: Default::default(),
            reasoning: Default::default(),
            max_tool_failure_per_turn: Default::default(),
            max_requests_per_turn: Default::default(),
            provider: Default::default(),
            path: Default::default(),
        }
    }
}

/// Estimates the token count from a string representation
/// This is a simple estimation that should be replaced with a more accurate
/// tokenizer
/// Estimates token count from a string representation
/// Re-exported for compaction reporting
pub fn estimate_token_count(count: usize) -> usize {
    // A very rough estimation that assumes ~4 characters per token on average
    // In a real implementation, this should use a proper LLM-specific tokenizer
    count / 4
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_effort_from_budget_low() {
        assert_eq!(Effort::from(0), Effort::Low);
        assert_eq!(Effort::from(1), Effort::Low);
        assert_eq!(Effort::from(1024), Effort::Low);
    }

    #[test]
    fn test_effort_from_budget_medium() {
        assert_eq!(Effort::from(1025), Effort::Medium);
        assert_eq!(Effort::from(5000), Effort::Medium);
        assert_eq!(Effort::from(8192), Effort::Medium);
    }

    #[test]
    fn test_effort_from_budget_high() {
        assert_eq!(Effort::from(8193), Effort::High);
        assert_eq!(Effort::from(10000), Effort::High);
        assert_eq!(Effort::from(100000), Effort::High);
    }

    #[test]
    fn test_merge_model() {
        // Base has a value, should not be overwritten
        let mut base = AgentDefinition::new("Base").model(ModelId::new("base"));
        let other = AgentDefinition::new("Other").model(ModelId::new("other"));
        base.merge(other);
        assert_eq!(base.model.unwrap(), ModelId::new("other"));

        // Base has no value, should take the other value
        let mut base = AgentDefinition::new("Base"); // No model
        let other = AgentDefinition::new("Other").model(ModelId::new("other"));
        base.merge(other);
        assert_eq!(base.model.unwrap(), ModelId::new("other"));
    }

    #[test]
    fn test_merge_tool_supported() {
        // Base has no value, should use other's value
        let mut base = AgentDefinition::new("Base"); // No tool_supported set
        let other = AgentDefinition::new("Other").tool_supported(true);
        base.merge(other);
        assert_eq!(base.tool_supported, Some(true));

        // Base has a value, should not be overwritten
        let mut base = AgentDefinition::new("Base").tool_supported(false);
        let other = AgentDefinition::new("Other").tool_supported(true);
        base.merge(other);
        assert_eq!(base.tool_supported, Some(true));
    }

    #[test]
    fn test_merge_tools() {
        // Base has no value, should take other's values
        let mut base = AgentDefinition::new("Base"); // no tools
        let other = AgentDefinition::new("Other")
            .tools(vec![ToolName::new("tool2"), ToolName::new("tool3")]);
        base.merge(other);

        // Should contain all tools from the other agent
        let tools = base.tools.as_ref().unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&ToolName::new("tool2")));
        assert!(tools.contains(&ToolName::new("tool3")));

        // Base has a value, should merge with other's tools
        let mut base = AgentDefinition::new("Base")
            .tools(vec![ToolName::new("tool1"), ToolName::new("tool2")]);
        let other = AgentDefinition::new("Other")
            .tools(vec![ToolName::new("tool3"), ToolName::new("tool4")]);
        base.merge(other);

        // Should have other's tools
        let tools = base.tools.as_ref().unwrap();
        assert_eq!(tools.len(), 4);
        assert!(tools.contains(&ToolName::new("tool1")));
        assert!(tools.contains(&ToolName::new("tool2")));
        assert!(tools.contains(&ToolName::new("tool3")));
        assert!(tools.contains(&ToolName::new("tool4")));
    }

    #[test]
    fn test_temperature_validation() {
        // Valid temperature values should deserialize correctly
        let valid_temps = [0.0, 0.5, 1.0, 1.5, 2.0];
        for temp in valid_temps {
            let json = json!({
                "id": "test-agent",
                "temperature": temp
            });

            let agent: std::result::Result<AgentDefinition, serde_json::Error> =
                serde_json::from_value(json);
            assert!(agent.is_ok(), "Valid temperature {temp} should deserialize");
            assert_eq!(agent.unwrap().temperature.unwrap().value(), temp);
        }

        // Invalid temperature values should fail deserialization
        let invalid_temps = [-0.1, 2.1, 3.0, -1.0, 10.0];
        for temp in invalid_temps {
            let json = json!({
                "id": "test-agent",
                "temperature": temp
            });

            let agent: std::result::Result<AgentDefinition, serde_json::Error> =
                serde_json::from_value(json);
            assert!(
                agent.is_err(),
                "Invalid temperature {temp} should fail deserialization"
            );
            let err = agent.unwrap_err().to_string();
            assert!(
                err.contains("temperature must be between 0.0 and 2.0"),
                "Error should mention valid range: {err}"
            );
        }

        // No temperature should deserialize to None
        let json = json!({
            "id": "test-agent"
        });

        let agent: AgentDefinition = serde_json::from_value(json).unwrap();
        assert_eq!(agent.temperature, None);
    }

    #[test]
    fn test_top_p_validation() {
        // Valid top_p values should deserialize correctly
        let valid_values = [0.0, 0.1, 0.5, 0.9, 1.0];
        for value in valid_values {
            let json = json!({
                "id": "test-agent",
                "top_p": value
            });

            let agent: std::result::Result<AgentDefinition, serde_json::Error> =
                serde_json::from_value(json);
            assert!(agent.is_ok(), "Valid top_p {value} should deserialize");
            assert_eq!(agent.unwrap().top_p.unwrap().value(), value);
        }

        // Invalid top_p values should fail deserialization
        let invalid_values = [-0.1, 1.1, 2.0, -1.0, 10.0];
        for value in invalid_values {
            let json = json!({
                "id": "test-agent",
                "top_p": value
            });

            let agent: std::result::Result<AgentDefinition, serde_json::Error> =
                serde_json::from_value(json);
            assert!(
                agent.is_err(),
                "Invalid top_p {value} should fail deserialization"
            );
            let err = agent.unwrap_err().to_string();
            assert!(
                err.contains("top_p must be between 0.0 and 1.0"),
                "Error should mention valid range: {err}"
            );
        }

        // No top_p should deserialize to None
        let json = json!({
            "id": "test-agent"
        });

        let agent: AgentDefinition = serde_json::from_value(json).unwrap();
        assert_eq!(agent.top_p, None);
    }

    #[test]
    fn test_top_k_validation() {
        // Valid top_k values should deserialize correctly
        let valid_values = [1, 10, 50, 100, 500, 1000];
        for value in valid_values {
            let json = json!({
                "id": "test-agent",
                "top_k": value
            });

            let agent: std::result::Result<AgentDefinition, serde_json::Error> =
                serde_json::from_value(json);
            assert!(agent.is_ok(), "Valid top_k {value} should deserialize");
            assert_eq!(agent.unwrap().top_k.unwrap().value(), value);
        }

        // Invalid top_k values should fail deserialization
        let invalid_values = [0, 1001, 2000, 5000];
        for value in invalid_values {
            let json = json!({
                "id": "test-agent",
                "top_k": value
            });

            let agent: std::result::Result<AgentDefinition, serde_json::Error> =
                serde_json::from_value(json);
            assert!(
                agent.is_err(),
                "Invalid top_k {value} should fail deserialization"
            );
            let err = agent.unwrap_err().to_string();
            assert!(
                err.contains("top_k must be between 1 and 1000"),
                "Error should mention valid range: {err}"
            );
        }

        // No top_k should deserialize to None
        let json = json!({
            "id": "test-agent"
        });

        let agent: AgentDefinition = serde_json::from_value(json).unwrap();
        assert_eq!(agent.top_k, None);
    }

    #[test]
    fn test_max_tokens_validation() {
        // Valid max_tokens values should deserialize correctly
        let valid_values = [1, 100, 1000, 4000, 8000, 100_000];
        for value in valid_values {
            let json = json!({
                "id": "test-agent",
                "max_tokens": value
            });

            let agent: std::result::Result<AgentDefinition, serde_json::Error> =
                serde_json::from_value(json);
            assert!(agent.is_ok(), "Valid max_tokens {value} should deserialize");
            assert_eq!(agent.unwrap().max_tokens.unwrap().value(), value);
        }

        // Invalid max_tokens values should fail deserialization
        let invalid_values = [0, 100_001, 200_000, 1_000_000];
        for value in invalid_values {
            let json = json!({
                "id": "test-agent",
                "max_tokens": value
            });

            let agent: std::result::Result<AgentDefinition, serde_json::Error> =
                serde_json::from_value(json);
            assert!(
                agent.is_err(),
                "Invalid max_tokens {value} should fail deserialization"
            );
            let err = agent.unwrap_err().to_string();
            assert!(
                err.contains("max_tokens must be between 1 and 100000"),
                "Error should mention valid range: {err}"
            );
        }

        // No max_tokens should deserialize to None
        let json = json!({
            "id": "test-agent"
        });

        let agent: AgentDefinition = serde_json::from_value(json).unwrap();
        assert_eq!(agent.max_tokens, None);
    }
}
