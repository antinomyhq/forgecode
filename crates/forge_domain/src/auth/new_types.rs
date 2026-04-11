use serde::{Deserialize, Serialize};

#[derive(
    Clone, Serialize, Deserialize, derive_more::From, derive_more::Deref, PartialEq, Eq, Hash, Debug,
    Default,
)]
#[serde(transparent)]
pub struct ApiKey(String);

impl std::fmt::Display for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", truncate_key(&self.0))
    }
}

impl AsRef<str> for ApiKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Truncates a key string for display purposes
///
/// If the key length is 20 characters or less, returns it unchanged.
/// Otherwise, shows the first 13 characters and last 4 characters with "..." in
/// between.
///
/// # Arguments
/// * `key` - The key string to truncate
///
/// # Returns
/// * A truncated version of the key for safe display
pub fn truncate_key(key: &str) -> String {
    if key.len() <= 20 {
        key.to_string()
    } else {
        format!("{}...{}", &key[..=12], &key[key.len() - 4..])
    }
}

#[derive(
    Clone, Serialize, Deserialize, derive_more::From, derive_more::Deref, PartialEq, Eq, Debug,
)]
#[serde(transparent)]
pub struct AuthorizationCode(String);

#[derive(
    Clone, Serialize, Deserialize, derive_more::From, derive_more::Deref, PartialEq, Eq, Debug,
)]
#[serde(transparent)]
pub struct DeviceCode(String);

#[derive(
    Clone, Serialize, Deserialize, derive_more::From, derive_more::Deref, PartialEq, Eq, Debug,
)]
#[serde(transparent)]
pub struct PkceVerifier(String);

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    derive_more::Deref,
    Hash,
    derive_more::From,
    derive_more::Display,
)]
#[serde(transparent)]
pub struct URLParam(String);

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, derive_more::Deref, derive_more::From,
)]
#[serde(transparent)]
pub struct URLParamValue(String);

/// A URL parameter specification with its name and optional preset options.
///
/// When `options` is `Some`, the UI presents a dropdown for selection.
/// When `options` is `None`, the UI presents a free-text input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct URLParamSpec {
    /// The parameter name used as the template variable and credential map key.
    pub name: URLParam,
    /// Optional list of allowed values. When present, the UI renders a
    /// dropdown.
    pub options: Option<Vec<String>>,
}

impl URLParamSpec {
    /// Creates a `URLParamSpec` with only a name, rendering as a free-text
    /// input.
    pub fn new(name: impl Into<URLParam>) -> Self {
        Self { name: name.into(), options: None }
    }

    /// Creates a `URLParamSpec` with preset options, rendering as a dropdown.
    pub fn with_options(name: impl Into<URLParam>, options: Vec<String>) -> Self {
        Self { name: name.into(), options: Some(options) }
    }
}

impl From<URLParam> for URLParamSpec {
    fn from(name: URLParam) -> Self {
        Self::new(name)
    }
}

impl From<String> for URLParamSpec {
    fn from(name: String) -> Self {
        Self::new(URLParam::from(name))
    }
}

#[derive(
    Clone,
    Serialize,
    Deserialize,
    derive_more::From,
    derive_more::Display,
    derive_more::Deref,
    Debug,
    PartialEq,
    Eq,
)]
#[serde(transparent)]
pub struct UserCode(String);

#[derive(
    Clone, Serialize, Deserialize, derive_more::From, derive_more::Deref, PartialEq, Eq, Debug,
)]
#[serde(transparent)]
pub struct State(String);

#[derive(
    Clone, Serialize, Deserialize, derive_more::From, derive_more::Deref, PartialEq, Eq, Debug,
)]
#[serde(transparent)]
pub struct RefreshToken(String);

#[derive(
    Clone,
    Serialize,
    Deserialize,
    derive_more::From,
    derive_more::Display,
    derive_more::Deref,
    PartialEq,
    Eq,
    Debug,
)]
#[serde(transparent)]
pub struct AccessToken(String);
