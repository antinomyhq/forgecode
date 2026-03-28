//! Registry of all user-facing `FORGE_*` environment variables.
//!
//! Each entry describes one configurable knob, its environment variable name,
//! the corresponding TOML key in `.forge.toml`, a default value, and a
//! human-readable description. The registry is used by `forge config get env`
//! and `forge config set env` to provide discoverability.

/// Describes a single configurable environment variable.
#[derive(Debug, Clone)]
pub struct EnvVarEntry {
    /// The environment variable name (e.g., `FORGE_TOOL_TIMEOUT_SECS`).
    pub env_name: &'static str,
    /// The TOML key path in `.forge.toml` (e.g., `tool_timeout_secs`).
    pub toml_key: &'static str,
    /// Default value as a string for display purposes.
    pub default: &'static str,
    /// Short description of what this variable controls.
    pub description: &'static str,
}

/// Returns the full catalogue of known `FORGE_*` environment variables.
///
/// The list is ordered by category: general limits, retry, HTTP, compaction,
/// and tuning parameters.
pub fn env_var_registry() -> &'static [EnvVarEntry] {
    static REGISTRY: &[EnvVarEntry] = &[
        // --- General limits ---
        EnvVarEntry {
            env_name: "FORGE_TOOL_TIMEOUT_SECS",
            toml_key: "tool_timeout_secs",
            default: "300",
            description: "Max seconds a tool call is allowed to run",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_SEARCH_LINES",
            toml_key: "max_search_lines",
            default: "1000",
            description: "Max lines returned by file search",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_SEARCH_RESULT_BYTES",
            toml_key: "max_search_result_bytes",
            default: "10240",
            description: "Max bytes returned by file search",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_FETCH_CHARS",
            toml_key: "max_fetch_chars",
            default: "50000",
            description: "Max characters for fetch content",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_STDOUT_PREFIX_LINES",
            toml_key: "max_stdout_prefix_lines",
            default: "100",
            description: "Max lines for shell output prefix",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_STDOUT_SUFFIX_LINES",
            toml_key: "max_stdout_suffix_lines",
            default: "100",
            description: "Max lines for shell output suffix",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_STDOUT_LINE_CHARS",
            toml_key: "max_stdout_line_chars",
            default: "500",
            description: "Max characters per line for shell output",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_LINE_CHARS",
            toml_key: "max_line_chars",
            default: "2000",
            description: "Max characters per line for file reads",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_READ_LINES",
            toml_key: "max_read_lines",
            default: "2000",
            description: "Max lines to read from a file",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_FILE_READ_BATCH_SIZE",
            toml_key: "max_file_read_batch_size",
            default: "50",
            description: "Max files read in a single batch operation",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_FILE_SIZE_BYTES",
            toml_key: "max_file_size_bytes",
            default: "104857600",
            description: "Max file size in bytes (100 MiB)",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_IMAGE_SIZE_BYTES",
            toml_key: "max_image_size_bytes",
            default: "262144",
            description: "Max image size in bytes for binary reads",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_PARALLEL_FILE_READS",
            toml_key: "max_parallel_file_reads",
            default: "64",
            description: "Max files read concurrently in parallel",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_CONVERSATIONS",
            toml_key: "max_conversations",
            default: "100",
            description: "Max conversations shown in list",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_EXTENSIONS",
            toml_key: "max_extensions",
            default: "15",
            description: "Max file extensions in system prompt",
        },
        EnvVarEntry {
            env_name: "FORGE_MODEL_CACHE_TTL_SECS",
            toml_key: "model_cache_ttl_secs",
            default: "604800",
            description: "Model list cache TTL in seconds (1 week)",
        },
        EnvVarEntry {
            env_name: "FORGE_AUTO_OPEN_DUMP",
            toml_key: "auto_open_dump",
            default: "false",
            description: "Open dump file in browser automatically",
        },
        EnvVarEntry {
            env_name: "FORGE_AUTO_DUMP",
            toml_key: "auto_dump",
            default: "",
            description: "Auto-dump on task completion: json | html",
        },
        // --- Semantic search ---
        EnvVarEntry {
            env_name: "FORGE_MAX_SEM_SEARCH_RESULTS",
            toml_key: "max_sem_search_results",
            default: "100",
            description: "Max results from vector search",
        },
        EnvVarEntry {
            env_name: "FORGE_SEM_SEARCH_TOP_K",
            toml_key: "sem_search_top_k",
            default: "10",
            description: "Top-k for semantic search filtering",
        },
        // --- Retry ---
        EnvVarEntry {
            env_name: "FORGE_RETRY__INITIAL_BACKOFF_MS",
            toml_key: "retry.initial_backoff_ms",
            default: "200",
            description: "Initial retry backoff in milliseconds",
        },
        EnvVarEntry {
            env_name: "FORGE_RETRY__MIN_DELAY_MS",
            toml_key: "retry.min_delay_ms",
            default: "1000",
            description: "Min delay between retries in milliseconds",
        },
        EnvVarEntry {
            env_name: "FORGE_RETRY__BACKOFF_FACTOR",
            toml_key: "retry.backoff_factor",
            default: "2",
            description: "Exponential backoff multiplier",
        },
        EnvVarEntry {
            env_name: "FORGE_RETRY__MAX_ATTEMPTS",
            toml_key: "retry.max_attempts",
            default: "8",
            description: "Max retry attempts for failed requests",
        },
        EnvVarEntry {
            env_name: "FORGE_RETRY__STATUS_CODES",
            toml_key: "retry.status_codes",
            default: "429,500,502,503,504,408,522,520,529",
            description: "HTTP status codes that trigger retries",
        },
        EnvVarEntry {
            env_name: "FORGE_RETRY__SUPPRESS_ERRORS",
            toml_key: "retry.suppress_errors",
            default: "false",
            description: "Suppress error output during retries",
        },
        // --- HTTP ---
        EnvVarEntry {
            env_name: "FORGE_HTTP__CONNECT_TIMEOUT_SECS",
            toml_key: "http.connect_timeout_secs",
            default: "30",
            description: "HTTP connection timeout in seconds",
        },
        EnvVarEntry {
            env_name: "FORGE_HTTP__READ_TIMEOUT_SECS",
            toml_key: "http.read_timeout_secs",
            default: "900",
            description: "HTTP read timeout in seconds",
        },
        EnvVarEntry {
            env_name: "FORGE_HTTP__POOL_IDLE_TIMEOUT_SECS",
            toml_key: "http.pool_idle_timeout_secs",
            default: "90",
            description: "HTTP pool idle timeout in seconds",
        },
        EnvVarEntry {
            env_name: "FORGE_HTTP__POOL_MAX_IDLE_PER_HOST",
            toml_key: "http.pool_max_idle_per_host",
            default: "5",
            description: "Max idle HTTP connections per host",
        },
        EnvVarEntry {
            env_name: "FORGE_HTTP__MAX_REDIRECTS",
            toml_key: "http.max_redirects",
            default: "10",
            description: "Max HTTP redirects followed",
        },
        EnvVarEntry {
            env_name: "FORGE_HTTP__HICKORY",
            toml_key: "http.hickory",
            default: "false",
            description: "Use Hickory DNS resolver instead of system",
        },
        EnvVarEntry {
            env_name: "FORGE_HTTP__TLS_BACKEND",
            toml_key: "http.tls_backend",
            default: "default",
            description: "TLS backend: default | rustls",
        },
        EnvVarEntry {
            env_name: "FORGE_HTTP__ADAPTIVE_WINDOW",
            toml_key: "http.adaptive_window",
            default: "true",
            description: "Enable HTTP/2 adaptive flow-control window",
        },
        EnvVarEntry {
            env_name: "FORGE_HTTP__KEEP_ALIVE_INTERVAL_SECS",
            toml_key: "http.keep_alive_interval_secs",
            default: "60",
            description: "TCP keep-alive probe interval in seconds",
        },
        EnvVarEntry {
            env_name: "FORGE_HTTP__KEEP_ALIVE_TIMEOUT_SECS",
            toml_key: "http.keep_alive_timeout_secs",
            default: "10",
            description: "TCP keep-alive timeout in seconds",
        },
        EnvVarEntry {
            env_name: "FORGE_HTTP__KEEP_ALIVE_WHILE_IDLE",
            toml_key: "http.keep_alive_while_idle",
            default: "true",
            description: "Send keep-alive probes while idle",
        },
        EnvVarEntry {
            env_name: "FORGE_HTTP__ACCEPT_INVALID_CERTS",
            toml_key: "http.accept_invalid_certs",
            default: "false",
            description: "Skip TLS certificate validation (insecure)",
        },
        // --- Tuning parameters ---
        EnvVarEntry {
            env_name: "FORGE_TEMPERATURE",
            toml_key: "temperature",
            default: "0.8",
            description: "Output randomness (0.0-2.0)",
        },
        EnvVarEntry {
            env_name: "FORGE_TOP_P",
            toml_key: "top_p",
            default: "0.8",
            description: "Nucleus sampling threshold (0.0-1.0)",
        },
        EnvVarEntry {
            env_name: "FORGE_TOP_K",
            toml_key: "top_k",
            default: "30",
            description: "Top-k vocabulary cutoff (1-1000)",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_TOKENS",
            toml_key: "max_tokens",
            default: "20480",
            description: "Max tokens per response (1-100000)",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_TOOL_FAILURE_PER_TURN",
            toml_key: "max_tool_failure_per_turn",
            default: "3",
            description: "Max tool failures before forcing completion",
        },
        EnvVarEntry {
            env_name: "FORGE_MAX_REQUESTS_PER_TURN",
            toml_key: "max_requests_per_turn",
            default: "100",
            description: "Max requests in a single turn",
        },
    ];

    REGISTRY
}

/// Looks up a single entry by its environment variable name
/// (case-insensitive).
pub fn find_env_var(name: &str) -> Option<&'static EnvVarEntry> {
    let upper = name.to_uppercase();
    env_var_registry().iter().find(|e| e.env_name == upper)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_registry_not_empty() {
        let registry = env_var_registry();
        assert!(registry.len() > 30, "registry should have at least 30 entries");
    }

    #[test]
    fn test_find_env_var_case_insensitive() {
        let actual = find_env_var("forge_tool_timeout_secs");
        assert!(actual.is_some());
        assert_eq!(actual.unwrap().toml_key, "tool_timeout_secs");
    }

    #[test]
    fn test_find_env_var_not_found() {
        let actual = find_env_var("FORGE_NONEXISTENT_KEY");
        assert!(actual.is_none());
    }

    #[test]
    fn test_all_entries_have_forge_prefix() {
        for entry in env_var_registry() {
            assert!(
                entry.env_name.starts_with("FORGE_"),
                "env_name '{}' should start with FORGE_",
                entry.env_name
            );
        }
    }
}
