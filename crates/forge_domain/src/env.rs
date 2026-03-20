use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::str::FromStr;

use derive_more::Display;
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{HttpConfig, RetryConfig};

const VERSION: &str = match option_env!("APP_VERSION") {
    Some(val) => val,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Debug, Setters, Clone, PartialEq, Serialize, Deserialize, fake::Dummy)]
#[serde(rename_all = "camelCase")]
#[setters(strip_option)]
/// Represents the environment in which the application is running.
pub struct Environment {
    /// The operating system of the environment.
    pub os: String,
    /// The process ID of the current process.
    pub pid: u32,
    /// The current working directory.
    pub cwd: PathBuf,
    /// The home directory.
    pub home: Option<PathBuf>,
    /// The shell being used.
    pub shell: String,
    /// The base path relative to which everything else stored.
    pub base_path: PathBuf,
    /// Base URL for Forge's backend APIs
    #[dummy(expr = "url::Url::parse(\"https://example.com\").unwrap()")]
    pub forge_api_url: Url,
    /// Configuration for the retry mechanism
    pub retry_config: RetryConfig,
    /// The maximum number of lines returned for FSSearch.
    pub max_search_lines: usize,
    /// Maximum bytes allowed for search results
    pub max_search_result_bytes: usize,
    /// Maximum characters for fetch content
    pub fetch_truncation_limit: usize,
    /// Maximum lines for shell output prefix
    pub stdout_max_prefix_length: usize,
    /// Maximum lines for shell output suffix
    pub stdout_max_suffix_length: usize,
    /// Maximum characters per line for shell output
    pub stdout_max_line_length: usize,
    /// Maximum characters per line for file read operations
    /// Controlled by FORGE_MAX_LINE_LENGTH environment variable.
    pub max_line_length: usize,
    /// Maximum number of lines to read from a file
    pub max_read_size: u64,
    /// Maximum number of files that can be read in a single batch operation.
    /// Controlled by FORGE_MAX_READ_BATCH_SIZE environment variable.
    pub max_file_read_batch_size: usize,
    /// Http configuration
    pub http: HttpConfig,
    /// Maximum file size in bytes for operations
    pub max_file_size: u64,
    /// Maximum image file size in bytes for binary read operations
    pub max_image_size: u64,
    /// Maximum execution time in seconds for a single tool call.
    /// Controls how long a tool can run before being terminated.
    pub tool_timeout: u64,
    /// Whether to automatically open HTML dump files in the browser.
    /// Controlled by FORGE_DUMP_AUTO_OPEN environment variable.
    pub auto_open_dump: bool,
    /// Path where debug request files should be written.
    /// Controlled by FORGE_DEBUG_REQUESTS environment variable.
    pub debug_requests: Option<PathBuf>,
    /// Custom history file path from FORGE_HISTORY_FILE environment variable.
    /// If None, uses the default history path.
    pub custom_history_path: Option<PathBuf>,
    /// Maximum number of conversations to show in list.
    /// Controlled by FORGE_MAX_CONVERSATIONS environment variable.
    pub max_conversations: usize,
    /// Maximum number of results to return from initial vector search.
    /// Controlled by FORGE_SEM_SEARCH_LIMIT environment variable.
    pub sem_search_limit: usize,
    /// Top-k parameter for relevance filtering during semantic search.
    /// Controls the number of nearest neighbors to consider.
    /// Controlled by FORGE_SEM_SEARCH_TOP_K environment variable.
    pub sem_search_top_k: usize,
    /// URL for the indexing server.
    /// Controlled by FORGE_WORKSPACE_SERVER_URL environment variable.
    #[dummy(expr = "url::Url::parse(\"http://localhost:8080\").unwrap()")]
    pub workspace_server_url: Url,
    /// Maximum number of file extensions to include in the system prompt.
    /// Controlled by FORGE_MAX_EXTENSIONS environment variable.
    pub max_extensions: usize,
    /// Format for automatically creating a dump when a task is completed.
    /// Controlled by FORGE_AUTO_DUMP environment variable.
    /// Set to "json" (or "true"/"1"/"yes") for JSON, "html" for HTML, or
    /// unset/other to disable.
    pub auto_dump: Option<AutoDumpFormat>,
    /// Maximum number of files read concurrently in parallel operations.
    /// Controlled by FORGE_PARALLEL_FILE_READS environment variable.
    /// Caps the `buffer_unordered` concurrency to avoid EMFILE errors.
    pub parallel_file_reads: usize,
    /// TTL in seconds for the model API list cache.
    /// Controlled by FORGE_MODEL_CACHE_TTL environment variable.
    pub model_cache_ttl: u64,
}

/// The output format used when auto-dumping a conversation on task completion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, fake::Dummy)]
#[serde(rename_all = "camelCase")]
pub enum AutoDumpFormat {
    /// Dump as a JSON file.
    Json,
    /// Dump as an HTML file.
    Html,
}

impl FromStr for AutoDumpFormat {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "html" => Ok(AutoDumpFormat::Html),
            "json" | "true" | "1" | "yes" => Ok(AutoDumpFormat::Json),
            _ => Err(()),
        }
    }
}

impl Environment {
    pub fn log_path(&self) -> PathBuf {
        self.base_path.join("logs")
    }

    pub fn history_path(&self) -> PathBuf {
        self.custom_history_path
            .clone()
            .unwrap_or(self.base_path.join(".forge_history"))
    }
    pub fn snapshot_path(&self) -> PathBuf {
        self.base_path.join("snapshots")
    }
    pub fn mcp_user_config(&self) -> PathBuf {
        self.base_path.join(".mcp.json")
    }

    pub fn templates(&self) -> PathBuf {
        self.base_path.join("templates")
    }
    pub fn agent_path(&self) -> PathBuf {
        self.base_path.join("agents")
    }
    pub fn agent_cwd_path(&self) -> PathBuf {
        self.cwd.join(".forge/agents")
    }

    pub fn command_path(&self) -> PathBuf {
        self.base_path.join("commands")
    }

    pub fn command_cwd_path(&self) -> PathBuf {
        self.cwd.join(".forge/commands")
    }
    pub fn permissions_path(&self) -> PathBuf {
        self.base_path.join("permissions.yaml")
    }

    pub fn mcp_local_config(&self) -> PathBuf {
        self.cwd.join(".mcp.json")
    }

    pub fn version(&self) -> String {
        VERSION.to_string()
    }

    pub fn app_config(&self) -> PathBuf {
        self.base_path.join(".config.json")
    }

    pub fn database_path(&self) -> PathBuf {
        self.base_path.join(".forge.db")
    }

    /// Returns the path to the cache directory
    pub fn cache_dir(&self) -> PathBuf {
        self.base_path.join("cache")
    }

    /// Returns the global skills directory path (~/.forge/skills)
    pub fn global_skills_path(&self) -> PathBuf {
        self.base_path.join("skills")
    }

    /// Returns the project-local skills directory path (.forge/skills)
    pub fn local_skills_path(&self) -> PathBuf {
        self.cwd.join(".forge/skills")
    }

    /// Returns the path to the credentials file where provider API keys are
    /// stored
    pub fn credentials_path(&self) -> PathBuf {
        self.base_path.join(".credentials.json")
    }

    pub fn workspace_hash(&self) -> WorkspaceHash {
        let mut hasher = DefaultHasher::default();
        self.cwd.hash(&mut hasher);

        WorkspaceHash(hasher.finish())
    }
}

#[derive(Clone, Copy, Display)]
pub struct WorkspaceHash(u64);
impl WorkspaceHash {
    pub fn new(id: u64) -> Self {
        WorkspaceHash(id)
    }

    pub fn id(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use fake::{Fake, Faker};
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_agent_cwd_path() {
        let fixture: Environment = Faker.fake();
        let fixture = fixture.cwd(PathBuf::from("/current/working/dir"));

        let actual = fixture.agent_cwd_path();
        let expected = PathBuf::from("/current/working/dir/.forge/agents");

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_agent_cwd_path_independent_from_agent_path() {
        let fixture: Environment = Faker.fake();
        let fixture = fixture
            .cwd(PathBuf::from("/different/current/dir"))
            .base_path(PathBuf::from("/completely/different/base"));

        let agent_path = fixture.agent_path();
        let agent_cwd_path = fixture.agent_cwd_path();
        let expected_agent_path = PathBuf::from("/completely/different/base/agents");
        let expected_agent_cwd_path = PathBuf::from("/different/current/dir/.forge/agents");

        // Verify that agent_path uses base_path
        assert_eq!(agent_path, expected_agent_path);

        // Verify that agent_cwd_path is independent and always relative to CWD
        assert_eq!(agent_cwd_path, expected_agent_cwd_path);

        // Verify they are different paths
        assert_ne!(agent_path, agent_cwd_path);
    }

    #[test]
    fn test_global_skills_path() {
        let fixture: Environment = Faker.fake();
        let fixture = fixture.base_path(PathBuf::from("/home/user/.forge"));

        let actual = fixture.global_skills_path();
        let expected = PathBuf::from("/home/user/.forge/skills");

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_local_skills_path() {
        let fixture: Environment = Faker.fake();
        let fixture = fixture.cwd(PathBuf::from("/projects/my-app"));

        let actual = fixture.local_skills_path();
        let expected = PathBuf::from("/projects/my-app/.forge/skills");

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_skills_paths_independent() {
        let fixture: Environment = Faker.fake();
        let fixture = fixture
            .cwd(PathBuf::from("/projects/my-app"))
            .base_path(PathBuf::from("/home/user/.forge"));

        let global_path = fixture.global_skills_path();
        let local_path = fixture.local_skills_path();

        let expected_global = PathBuf::from("/home/user/.forge/skills");
        let expected_local = PathBuf::from("/projects/my-app/.forge/skills");

        // Verify global path uses base_path
        assert_eq!(global_path, expected_global);

        // Verify local path uses cwd
        assert_eq!(local_path, expected_local);

        // Verify they are different paths
        assert_ne!(global_path, local_path);
    }
}

#[test]
fn test_command_path() {
    let fixture = Environment {
        os: "linux".to_string(),
        pid: 1234,
        cwd: PathBuf::from("/current/working/dir"),
        home: Some(PathBuf::from("/home/user")),
        shell: "zsh".to_string(),
        base_path: PathBuf::from("/home/user/.forge"),
        forge_api_url: "https://api.example.com".parse().unwrap(),
        retry_config: RetryConfig::default(),
        max_search_lines: 1000,
        max_search_result_bytes: 10240,
        fetch_truncation_limit: 50000,
        stdout_max_prefix_length: 100,
        stdout_max_suffix_length: 100,
        stdout_max_line_length: 500,
        max_line_length: 2000,
        max_read_size: 2000,
        max_file_read_batch_size: 50,
        http: HttpConfig::default(),
        max_file_size: 104857600,
        tool_timeout: 300,
        auto_open_dump: false,
        debug_requests: None,
        custom_history_path: None,
        max_conversations: 100,
        sem_search_limit: 100,
        sem_search_top_k: 10,
        max_image_size: 262144,
        workspace_server_url: "http://localhost:8080".parse().unwrap(),
        max_extensions: 15,
        auto_dump: None,
        parallel_file_reads: 64,
        model_cache_ttl: 604_800,
    };

    let actual = fixture.command_path();
    let expected = PathBuf::from("/home/user/.forge/commands");

    assert_eq!(actual, expected);
}

#[test]
fn test_command_cwd_path() {
    let fixture = Environment {
        os: "linux".to_string(),
        pid: 1234,
        cwd: PathBuf::from("/current/working/dir"),
        home: Some(PathBuf::from("/home/user")),
        shell: "zsh".to_string(),
        base_path: PathBuf::from("/home/user/.forge"),
        forge_api_url: "https://api.example.com".parse().unwrap(),
        retry_config: RetryConfig::default(),
        max_search_lines: 1000,
        max_search_result_bytes: 10240,
        fetch_truncation_limit: 50000,
        stdout_max_prefix_length: 100,
        stdout_max_suffix_length: 100,
        stdout_max_line_length: 500,
        max_line_length: 2000,
        max_read_size: 2000,
        max_file_read_batch_size: 50,
        http: HttpConfig::default(),
        max_file_size: 104857600,
        tool_timeout: 300,
        auto_open_dump: false,
        debug_requests: None,
        custom_history_path: None,
        max_conversations: 100,
        sem_search_limit: 100,
        sem_search_top_k: 10,
        max_image_size: 262144,
        workspace_server_url: "http://localhost:8080".parse().unwrap(),
        max_extensions: 15,
        auto_dump: None,
        parallel_file_reads: 64,
        model_cache_ttl: 604_800,
    };

    let actual = fixture.command_cwd_path();
    let expected = PathBuf::from("/current/working/dir/.forge/commands");

    assert_eq!(actual, expected);
}

#[test]
fn test_command_cwd_path_independent_from_command_path() {
    let fixture = Environment {
        os: "linux".to_string(),
        pid: 1234,
        cwd: PathBuf::from("/different/current/dir"),
        home: Some(PathBuf::from("/different/home")),
        shell: "bash".to_string(),
        base_path: PathBuf::from("/completely/different/base"),
        forge_api_url: "https://api.example.com".parse().unwrap(),
        retry_config: RetryConfig::default(),
        max_search_lines: 1000,
        max_search_result_bytes: 10240,
        fetch_truncation_limit: 50000,
        stdout_max_prefix_length: 100,
        stdout_max_suffix_length: 100,
        stdout_max_line_length: 500,
        max_line_length: 2000,
        max_read_size: 2000,
        max_file_read_batch_size: 50,
        http: HttpConfig::default(),
        max_file_size: 104857600,
        tool_timeout: 300,
        auto_open_dump: false,
        debug_requests: None,
        custom_history_path: None,
        max_conversations: 100,
        sem_search_limit: 100,
        sem_search_top_k: 10,
        max_image_size: 262144,
        workspace_server_url: "http://localhost:8080".parse().unwrap(),
        max_extensions: 15,
        auto_dump: None,
        parallel_file_reads: 64,
        model_cache_ttl: 604_800,
    };

    let command_path = fixture.command_path();
    let command_cwd_path = fixture.command_cwd_path();
    let expected_command_path = PathBuf::from("/completely/different/base/commands");
    let expected_command_cwd_path = PathBuf::from("/different/current/dir/.forge/commands");

    // Verify that command_path uses base_path
    assert_eq!(command_path, expected_command_path);

    // Verify that command_cwd_path is independent and always relative to CWD
    assert_eq!(command_cwd_path, expected_command_cwd_path);

    // Verify they are different paths
    assert_ne!(command_path, command_cwd_path);
}
