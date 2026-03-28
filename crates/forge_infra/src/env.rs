use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use forge_app::EnvironmentInfra;
use forge_config::{ConfigReader, ForgeConfig, ModelConfig};
use forge_domain::{
    AutoDumpFormat, Compact, ConfigOperation, Environment, HttpConfig, MaxTokens, ModelId,
    RetryConfig, SessionConfig, Temperature, TlsBackend, TlsVersion, TopK, TopP, Update,
    UpdateFrequency,
};
use reqwest::Url;
use tracing::{debug, error};

/// Converts a [`ModelConfig`] into a domain-level [`SessionConfig`].
fn to_session_config(mc: &ModelConfig) -> SessionConfig {
    SessionConfig {
        provider_id: mc.provider_id.clone(),
        model_id: mc.model_id.clone(),
    }
}

/// Converts a [`forge_config::TlsVersion`] into a [`forge_domain::TlsVersion`].
fn to_tls_version(v: forge_config::TlsVersion) -> TlsVersion {
    match v {
        forge_config::TlsVersion::V1_0 => TlsVersion::V1_0,
        forge_config::TlsVersion::V1_1 => TlsVersion::V1_1,
        forge_config::TlsVersion::V1_2 => TlsVersion::V1_2,
        forge_config::TlsVersion::V1_3 => TlsVersion::V1_3,
    }
}

/// Converts a [`forge_config::TlsBackend`] into a [`forge_domain::TlsBackend`].
fn to_tls_backend(b: forge_config::TlsBackend) -> TlsBackend {
    match b {
        forge_config::TlsBackend::Default => TlsBackend::Default,
        forge_config::TlsBackend::Rustls => TlsBackend::Rustls,
    }
}

/// Converts a [`forge_config::HttpConfig`] into a [`forge_domain::HttpConfig`].
fn to_http_config(h: forge_config::HttpConfig) -> HttpConfig {
    HttpConfig {
        connect_timeout: h.connect_timeout_secs,
        read_timeout: h.read_timeout_secs,
        pool_idle_timeout: h.pool_idle_timeout_secs,
        pool_max_idle_per_host: h.pool_max_idle_per_host,
        max_redirects: h.max_redirects,
        hickory: h.hickory,
        tls_backend: to_tls_backend(h.tls_backend),
        min_tls_version: h.min_tls_version.map(to_tls_version),
        max_tls_version: h.max_tls_version.map(to_tls_version),
        adaptive_window: h.adaptive_window,
        keep_alive_interval: h.keep_alive_interval_secs,
        keep_alive_timeout: h.keep_alive_timeout_secs,
        keep_alive_while_idle: h.keep_alive_while_idle,
        accept_invalid_certs: h.accept_invalid_certs,
        root_cert_paths: h.root_cert_paths,
    }
}

/// Converts a [`forge_config::RetryConfig`] into a
/// [`forge_domain::RetryConfig`].
fn to_retry_config(r: forge_config::RetryConfig) -> RetryConfig {
    RetryConfig {
        initial_backoff_ms: r.initial_backoff_ms,
        min_delay_ms: r.min_delay_ms,
        backoff_factor: r.backoff_factor,
        max_retry_attempts: r.max_attempts,
        retry_status_codes: r.status_codes,
        max_delay: r.max_delay_secs,
        suppress_retry_errors: r.suppress_errors,
    }
}

/// Converts a [`forge_config::AutoDumpFormat`] into a
/// [`forge_domain::AutoDumpFormat`].
fn to_auto_dump_format(f: forge_config::AutoDumpFormat) -> AutoDumpFormat {
    match f {
        forge_config::AutoDumpFormat::Json => AutoDumpFormat::Json,
        forge_config::AutoDumpFormat::Html => AutoDumpFormat::Html,
    }
}

/// Converts a [`forge_config::UpdateFrequency`] into a
/// [`forge_domain::UpdateFrequency`].
fn to_update_frequency(f: forge_config::UpdateFrequency) -> UpdateFrequency {
    match f {
        forge_config::UpdateFrequency::Daily => UpdateFrequency::Daily,
        forge_config::UpdateFrequency::Weekly => UpdateFrequency::Weekly,
        forge_config::UpdateFrequency::Always => UpdateFrequency::Always,
    }
}

/// Converts a [`forge_config::Update`] into a [`forge_domain::Update`].
fn to_update(u: forge_config::Update) -> Update {
    Update {
        frequency: u.frequency.map(to_update_frequency),
        auto_update: u.auto_update,
    }
}

/// Converts a [`forge_config::Compact`] into a [`forge_domain::Compact`].
fn to_compact(c: forge_config::Compact) -> Compact {
    Compact {
        retention_window: c.retention_window,
        eviction_window: c.eviction_window,
        max_tokens: c.max_tokens,
        token_threshold: c.token_threshold,
        turn_threshold: c.turn_threshold,
        message_threshold: c.message_threshold,
        model: c.model.map(ModelId::new),
        on_turn_end: c.on_turn_end,
    }
}

/// Builds a [`forge_domain::Environment`] entirely from a [`ForgeConfig`] and
/// runtime context (`restricted`, `cwd`), mapping every config field to its
/// corresponding environment field.
fn to_environment(fc: ForgeConfig, cwd: PathBuf) -> Environment {
    Environment {
        // --- Infrastructure-derived fields ---
        os: std::env::consts::OS.to_string(),
        pid: std::process::id(),
        cwd,
        home: dirs::home_dir(),
        shell: if cfg!(target_os = "windows") {
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
        },
        base_path: ConfigReader::base_path(),

        // --- ForgeConfig-mapped fields ---
        retry_config: fc.retry.map(to_retry_config).unwrap_or_default(),
        max_search_lines: fc.max_search_lines,
        max_search_result_bytes: fc.max_search_result_bytes,
        fetch_truncation_limit: fc.max_fetch_chars,
        stdout_max_prefix_length: fc.max_stdout_prefix_lines,
        stdout_max_suffix_length: fc.max_stdout_suffix_lines,
        stdout_max_line_length: fc.max_stdout_line_chars,
        max_line_length: fc.max_line_chars,
        max_read_size: fc.max_read_lines,
        max_file_read_batch_size: fc.max_file_read_batch_size,
        http: fc.http.map(to_http_config).unwrap_or_default(),
        max_file_size: fc.max_file_size_bytes,
        max_image_size: fc.max_image_size_bytes,
        tool_timeout: fc.tool_timeout_secs,
        auto_open_dump: fc.auto_open_dump,
        debug_requests: fc.debug_requests,
        custom_history_path: fc.custom_history_path,
        max_conversations: fc.max_conversations,
        sem_search_limit: fc.max_sem_search_results,
        sem_search_top_k: fc.sem_search_top_k,
        service_url: Url::parse(fc.services_url.as_str())
            .unwrap_or_else(|_| Url::parse("https://api.forgecode.dev").unwrap()),
        max_extensions: fc.max_extensions,
        auto_dump: fc.auto_dump.map(to_auto_dump_format),
        parallel_file_reads: fc.max_parallel_file_reads,
        model_cache_ttl: fc.model_cache_ttl_secs,
        session: fc.session.as_ref().map(to_session_config),
        commit: fc.commit.as_ref().map(to_session_config),
        suggest: fc.suggest.as_ref().map(to_session_config),
        is_restricted: fc.restricted,
        tool_supported: fc.tool_supported,
        temperature: fc.temperature.and_then(|v| Temperature::new(v).ok()),
        top_p: fc.top_p.and_then(|v| TopP::new(v).ok()),
        top_k: fc.top_k.and_then(|v| TopK::new(v).ok()),
        max_tokens: fc.max_tokens.and_then(|v| MaxTokens::new(v).ok()),
        max_tool_failure_per_turn: fc.max_tool_failure_per_turn,
        max_requests_per_turn: fc.max_requests_per_turn,
        compact: fc.compact.map(to_compact),
        updates: fc.updates.map(to_update),
    }
}

/// Converts a [`forge_domain::RetryConfig`] back into a
/// [`forge_config::RetryConfig`].
fn from_retry_config(r: &RetryConfig) -> forge_config::RetryConfig {
    forge_config::RetryConfig {
        initial_backoff_ms: r.initial_backoff_ms,
        min_delay_ms: r.min_delay_ms,
        backoff_factor: r.backoff_factor,
        max_attempts: r.max_retry_attempts,
        status_codes: r.retry_status_codes.clone(),
        max_delay_secs: r.max_delay,
        suppress_errors: r.suppress_retry_errors,
    }
}

/// Converts a [`forge_domain::HttpConfig`] back into a
/// [`forge_config::HttpConfig`].
fn from_http_config(h: &HttpConfig) -> forge_config::HttpConfig {
    forge_config::HttpConfig {
        connect_timeout_secs: h.connect_timeout,
        read_timeout_secs: h.read_timeout,
        pool_idle_timeout_secs: h.pool_idle_timeout,
        pool_max_idle_per_host: h.pool_max_idle_per_host,
        max_redirects: h.max_redirects,
        hickory: h.hickory,
        tls_backend: from_tls_backend(h.tls_backend.clone()),
        min_tls_version: h.min_tls_version.clone().map(from_tls_version),
        max_tls_version: h.max_tls_version.clone().map(from_tls_version),
        adaptive_window: h.adaptive_window,
        keep_alive_interval_secs: h.keep_alive_interval,
        keep_alive_timeout_secs: h.keep_alive_timeout,
        keep_alive_while_idle: h.keep_alive_while_idle,
        accept_invalid_certs: h.accept_invalid_certs,
        root_cert_paths: h.root_cert_paths.clone(),
    }
}

/// Converts a [`forge_domain::TlsVersion`] back into a
/// [`forge_config::TlsVersion`].
fn from_tls_version(v: TlsVersion) -> forge_config::TlsVersion {
    match v {
        TlsVersion::V1_0 => forge_config::TlsVersion::V1_0,
        TlsVersion::V1_1 => forge_config::TlsVersion::V1_1,
        TlsVersion::V1_2 => forge_config::TlsVersion::V1_2,
        TlsVersion::V1_3 => forge_config::TlsVersion::V1_3,
    }
}

/// Converts a [`forge_domain::TlsBackend`] back into a
/// [`forge_config::TlsBackend`].
fn from_tls_backend(b: TlsBackend) -> forge_config::TlsBackend {
    match b {
        TlsBackend::Default => forge_config::TlsBackend::Default,
        TlsBackend::Rustls => forge_config::TlsBackend::Rustls,
    }
}

/// Converts a [`forge_domain::AutoDumpFormat`] back into a
/// [`forge_config::AutoDumpFormat`].
fn from_auto_dump_format(f: &AutoDumpFormat) -> forge_config::AutoDumpFormat {
    match f {
        AutoDumpFormat::Json => forge_config::AutoDumpFormat::Json,
        AutoDumpFormat::Html => forge_config::AutoDumpFormat::Html,
    }
}

/// Converts a [`forge_domain::UpdateFrequency`] back into a
/// [`forge_config::UpdateFrequency`].
fn from_update_frequency(f: UpdateFrequency) -> forge_config::UpdateFrequency {
    match f {
        UpdateFrequency::Daily => forge_config::UpdateFrequency::Daily,
        UpdateFrequency::Weekly => forge_config::UpdateFrequency::Weekly,
        UpdateFrequency::Always => forge_config::UpdateFrequency::Always,
    }
}

/// Converts a [`forge_domain::Update`] back into a [`forge_config::Update`].
fn from_update(u: &Update) -> forge_config::Update {
    forge_config::Update {
        frequency: u.frequency.clone().map(from_update_frequency),
        auto_update: u.auto_update,
    }
}

/// Converts a [`forge_domain::Compact`] back into a [`forge_config::Compact`].
fn from_compact(c: &Compact) -> forge_config::Compact {
    forge_config::Compact {
        retention_window: c.retention_window,
        eviction_window: c.eviction_window,
        max_tokens: c.max_tokens,
        token_threshold: c.token_threshold,
        turn_threshold: c.turn_threshold,
        message_threshold: c.message_threshold,
        model: c.model.as_ref().map(|m| m.to_string()),
        on_turn_end: c.on_turn_end,
    }
}

/// Converts an [`Environment`] back into a [`ForgeConfig`] suitable for
/// persisting.
///
/// Builds a fresh [`ForgeConfig`] from [`ForgeConfig::default()`] and maps
/// every field that originated from [`ForgeConfig`] back from the
/// [`Environment`], preserving the round-trip identity. Fields that only exist
/// in [`ForgeConfig`] but are not represented in [`Environment`] (e.g.
/// `updates`, `temperature`, `compact`) remain at their default values.
fn to_forge_config(env: &Environment) -> ForgeConfig {
    let mut fc = ForgeConfig::default();

    // --- Fields mapped through Environment ---
    let default_retry = RetryConfig::default();
    fc.retry = if env.retry_config == default_retry {
        None
    } else {
        Some(from_retry_config(&env.retry_config))
    };
    fc.max_search_lines = env.max_search_lines;
    fc.max_search_result_bytes = env.max_search_result_bytes;
    fc.max_fetch_chars = env.fetch_truncation_limit;
    fc.max_stdout_prefix_lines = env.stdout_max_prefix_length;
    fc.max_stdout_suffix_lines = env.stdout_max_suffix_length;
    fc.max_stdout_line_chars = env.stdout_max_line_length;
    fc.max_line_chars = env.max_line_length;
    fc.max_read_lines = env.max_read_size;
    fc.max_file_read_batch_size = env.max_file_read_batch_size;
    let default_http = HttpConfig::default();
    fc.http = if env.http == default_http {
        None
    } else {
        Some(from_http_config(&env.http))
    };
    fc.max_file_size_bytes = env.max_file_size;
    fc.max_image_size_bytes = env.max_image_size;
    fc.tool_timeout_secs = env.tool_timeout;
    fc.auto_open_dump = env.auto_open_dump;
    fc.debug_requests = env.debug_requests.clone();
    fc.custom_history_path = env.custom_history_path.clone();
    fc.max_conversations = env.max_conversations;
    fc.max_sem_search_results = env.sem_search_limit;
    fc.sem_search_top_k = env.sem_search_top_k;
    fc.services_url = env.service_url.to_string();
    fc.max_extensions = env.max_extensions;
    fc.auto_dump = env.auto_dump.as_ref().map(from_auto_dump_format);
    fc.max_parallel_file_reads = env.parallel_file_reads;
    fc.model_cache_ttl_secs = env.model_cache_ttl;
    fc.restricted = env.is_restricted;
    fc.tool_supported = env.tool_supported;

    // --- Workflow fields ---
    fc.temperature = env.temperature.map(|t| t.value());
    fc.top_p = env.top_p.map(|t| t.value());
    fc.top_k = env.top_k.map(|t| t.value());
    fc.max_tokens = env.max_tokens.map(|t| t.value());
    fc.max_tool_failure_per_turn = env.max_tool_failure_per_turn;
    fc.max_requests_per_turn = env.max_requests_per_turn;
    fc.compact = env.compact.as_ref().map(from_compact);
    fc.updates = env.updates.as_ref().map(from_update);

    // --- Session configs ---
    fc.session = env.session.as_ref().map(|sc| ModelConfig {
        provider_id: sc.provider_id.clone(),
        model_id: sc.model_id.clone(),
    });
    fc.commit = env.commit.as_ref().map(|sc| ModelConfig {
        provider_id: sc.provider_id.clone(),
        model_id: sc.model_id.clone(),
    });
    fc.suggest = env.suggest.as_ref().map(|sc| ModelConfig {
        provider_id: sc.provider_id.clone(),
        model_id: sc.model_id.clone(),
    });
    fc
}

/// Infrastructure implementation for managing application configuration with
/// caching support.
///
/// Uses [`ForgeConfig::read`] and [`ForgeConfig::write`] for all file I/O and
/// maintains an in-memory cache to reduce disk access. Also handles
/// environment variable discovery via `.env` files and OS APIs.
pub struct ForgeEnvironmentInfra {
    cwd: PathBuf,
    cache: Arc<std::sync::Mutex<Option<ForgeConfig>>>,
}

impl ForgeEnvironmentInfra {
    /// Creates a new [`ForgeConfigInfra`].
    ///
    /// # Arguments
    /// * `restricted` - If true, enables restricted mode
    /// * `cwd` - The working directory path; used to resolve `.env` files
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd, cache: Arc::new(std::sync::Mutex::new(None)) }
    }

    /// Reads [`ForgeConfig`] from disk via [`ForgeConfig::read`].
    fn read_from_disk() -> ForgeConfig {
        match ForgeConfig::read() {
            Ok(config) => {
                debug!(config = ?config, "read .forge.toml");
                config
            }
            Err(e) => {
                // NOTE: This should never-happen
                error!(error = ?e, "Failed to read config file. Using default config.");
                Default::default()
            }
        }
    }
}

impl EnvironmentInfra for ForgeEnvironmentInfra {
    fn get_env_var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn get_env_vars(&self) -> BTreeMap<String, String> {
        std::env::vars().collect()
    }

    fn get_environment(&self) -> Environment {
        let fc = {
            let mut cache = self.cache.lock().expect("cache mutex poisoned");
            if let Some(ref config) = *cache {
                config.clone()
            } else {
                let config = Self::read_from_disk();
                *cache = Some(config.clone());
                config
            }
        };

        to_environment(fc, self.cwd.clone())
    }

    async fn update_environment(&self, ops: Vec<ConfigOperation>) -> anyhow::Result<()> {
        // Load the global config
        let fc = ConfigReader::default()
            .read_defaults()
            .read_global()
            .build()?;

        debug!(config = ?fc, "loaded config for update");

        // Convert to Environment and apply each operation
        debug!(?ops, "applying app config operations");
        let mut env = to_environment(fc.clone(), self.cwd.clone());
        for op in ops {
            env.apply_op(op);
        }

        // Convert Environment back to ForgeConfig and persist
        let fc = to_forge_config(&env);
        fc.write()?;
        debug!(config = ?fc, "written .forge.toml");

        // Reset cache
        *self.cache.lock().expect("cache mutex poisoned") = None;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use forge_config::ForgeConfig;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_to_environment_default_config() {
        let fixture = ForgeConfig::default();
        let actual = to_environment(fixture, PathBuf::from("/test/cwd"));

        // Config-derived fields should all be zero/default since ForgeConfig
        // derives Default (all-zeros) without the defaults file.
        assert_eq!(actual.cwd, PathBuf::from("/test/cwd"));
        assert!(!actual.is_restricted);
        assert_eq!(actual.retry_config, RetryConfig::default());
        assert_eq!(actual.http, HttpConfig::default());
        assert!(!actual.auto_open_dump);
        assert_eq!(actual.auto_dump, None);
        assert_eq!(actual.debug_requests, None);
        assert_eq!(actual.custom_history_path, None);
        assert_eq!(actual.session, None);
        assert_eq!(actual.commit, None);
        assert_eq!(actual.suggest, None);
    }

    #[test]
    fn test_to_environment_restricted_mode() {
        let fixture = ForgeConfig::default().restricted(true);
        let actual = to_environment(fixture, PathBuf::from("/tmp"));

        assert!(actual.is_restricted);
    }

    #[test]
    fn test_forge_config_environment_identity() {
        // Property test: for ANY randomly generated ForgeConfig `fc`, the
        // config-mapped fields of the Environment produced by
        // `to_environment(fc)` must survive a full round-trip through
        // `to_forge_config` and back unchanged.
        //
        //   fc  -->  env  -->  fc'  -->  env'
        //            ^                    ^
        //            |--- config fields --|  must be equal
        use fake::{Fake, Faker};

        let cwd = PathBuf::from("/identity/test");

        for _ in 0..100 {
            let fixture: ForgeConfig = Faker.fake();

            // fc -> env -> fc' -> env'
            let env = to_environment(fixture, cwd.clone());
            let fc_prime = to_forge_config(&env);
            let env_prime = to_environment(fc_prime, cwd.clone());

            // Infrastructure-derived fields (os, pid, home, shell, base_path)
            // are re-derived from the runtime, so they are equal by
            // construction. Config-mapped fields must satisfy the identity:
            // env == env'
            assert_eq!(env, env_prime);
        }
    }
}
