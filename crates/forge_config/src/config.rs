use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Error;
use crate::read::read;

/// Root configuration type for the forge_config crate.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ForgeConfig {
    /// Per-agent configuration overrides, keyed by agent identifier.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub agents: HashMap<AgentId, AgentConfig>,

    /// Base URL for Forge's backend APIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,

    /// Format for automatically creating a dump when a task is completed.
    /// Set to "json" (or "true"/"1"/"yes") for JSON, "html" for HTML, or
    /// omit to disable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_dump: Option<AutoDumpFormat>,

    /// Whether to automatically open HTML dump files in the browser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_open_dump: Option<bool>,

    /// Custom banner text displayed on startup instead of the default ASCII
    /// art.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<String>,

    /// Compaction settings controlling when and how conversation history is
    /// summarized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction: Option<CompactionConfig>,

    /// Model identifier for commit message generation (e.g. `"gpt-4o"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_model_id: Option<String>,

    /// Provider identifier for commit message generation (e.g. `"openai"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_provider_id: Option<String>,

    /// Currency display settings for the ZSH theme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<CurrencyConfig>,

    /// Custom history file path. If omitted, uses the default history path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_history_path: Option<String>,

    /// Path where debug request files should be written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_requests: Option<String>,

    /// HTTP client settings controlling timeouts, TLS, and connection pooling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpConfig>,

    /// Maximum number of conversations to show in list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_conversations: Option<usize>,

    /// Maximum number of file extensions to include in the system prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_extensions: Option<usize>,

    /// Maximum characters for fetch content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fetch_length: Option<usize>,

    /// Maximum characters per line for file read operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_line_length: Option<usize>,

    /// Maximum number of lines to read from a file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_lines: Option<u64>,

    /// Maximum number of files that can be open in a single batch operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_open: Option<usize>,

    /// Maximum file size in bytes for operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_size: Option<u64>,

    /// Maximum image file size in bytes for binary read operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_image_size: Option<u64>,

    /// Maximum number of tool-calling requests the agent may make in a single
    /// turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_requests_per_turn: Option<usize>,

    /// Maximum number of lines returned for FSSearch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_search_lines: Option<usize>,

    /// Maximum bytes allowed for search results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_search_result_size: Option<usize>,

    /// Maximum number of consecutive tool failures allowed before the agent
    /// aborts the turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_failure_per_turn: Option<usize>,

    /// Identifier of the default model to use (e.g. `"gpt-4o"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,

    /// Identifier of the default provider that hosts the model (e.g.
    /// `"openai"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,

    /// Named presets keyed by preset identifier, each bundling model and
    /// sampling settings.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub presets: HashMap<PresetId, PresetConfig>,

    /// Identifier of the preset to activate by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset_id: Option<PresetId>,

    /// Retry behaviour for failed HTTP requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryConfig>,

    /// Maximum number of results to return from initial vector search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sem_search_limit: Option<usize>,

    /// Top-k parameter for relevance filtering during semantic search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sem_search_top_k: Option<usize>,

    /// Maximum characters per line for shell output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_max_line_length: Option<usize>,

    /// Maximum lines for shell output prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_max_prefix_lines: Option<usize>,

    /// Maximum lines for shell output suffix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_max_suffix_lines: Option<usize>,

    /// Model identifier for code suggestion generation (e.g. `"gpt-4o"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggest_model_id: Option<String>,

    /// Provider identifier for code suggestion generation (e.g. `"openai"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggest_provider_id: Option<String>,

    /// Directory path containing custom Handlebars prompt templates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates_dir: Option<String>,

    /// Maximum execution time in seconds for a single tool call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_timeout: Option<u64>,

    /// Automatic update settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<UpdateConfig>,
}

impl ForgeConfig {
    /// Reads a [`ForgeConfig`] from `.env` files, YAML/JSON config at
    /// `~/forge/forge.{yaml,json}`, and environment variables.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if any source fails to parse or deserialization fails.
    pub async fn read() -> Result<Self, Error> {
        read().await
    }

    /// Returns all configurable fields as `(env_var, description)` tuples
    /// derived from the JSON Schema. Nested structs (e.g. `compaction`) are
    /// expanded using the `FORGE_PARENT__CHILD` double-underscore separator
    /// convention used by the environment variable reader.
    pub fn env_vars() -> Vec<(String, String)> {
        crate::read::env_vars()
    }
}

/// Frequency at which update checks are performed.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdateFrequency {
    Daily,
    Weekly,
    #[default]
    Always,
}

/// The output format used when auto-dumping a conversation on task completion.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutoDumpFormat {
    /// Dump as a JSON file.
    Json,
    /// Dump as an HTML file.
    Html,
}

fn deserialize_optional_percentage<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let value = Option::<f64>::deserialize(deserializer)?;
    if let Some(v) = value
        && !(0.0..=1.0).contains(&v)
    {
        return Err(Error::custom(format!(
            "percentage must be between 0.0 and 1.0, got {v}"
        )));
    }

    Ok(value)
}

/// Unique identifier for a named preset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct PresetId(String);

/// Unique identifier for a named agent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct AgentId(pub String);

/// TLS backend selection for HTTP connections.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TlsBackend {
    #[default]
    Default,
    Rustls,
}

/// TLS protocol version constraint.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub enum TlsVersion {
    #[serde(rename = "1.0")]
    V1_0,
    #[serde(rename = "1.1")]
    V1_1,
    #[serde(rename = "1.2")]
    V1_2,
    #[default]
    #[serde(rename = "1.3")]
    V1_3,
}

/// Model preset configuration that bundles sampling parameters, prompts,
/// tool selections, and custom request overrides into a reusable profile.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PresetConfig {
    /// Sampling temperature controlling randomness (0.0 = deterministic, higher
    /// = more random).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Nucleus sampling threshold; only tokens whose cumulative probability
    /// reaches this value are considered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Limits sampling to the top-k most probable tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,

    /// Maximum number of tokens the model may generate in a single response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Hint for models that support variable reasoning depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,

    /// System prompt prepended to every conversation using this preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,

    /// User prompt template injected at the start of a conversation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_prompt: Option<String>,

    /// List of tool names enabled for this preset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,

    /// Extra HTTP headers merged into every request made with this preset.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub request_headers: HashMap<String, String>,

    /// JSON body overrides applied to outgoing requests via JSON-pointer paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub request_body: Vec<BodyParam>,
}

/// Hint controlling how much internal reasoning a model should perform.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
    /// Provider-specific or free-form reasoning effort value.
    Custom(String),
}

/// A single JSON body override targeting a specific path in the request
/// payload.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BodyParam {
    /// JSON-pointer segments identifying where to set the value (e.g.
    /// `["parameters", "stop"]`).
    pub path: Vec<String>,
    /// The value to insert at the given path.
    pub value: Value,
}

/// Controls when and how conversation history is compacted to stay within
/// context limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct CompactionConfig {
    /// Maximum percentage of the context that can be summarized during
    /// compaction.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_percentage"
    )]
    pub eviction_window: Option<f64>,

    /// Maximum number of tokens to keep after compaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,

    /// Maximum number of messages before triggering compaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_threshold: Option<usize>,

    /// Whether to trigger compaction when the last message is from a user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_turn_end: Option<bool>,

    /// Number of most recent messages to preserve during compaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_window: Option<usize>,

    /// Maximum number of tokens before triggering compaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_threshold: Option<usize>,

    /// Maximum number of conversation turns before triggering compaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_threshold: Option<usize>,
}

/// Currency display settings used in the ZSH theme.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct CurrencyConfig {
    /// Conversion rate applied to token costs for currency display.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversion_rate: Option<f64>,

    /// Currency symbol used for cost display (e.g. `"$"`, `"€"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// HTTP client settings controlling timeouts, TLS, DNS, and connection pooling.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct HttpConfig {
    /// Accept invalid TLS certificates. Use with caution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept_invalid_certs: Option<bool>,

    /// Enable HTTP/2 adaptive window sizing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptive_window: Option<bool>,

    /// Connection timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect_timeout: Option<u64>,

    /// Use Hickory DNS resolver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hickory: Option<bool>,

    /// Keep-alive interval in seconds. Set to `null` to disable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_alive_interval: Option<u64>,

    /// Keep-alive timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_alive_timeout: Option<u64>,

    /// Keep-alive while connection is idle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_alive_while_idle: Option<bool>,

    /// Maximum number of redirects to follow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_redirects: Option<usize>,

    /// Maximum TLS protocol version to use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tls_version: Option<TlsVersion>,

    /// Minimum TLS protocol version to use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_tls_version: Option<TlsVersion>,

    /// Pool idle timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_idle_timeout: Option<u64>,

    /// Maximum number of idle connections per host in the connection pool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_max_idle_per_host: Option<usize>,

    /// Read timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_timeout: Option<u64>,

    /// Paths to root certificate files (PEM, CRT, CER format).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_cert_paths: Option<Vec<String>>,

    /// TLS backend to use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_backend: Option<TlsBackend>,
}

/// Retry behaviour for failed HTTP requests.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RetryConfig {
    /// Backoff multiplication factor applied on each successive retry attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backoff_factor: Option<u64>,

    /// Initial delay in milliseconds before the first retry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_delay: Option<u64>,

    /// Maximum number of retry attempts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<usize>,

    /// Maximum delay between retries in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_delay: Option<u64>,

    /// Minimum delay in milliseconds between retry attempts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_delay: Option<u64>,

    /// HTTP status codes that should trigger retries (e.g., 429, 500, 502).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_codes: Option<Vec<u16>>,

    /// Whether to suppress retry error logging and events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_errors: Option<bool>,
}

/// Automatic update settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct UpdateConfig {
    /// Whether to automatically apply updates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// How often to check for updates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency: Option<UpdateFrequency>,
}

/// Per-agent configuration overriding global defaults for model selection,
/// compaction behaviour, and turn limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct AgentConfig {
    /// Compaction settings for this agent, overriding the global compaction
    /// config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction: Option<CompactionConfig>,

    /// Maximum number of agentic turns before the agent stops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<usize>,

    /// Model identifier to use for this agent (e.g. `"gpt-4o"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,

    /// Preset to apply for this agent, overriding the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset_id: Option<PresetId>,

    /// Provider identifier to use for this agent (e.g. `"openai"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
}
