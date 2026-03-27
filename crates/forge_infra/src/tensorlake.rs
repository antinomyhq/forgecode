use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use async_trait::async_trait;
use forge_app::CommandInfra;
use forge_domain::CommandOutput;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

const TENSORLAKE_API_BASE: &str = "https://api.tensorlake.ai";

/// Configuration for a Tensorlake sandbox session.
#[derive(Debug, Clone)]
pub struct TensorlakeConfig {
    /// Tensorlake API key used for all requests.
    pub api_key: String,
    /// Number of vCPUs to allocate for the sandbox (default: 2.0).
    pub cpus: f64,
    /// Memory in megabytes to allocate for the sandbox (default: 4096).
    pub memory_mb: u64,
    /// Inactivity timeout in seconds before the sandbox auto-suspends (default:
    /// 3600).
    pub timeout_secs: u64,
    /// Base URL for the Tensorlake API (default: `https://api.tensorlake.ai`).
    /// Overridable in tests to point at a local mock server.
    pub base_url: String,
}

impl TensorlakeConfig {
    /// Creates a new `TensorlakeConfig` with the given API key and sensible
    /// defaults.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            cpus: 2.0,
            memory_mb: 4096,
            timeout_secs: 3600,
            base_url: TENSORLAKE_API_BASE.to_string(),
        }
    }
}

/// Response returned by the Tensorlake sandboxes create endpoint.
#[derive(Debug, Deserialize)]
struct CreateSandboxResponse {
    sandbox_id: String,
}

/// Response returned by the per-sandbox process execution endpoint.
#[derive(Debug, Deserialize)]
struct StartProcessResponse {
    pid: u64,
}

/// Combined output response from the per-sandbox output endpoint.
#[derive(Debug, Deserialize)]
struct ProcessOutputResponse {
    lines: Vec<String>,
}

/// Request body for starting a process inside a sandbox.
#[derive(Debug, Serialize)]
struct StartProcessRequest<'a> {
    command: &'a str,
    args: Vec<&'a str>,
    working_dir: String,
    /// Optional environment variables to inject, as `KEY=VALUE` pairs parsed
    /// into a map. The Tensorlake API accepts `{"KEY": "VALUE"}` format.
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<HashMap<String, String>>,
}

/// Owns the sandbox lifetime and issues the DELETE on drop.
///
/// Wrapped in `Arc` inside `TensorlakeCommandExecutor` so that the sandbox is
/// terminated exactly once — when the last clone of the executor is dropped and
/// the `Arc` reference count reaches zero.
struct SandboxGuard {
    sandbox_id: Arc<Mutex<Option<String>>>,
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl Drop for SandboxGuard {
    /// Synchronously terminates the sandbox by sending a DELETE request.
    ///
    /// Uses `block_in_place` + `block_on` so the HTTP call completes before
    /// `Drop` returns, ensuring the sandbox is always cleaned up on normal
    /// exit, Ctrl-C, and panics that unwind the stack. SIGKILL remains
    /// unhandled by design; `timeout_secs` acts as the safety net there.
    fn drop(&mut self) {
        let sandbox_id = self.sandbox_id.clone();
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async move {
                    let guard = sandbox_id.lock().await;
                    if let Some(id) = guard.as_deref() {
                        let url = format!("{}/sandboxes/{}", base_url, id);
                        let _ = client.delete(&url).bearer_auth(&api_key).send().await;
                        tracing::debug!(sandbox_id = %id, "Tensorlake sandbox terminated");
                    }
                });
            });
        }
    }
}

/// Infrastructure implementation that executes shell commands inside an
/// isolated Tensorlake Firecracker microVM sandbox.
///
/// A single sandbox is created lazily on the first command execution and reused
/// for the lifetime of the `TensorlakeCommandExecutor` instance. The sandbox is
/// terminated when the last clone of the executor is dropped.
#[derive(Clone)]
pub struct TensorlakeCommandExecutor {
    config: TensorlakeConfig,
    client: reqwest::Client,
    /// Lazily initialized sandbox ID, shared across clones via `Arc<Mutex<…>>`.
    sandbox_id: Arc<Mutex<Option<String>>>,
    /// Dropping the last clone of this `Arc` triggers `SandboxGuard::drop`,
    /// which issues the sandbox DELETE exactly once.
    _guard: Arc<SandboxGuard>,
}

impl TensorlakeCommandExecutor {
    /// Creates a new executor with the provided Tensorlake configuration.
    pub fn new(config: TensorlakeConfig) -> Self {
        let client = reqwest::Client::new();
        let sandbox_id = Arc::new(Mutex::new(None));
        let guard = Arc::new(SandboxGuard {
            sandbox_id: sandbox_id.clone(),
            client: client.clone(),
            api_key: config.api_key.clone(),
            base_url: config.base_url.clone(),
        });
        Self { config, client, sandbox_id, _guard: guard }
    }

    /// Returns the sandbox ID, creating a new sandbox if one has not been
    /// provisioned yet for this session.
    async fn ensure_sandbox(&self) -> anyhow::Result<String> {
        let mut guard = self.sandbox_id.lock().await;
        if let Some(id) = guard.as_deref() {
            return Ok(id.to_string());
        }

        let id = self.create_sandbox().await?;
        tracing::info!(sandbox_id = %id, "Tensorlake sandbox created");
        *guard = Some(id.clone());
        Ok(id)
    }

    /// Provisions a new Tensorlake sandbox and returns its ID.
    async fn create_sandbox(&self) -> anyhow::Result<String> {
        let url = format!("{}/sandboxes", self.config.base_url);
        let body = serde_json::json!({
            "resources": {
                "cpus": self.config.cpus,
                "memory_mb": self.config.memory_mb,
            },
            "timeout_secs": self.config.timeout_secs,
        });

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.config.api_key)
            .json(&body)
            .send()
            .await
            .context("Failed to send create sandbox request to Tensorlake")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Tensorlake sandbox creation failed with status {status}: {text}"
            ));
        }

        let parsed: CreateSandboxResponse = response
            .json()
            .await
            .context("Failed to parse Tensorlake create sandbox response")?;

        // Poll until the sandbox is running (it starts as "pending")
        self.wait_for_running(&parsed.sandbox_id).await?;

        Ok(parsed.sandbox_id)
    }

    /// Polls the sandbox status endpoint until the sandbox reaches the
    /// "running" state.
    async fn wait_for_running(&self, sandbox_id: &str) -> anyhow::Result<()> {
        let url = format!("{}/sandboxes/{}", self.config.base_url, sandbox_id);
        for attempt in 0..60 {
            tokio::time::sleep(std::time::Duration::from_secs(if attempt == 0 {
                1
            } else {
                2
            }))
            .await;

            let resp = self
                .client
                .get(&url)
                .bearer_auth(&self.config.api_key)
                .send()
                .await
                .context("Failed to poll sandbox status")?;

            if !resp.status().is_success() {
                continue;
            }

            let body: serde_json::Value = resp
                .json()
                .await
                .context("Failed to parse sandbox status")?;

            match body.get("status").and_then(|s| s.as_str()) {
                Some("running") => return Ok(()),
                Some("terminated") => {
                    return Err(anyhow!("Tensorlake sandbox terminated unexpectedly"));
                }
                _ => continue,
            }
        }
        Err(anyhow!(
            "Timed out waiting for Tensorlake sandbox to become running"
        ))
    }

    /// Returns the per-sandbox proxy base URL for API calls.
    fn sandbox_proxy_url(&self, sandbox_id: &str) -> String {
        format!("https://{}.sandbox.tensorlake.ai/api/v1", sandbox_id)
    }
}

#[async_trait]
impl CommandInfra for TensorlakeCommandExecutor {
    /// Executes a shell command inside the Tensorlake sandbox and returns the
    /// captured output.
    async fn execute_command(
        &self,
        command: String,
        working_dir: PathBuf,
        _silent: bool,
        env_vars: Option<Vec<String>>,
    ) -> anyhow::Result<CommandOutput> {
        let sandbox_id = self.ensure_sandbox().await?;
        let proxy = self.sandbox_proxy_url(&sandbox_id);

        // The sandbox is a remote Linux microVM. Host-specific paths (e.g. macOS
        // `/Users/…`) do not exist inside the sandbox. Fall back to `/tmp` so
        // that process spawn never fails with "No such file or directory".

        let cwd = {
            let host_path = working_dir.to_string_lossy();
            // Handle both Unix and Windows paths
            if host_path.starts_with("/Users/") || host_path.starts_with("/home/") || host_path.contains(":\\") {
                "/tmp".to_string()
            } else {
                host_path.into_owned()
            }
        };


        // Parse `KEY=VALUE` strings into the dict format the Tensorlake API expects.
        let env = env_vars.map(|vars| {
            vars.into_iter()
                .filter_map(|kv| {
                    let mut parts = kv.splitn(2, '=');
                    let key = parts.next()?.to_string();
                    let value = parts.next().unwrap_or("").to_string();
                    Some((key, value))
                })
                .collect::<HashMap<String, String>>()
        });

        // Use `sh -c <command>` so pipes and redirects work correctly.
        let start_url = format!("{}/processes", proxy);
        let request = StartProcessRequest {
            command: "sh",
            args: vec!["-c", &command],
            working_dir: cwd,
            env,
        };

        tracing::info!(command = %command, sandbox_id = %sandbox_id, "Executing command in Tensorlake sandbox");

        let response = self
            .client
            .post(&start_url)
            .bearer_auth(&self.config.api_key)
            .json(&request)
            .send()
            .await
            .context("Failed to start process in Tensorlake sandbox")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Tensorlake process start failed with status {status}: {text}"
            ));
        }

        let started: StartProcessResponse = response
            .json()
            .await
            .context("Failed to parse Tensorlake process start response")?;

        let pid = started.pid;

        // Poll until the process exits.
        let exit_code = self.wait_for_process_exit(&proxy, pid).await?;

        // Collect stdout and stderr.
        let stdout = self.get_process_output(&proxy, pid, "stdout").await?;
        let stderr = self.get_process_output(&proxy, pid, "stderr").await?;

        Ok(CommandOutput { stdout, stderr, exit_code: Some(exit_code), command })
    }

    /// Interactive (raw) commands are not supported in Tensorlake sandbox mode.
    ///
    /// Raw command execution requires an attached TTY which is not available
    /// over the Tensorlake HTTP API. This method always returns an error
    /// directing the caller to use `execute_command` instead.
    async fn execute_command_raw(
        &self,
        _command: &str,
        _working_dir: PathBuf,
        _env_vars: Option<Vec<String>>,
    ) -> anyhow::Result<std::process::ExitStatus> {
        Err(anyhow!(
            "Interactive (raw) command execution is not supported in Tensorlake sandbox mode. \
             Use non-interactive commands instead."
        ))
    }
}

impl TensorlakeCommandExecutor {
    /// Polls the process status until it exits, returning the exit code.
    async fn wait_for_process_exit(&self, proxy: &str, pid: u64) -> anyhow::Result<i32> {
        let url = format!("{}/processes/{}", proxy, pid);
        for _ in 0..300 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            let resp = self
                .client
                .get(&url)
                .bearer_auth(&self.config.api_key)
                .send()
                .await
                .context("Failed to poll process status")?;

            if !resp.status().is_success() {
                let status = resp.status();
                return Err(anyhow!(
                    "Failed to poll process {pid} status: HTTP {status}"
                ));
            }

            let body: serde_json::Value = resp
                .json()
                .await
                .context("Failed to parse process status")?;

            if body.get("status").and_then(|s| s.as_str()) == Some("exited") {
                let code = body.get("exit_code").and_then(|c| c.as_i64()).unwrap_or(0) as i32;
                return Ok(code);
            }
        }
        Err(anyhow!(
            "Timed out waiting for Tensorlake process {} to exit",
            pid
        ))
    }

    /// Fetches the captured output lines (stdout or stderr) for a completed
    /// process.
    async fn get_process_output(
        &self,
        proxy: &str,
        pid: u64,
        stream: &str,
    ) -> anyhow::Result<String> {
        let url = format!("{}/processes/{}/{}", proxy, pid, stream);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.config.api_key)
            .send()
            .await
            .with_context(|| format!("Failed to fetch process {stream} for pid {pid}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(anyhow!(
                "Failed to fetch {stream} for pid {pid}: HTTP {status}"
            ));
        }

        let output: ProcessOutputResponse = resp
            .json()
            .await
            .with_context(|| format!("Failed to parse process {stream} response"))?;

        Ok(output.lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_tensorlake_config_defaults() {
        let fixture = TensorlakeConfig::new("test-api-key".to_string());

        assert_eq!(fixture.api_key, "test-api-key");
        assert_eq!(fixture.cpus, 2.0);
        assert_eq!(fixture.memory_mb, 4096);
        assert_eq!(fixture.timeout_secs, 3600);
        assert_eq!(fixture.base_url, "https://api.tensorlake.ai");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tensorlake_executor_creation() {
        let config = TensorlakeConfig::new("test-api-key".to_string());
        let executor = TensorlakeCommandExecutor::new(config.clone());

        assert_eq!(executor.config.api_key, config.api_key);
        assert_eq!(executor.config.cpus, config.cpus);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sandbox_proxy_url() {
        let config = TensorlakeConfig::new("key".to_string());
        let executor = TensorlakeCommandExecutor::new(config);
        let url = executor.sandbox_proxy_url("abc123");
        assert_eq!(url, "https://abc123.sandbox.tensorlake.ai/api/v1");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_cleanup_fires_on_last_clone_dropped() {
        let config = TensorlakeConfig::new("key".to_string());
        let executor = TensorlakeCommandExecutor::new(config);
        let clone = executor.clone();

        // Both the original and the clone share the same Arc<SandboxGuard>.
        assert_eq!(Arc::strong_count(&executor._guard), 2);

        // Dropping the clone reduces the count to 1 — cleanup not yet triggered.
        drop(clone);
        assert_eq!(Arc::strong_count(&executor._guard), 1);

        // Dropping the original reduces the count to 0 — cleanup fires here.
        // (No sandbox was provisioned so the spawned task is a no-op.)
        drop(executor);
    }

    /// Verifies that dropping the executor sends a DELETE /sandboxes/{id}
    /// request synchronously — i.e. the request completes before `drop`
    /// returns, so the mock server receives it before `assert_hits` is
    /// checked.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_sandbox_terminated_on_drop() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("DELETE", "/sandboxes/test-sandbox-id")
            .with_status(200)
            .expect(1)
            .create_async()
            .await;

        let mut config = TensorlakeConfig::new("test-key".to_string());
        config.base_url = server.url();

        let executor = TensorlakeCommandExecutor::new(config);

        // Manually plant a sandbox ID so the guard has something to DELETE.
        {
            let mut guard = executor.sandbox_id.lock().await;
            *guard = Some("test-sandbox-id".to_string());
        }

        // Drop the executor — SandboxGuard::drop must complete the DELETE before
        // returning, so the mock expectation is satisfied synchronously.
        drop(executor);

        mock.assert_async().await;
    }

    /// Verifies that a clone and the original share the guard, and the DELETE
    /// is sent exactly once — only when the last clone is dropped.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_sandbox_terminated_exactly_once_across_clones() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("DELETE", "/sandboxes/shared-sandbox")
            .with_status(200)
            .expect(1) // must fire exactly once, not twice
            .create_async()
            .await;

        let mut config = TensorlakeConfig::new("test-key".to_string());
        config.base_url = server.url();

        let executor = TensorlakeCommandExecutor::new(config);
        let clone = executor.clone();

        {
            let mut guard = executor.sandbox_id.lock().await;
            *guard = Some("shared-sandbox".to_string());
        }

        // Dropping the clone must NOT trigger the DELETE yet.
        drop(clone);
        // Dropping the original must trigger the DELETE exactly once.
        drop(executor);

        mock.assert_async().await;
    }

    #[test]
    fn test_env_vars_parsed_into_map() {
        let vars = vec![
            "FOO=bar".to_string(),
            "BAZ=qux=with=equals".to_string(),
            "EMPTY=".to_string(),
        ];

        let actual: HashMap<String, String> = vars
            .into_iter()
            .filter_map(|kv| {
                let mut parts = kv.splitn(2, '=');
                let key = parts.next()?.to_string();
                let value = parts.next().unwrap_or("").to_string();
                Some((key, value))
            })
            .collect();

        let mut expected = HashMap::new();
        expected.insert("FOO".to_string(), "bar".to_string());
        expected.insert("BAZ".to_string(), "qux=with=equals".to_string());
        expected.insert("EMPTY".to_string(), "".to_string());

        assert_eq!(actual, expected);
    }
}
