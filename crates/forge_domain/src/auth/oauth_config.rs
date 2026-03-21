use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(
    Clone,
    Serialize,
    Deserialize,
    derive_more::From,
    derive_more::Deref,
    PartialEq,
    Eq,
    Debug,
    JsonSchema,
)]
#[serde(transparent)]
#[schemars(with = "String")]
pub struct ClientId(String);

/// OAuth configuration for authentication flows
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OAuthConfig {
    /// The URL to initiate the OAuth authorization or device code flow
    #[schemars(with = "String")]
    pub auth_url: Url,
    /// The URL to exchange authorization codes or device codes for access
    /// tokens
    #[schemars(with = "String")]
    pub token_url: Url,
    /// The OAuth client identifier
    pub client_id: ClientId,
    /// The OAuth scopes to request
    pub scopes: Vec<String>,
    /// The redirect URI for authorization code flows
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
    /// Whether to use PKCE (Proof Key for Code Exchange)
    #[serde(default)]
    pub use_pkce: bool,
    /// The URL to refresh the token after OAuth-with-API-Key flow
    #[schemars(with = "Option<String>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_refresh_url: Option<Url>,
    /// Custom HTTP headers to include in OAuth requests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_headers: Option<HashMap<String, String>>,
    /// Extra parameters to include in the authorization request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_auth_params: Option<HashMap<String, String>>,
}
