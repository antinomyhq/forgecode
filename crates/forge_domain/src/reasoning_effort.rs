use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Controls the reasoning effort level for models that support variable thinking.
///
/// This parameter is sent directly as `reasoning_effort` in the OpenAI-compatible
/// API request body. Models like GPT-5.x and Codex use this to control how much
/// computation is spent on reasoning:
/// - `none` — no reasoning, raw completion
/// - `minimal` — very light reasoning
/// - `low` — light reasoning, fast responses
/// - `medium` — balanced reasoning effort (default for most models)
/// - `high` — thorough reasoning
/// - `xhigh` — maximum reasoning, most thorough responses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffortLevel {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

impl ReasoningEffortLevel {
    /// Returns the string representation used in API requests
    pub fn as_str(&self) -> &'static str {
        match self {
            ReasoningEffortLevel::None => "none",
            ReasoningEffortLevel::Minimal => "minimal",
            ReasoningEffortLevel::Low => "low",
            ReasoningEffortLevel::Medium => "medium",
            ReasoningEffortLevel::High => "high",
            ReasoningEffortLevel::Xhigh => "xhigh",
        }
    }
}

impl fmt::Display for ReasoningEffortLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_reasoning_effort_level_serialization() {
        assert_eq!(
            serde_json::to_value(ReasoningEffortLevel::None).unwrap(),
            json!("none")
        );
        assert_eq!(
            serde_json::to_value(ReasoningEffortLevel::Minimal).unwrap(),
            json!("minimal")
        );
        assert_eq!(
            serde_json::to_value(ReasoningEffortLevel::Low).unwrap(),
            json!("low")
        );
        assert_eq!(
            serde_json::to_value(ReasoningEffortLevel::Medium).unwrap(),
            json!("medium")
        );
        assert_eq!(
            serde_json::to_value(ReasoningEffortLevel::High).unwrap(),
            json!("high")
        );
        assert_eq!(
            serde_json::to_value(ReasoningEffortLevel::Xhigh).unwrap(),
            json!("xhigh")
        );
    }

    #[test]
    fn test_reasoning_effort_level_deserialization() {
        let none: ReasoningEffortLevel = serde_json::from_value(json!("none")).unwrap();
        assert_eq!(none, ReasoningEffortLevel::None);

        let minimal: ReasoningEffortLevel = serde_json::from_value(json!("minimal")).unwrap();
        assert_eq!(minimal, ReasoningEffortLevel::Minimal);

        let low: ReasoningEffortLevel = serde_json::from_value(json!("low")).unwrap();
        assert_eq!(low, ReasoningEffortLevel::Low);

        let medium: ReasoningEffortLevel = serde_json::from_value(json!("medium")).unwrap();
        assert_eq!(medium, ReasoningEffortLevel::Medium);

        let high: ReasoningEffortLevel = serde_json::from_value(json!("high")).unwrap();
        assert_eq!(high, ReasoningEffortLevel::High);

        let xhigh: ReasoningEffortLevel = serde_json::from_value(json!("xhigh")).unwrap();
        assert_eq!(xhigh, ReasoningEffortLevel::Xhigh);
    }

    #[test]
    fn test_reasoning_effort_level_invalid_deserialization() {
        let result: Result<ReasoningEffortLevel, _> = serde_json::from_value(json!("invalid"));
        assert!(result.is_err());
    }

    #[test]
    fn test_reasoning_effort_level_display() {
        assert_eq!(ReasoningEffortLevel::None.to_string(), "none");
        assert_eq!(ReasoningEffortLevel::Minimal.to_string(), "minimal");
        assert_eq!(ReasoningEffortLevel::Low.to_string(), "low");
        assert_eq!(ReasoningEffortLevel::Medium.to_string(), "medium");
        assert_eq!(ReasoningEffortLevel::High.to_string(), "high");
        assert_eq!(ReasoningEffortLevel::Xhigh.to_string(), "xhigh");
    }

    #[test]
    fn test_reasoning_effort_level_as_str() {
        assert_eq!(ReasoningEffortLevel::None.as_str(), "none");
        assert_eq!(ReasoningEffortLevel::Minimal.as_str(), "minimal");
        assert_eq!(ReasoningEffortLevel::Low.as_str(), "low");
        assert_eq!(ReasoningEffortLevel::Medium.as_str(), "medium");
        assert_eq!(ReasoningEffortLevel::High.as_str(), "high");
        assert_eq!(ReasoningEffortLevel::Xhigh.as_str(), "xhigh");
    }

    #[test]
    fn test_reasoning_effort_level_in_struct() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            reasoning_effort: ReasoningEffortLevel,
        }

        let json = json!({
            "reasoning_effort": "medium"
        });
        let test_struct: Result<TestStruct, _> = serde_json::from_value(json);
        assert!(test_struct.is_ok());
        assert_eq!(
            test_struct.unwrap().reasoning_effort,
            ReasoningEffortLevel::Medium
        );
    }
}
