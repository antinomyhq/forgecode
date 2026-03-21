use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Controls the service tier for API requests.
///
/// This parameter is sent as `service_tier` in the OpenAI API request body.
/// It controls the processing priority and cost of requests:
/// - `fast` — priority processing at 2x cost (maps to "priority" in the API)
/// - `flex` — flexible processing at reduced cost
/// - `auto` — let the API choose the appropriate tier (default)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ServiceTier {
    Fast,
    Flex,
    Auto,
}

impl ServiceTier {
    /// Returns the string representation used in API requests.
    /// Note: `Fast` maps to "priority" per OpenAI API spec.
    pub fn api_str(&self) -> &'static str {
        match self {
            ServiceTier::Fast => "priority",
            ServiceTier::Flex => "flex",
            ServiceTier::Auto => "auto",
        }
    }
}

impl fmt::Display for ServiceTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceTier::Fast => write!(f, "fast"),
            ServiceTier::Flex => write!(f, "flex"),
            ServiceTier::Auto => write!(f, "auto"),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_service_tier_serialization() {
        assert_eq!(
            serde_json::to_value(ServiceTier::Fast).unwrap(),
            json!("fast")
        );
        assert_eq!(
            serde_json::to_value(ServiceTier::Flex).unwrap(),
            json!("flex")
        );
        assert_eq!(
            serde_json::to_value(ServiceTier::Auto).unwrap(),
            json!("auto")
        );
    }

    #[test]
    fn test_service_tier_deserialization() {
        let fast: ServiceTier = serde_json::from_value(json!("fast")).unwrap();
        assert_eq!(fast, ServiceTier::Fast);

        let flex: ServiceTier = serde_json::from_value(json!("flex")).unwrap();
        assert_eq!(flex, ServiceTier::Flex);

        let auto: ServiceTier = serde_json::from_value(json!("auto")).unwrap();
        assert_eq!(auto, ServiceTier::Auto);
    }

    #[test]
    fn test_service_tier_api_str() {
        assert_eq!(ServiceTier::Fast.api_str(), "priority");
        assert_eq!(ServiceTier::Flex.api_str(), "flex");
        assert_eq!(ServiceTier::Auto.api_str(), "auto");
    }

    #[test]
    fn test_service_tier_display() {
        assert_eq!(ServiceTier::Fast.to_string(), "fast");
        assert_eq!(ServiceTier::Flex.to_string(), "flex");
        assert_eq!(ServiceTier::Auto.to_string(), "auto");
    }
}
