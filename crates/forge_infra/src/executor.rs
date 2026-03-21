use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use forge_app::CommandInfra;
use forge_domain::{CommandOutput, ConsoleWriter as OutputPrinterTrait, Environment};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::console::StdConsoleWriter;

/// Service for executing shell commands
#[derive(Clone, Debug)]
pub struct ForgeCommandExecutorService {
    restricted: bool,
    env: Environment,
    output_printer: Arc<StdConsoleWriter>,

    // Mutex to ensure that only one command is executed at a time
    ready: Arc<Mutex<()>>,
}

impl ForgeCommandExecutorService {
    pub fn new(restricted: bool, env: Environment, output_printer: Arc<StdConsoleWriter>) -> Self {
        Self {
            restricted,
            env,
            output_printer,
            ready: Arc::new(Mutex::new(())),
        }
    }

    fn prepare_command(
        &self,
        command_str: &str,
        working_dir: &Path,
        env_vars: Option<Vec<String>>,
    ) -> Command {
        // Create a basic command
        let is_windows = cfg!(target_os = "windows");
        let shell = if self.restricted && !is_windows {
            "rbash"
        } else {
            self.env.shell.as_str()
        };
        let mut command = Command::new(shell);

        // Core color settings for general commands
        command
            .env("CLICOLOR_FORCE", "1")
            .env("FORCE_COLOR", "true")
            .env_remove("NO_COLOR");

        // Language/program specific color settings
        command
            .env("SBT_OPTS", "-Dsbt.color=always")
            .env("JAVA_OPTS", "-Dsbt.color=always");

        // enabled Git colors
        command.env("GIT_CONFIG_PARAMETERS", "'color.ui=always'");

        // Other common tools
        command.env("GREP_OPTIONS", "--color=always"); // GNU grep

        let parameter = if is_windows { "/C" } else { "-c" };
        command.arg(parameter);

        #[cfg(windows)]
        command.raw_arg(command_str);
        #[cfg(unix)]
        command.arg(command_str);

        tracing::info!(command = command_str, "Executing command");

        command.kill_on_drop(true);

        // Set the working directory
        command.current_dir(working_dir);

        // Configure the command for output
        command
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Set requested environment variables
        if let Some(env_vars) = env_vars {
            for env_var in env_vars {
                if let Ok(value) = std::env::var(&env_var) {
                    command.env(&env_var, value);
                    tracing::debug!(env_var = %env_var, "Set environment variable from system");
                } else {
                    tracing::warn!(env_var = %env_var, "Environment variable not found in system");
                }
            }
        }

        command
    }

    /// Internal method to execute commands with streaming to console
    async fn execute_command_internal(
        &self,
        command: String,
        working_dir: &Path,
        silent: bool,
        env_vars: Option<Vec<String>>,
    ) -> anyhow::Result<CommandOutput> {
        let ready = self.ready.lock().await;

        let mut prepared_command = self.prepare_command(&command, working_dir, env_vars);

        // Spawn the command
        let mut child = prepared_command.spawn()?;

        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();

        // Stream the output of the command to stdout and stderr concurrently
        let (status, stdout_buffer, stderr_buffer) = if silent {
            tokio::try_join!(
                child.wait(),
                stream(&mut stdout_pipe, io::sink()),
                stream(&mut stderr_pipe, io::sink())
            )?
        } else {
            let stdout_writer = OutputPrinterWriter::stdout(self.output_printer.clone());
            let stderr_writer = OutputPrinterWriter::stderr(self.output_printer.clone());
            tokio::try_join!(
                child.wait(),
                stream(&mut stdout_pipe, stdout_writer),
                stream(&mut stderr_pipe, stderr_writer)
            )?
        };

        // Drop happens after `try_join` due to <https://github.com/tokio-rs/tokio/issues/4309>
        drop(stdout_pipe);
        drop(stderr_pipe);
        drop(ready);

        // Use smart encoding detection for Windows code pages (CP866, CP1251, etc.)
        Ok(CommandOutput {
            stdout: crate::text_encoding::bytes_to_string_smart(&stdout_buffer),
            stderr: crate::text_encoding::bytes_to_string_smart(&stderr_buffer),
            exit_code: status.code(),
            command,
        })
    }
}

/// Writer that delegates to OutputPrinter for synchronized writes.
struct OutputPrinterWriter {
    printer: Arc<StdConsoleWriter>,
    is_stdout: bool,
}

impl OutputPrinterWriter {
    fn stdout(printer: Arc<StdConsoleWriter>) -> Self {
        Self { printer, is_stdout: true }
    }

    fn stderr(printer: Arc<StdConsoleWriter>) -> Self {
        Self { printer, is_stdout: false }
    }
}

impl Write for OutputPrinterWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.is_stdout {
            self.printer.write(buf)
        } else {
            self.printer.write_err(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.is_stdout {
            self.printer.flush()
        } else {
            self.printer.flush_err()
        }
    }
}

/// reads the output from A and writes it to W
async fn stream<A: AsyncReadExt + Unpin, W: Write>(
    io: &mut Option<A>,
    mut writer: W,
) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    if let Some(io) = io.as_mut() {
        let mut buff = [0; 1024];
        loop {
            let n = io.read(&mut buff).await?;
            if n == 0 {
                break;
            }
            writer.write_all(&buff[..n])?;
            // note: flush is necessary else we get the cursor could not be found error.
            writer.flush()?;
            output.extend_from_slice(&buff[..n]);
        }
    }
    Ok(output)
}

/// The implementation for CommandExecutorService
#[async_trait::async_trait]
impl CommandInfra for ForgeCommandExecutorService {
    async fn execute_command(
        &self,
        command: String,
        working_dir: PathBuf,
        silent: bool,
        env_vars: Option<Vec<String>>,
    ) -> anyhow::Result<CommandOutput> {
        self.execute_command_internal(command, &working_dir, silent, env_vars)
            .await
    }

    async fn execute_command_raw(
        &self,
        command: &str,
        working_dir: PathBuf,
        env_vars: Option<Vec<String>>,
    ) -> anyhow::Result<std::process::ExitStatus> {
        let mut prepared_command = self.prepare_command(command, &working_dir, env_vars);

        // overwrite the stdin, stdout and stderr to inherit
        prepared_command
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());

        Ok(prepared_command.spawn()?.wait().await?)
    }
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;

    use super::*;

    fn test_env() -> Environment {
        use fake::{Fake, Faker};
        let max_bytes: f64 = 250.0 * 1024.0; // 250 KB
        let fixture: Environment = Faker.fake();
        fixture
            .max_search_result_bytes(max_bytes.ceil() as usize)
            .shell(
                if cfg!(target_os = "windows") {
                    "cmd"
                } else {
                    "bash"
                }
                .to_string(),
            )
    }

    fn test_printer() -> Arc<StdConsoleWriter> {
        Arc::new(StdConsoleWriter::default())
    }

    #[tokio::test]
    async fn test_command_executor() {
        let fixture = ForgeCommandExecutorService::new(false, test_env(), test_printer());
        let cmd = "echo 'hello world'";
        let dir = ".";

        let actual = fixture
            .execute_command(cmd.to_string(), PathBuf::new().join(dir), false, None)
            .await
            .unwrap();

        let mut expected = CommandOutput {
            stdout: "hello world\n".to_string(),
            stderr: "".to_string(),
            command: "echo \"hello world\"".into(),
            exit_code: Some(0),
        };

        if cfg!(target_os = "windows") {
            expected.stdout = format!("'{}'", expected.stdout);
        }

        assert_eq!(actual.stdout.trim(), expected.stdout.trim());
        assert_eq!(actual.stderr, expected.stderr);
        assert_eq!(actual.success(), expected.success());
    }
    #[tokio::test]
    async fn test_command_executor_with_env_vars_success() {
        // Set up test environment variables
        unsafe {
            std::env::set_var("TEST_ENV_VAR", "test_value");
            std::env::set_var("ANOTHER_TEST_VAR", "another_value");
        }

        let fixture = ForgeCommandExecutorService::new(false, test_env(), test_printer());
        let cmd = if cfg!(target_os = "windows") {
            "echo %TEST_ENV_VAR%"
        } else {
            "echo $TEST_ENV_VAR"
        };

        let actual = fixture
            .execute_command(
                cmd.to_string(),
                PathBuf::new().join("."),
                false,
                Some(vec!["TEST_ENV_VAR".to_string()]),
            )
            .await
            .unwrap();

        assert!(actual.success());
        assert!(actual.stdout.contains("test_value"));

        // Clean up
        unsafe {
            std::env::remove_var("TEST_ENV_VAR");
            std::env::remove_var("ANOTHER_TEST_VAR");
        }
    }

    #[tokio::test]
    async fn test_command_executor_with_missing_env_vars() {
        unsafe {
            std::env::remove_var("MISSING_ENV_VAR");
        }

        let fixture = ForgeCommandExecutorService::new(false, test_env(), test_printer());
        let cmd = if cfg!(target_os = "windows") {
            "echo %MISSING_ENV_VAR%"
        } else {
            "echo ${MISSING_ENV_VAR:-default_value}"
        };

        let actual = fixture
            .execute_command(
                cmd.to_string(),
                PathBuf::new().join("."),
                false,
                Some(vec!["MISSING_ENV_VAR".to_string()]),
            )
            .await
            .unwrap();

        // Should still succeed even with missing env vars
        assert!(actual.success());
    }

    #[tokio::test]
    async fn test_command_executor_with_empty_env_list() {
        let fixture = ForgeCommandExecutorService::new(false, test_env(), test_printer());
        let cmd = "echo 'no env vars'";

        let actual = fixture
            .execute_command(
                cmd.to_string(),
                PathBuf::new().join("."),
                false,
                Some(vec![]),
            )
            .await
            .unwrap();

        assert!(actual.success());
        assert!(actual.stdout.contains("no env vars"));
    }

    #[tokio::test]
    async fn test_command_executor_with_multiple_env_vars() {
        unsafe {
            std::env::set_var("FIRST_VAR", "first");
            std::env::set_var("SECOND_VAR", "second");
        }

        let fixture = ForgeCommandExecutorService::new(false, test_env(), test_printer());
        let cmd = if cfg!(target_os = "windows") {
            "echo %FIRST_VAR% %SECOND_VAR%"
        } else {
            "echo $FIRST_VAR $SECOND_VAR"
        };

        let actual = fixture
            .execute_command(
                cmd.to_string(),
                PathBuf::new().join("."),
                false,
                Some(vec!["FIRST_VAR".to_string(), "SECOND_VAR".to_string()]),
            )
            .await
            .unwrap();

        assert!(actual.success());
        assert!(actual.stdout.contains("first"));
        assert!(actual.stdout.contains("second"));

        // Clean up
        unsafe {
            std::env::remove_var("FIRST_VAR");
            std::env::remove_var("SECOND_VAR");
        }
    }

    #[tokio::test]
    async fn test_command_executor_silent() {
        let fixture = ForgeCommandExecutorService::new(false, test_env(), test_printer());
        let cmd = "echo 'silent test'";
        let dir = ".";

        let actual = fixture
            .execute_command(cmd.to_string(), PathBuf::new().join(dir), true, None)
            .await
            .unwrap();

        let mut expected = CommandOutput {
            stdout: "silent test\n".to_string(),
            stderr: "".to_string(),
            command: "echo \"silent test\"".into(),
            exit_code: Some(0),
        };

        if cfg!(target_os = "windows") {
            expected.stdout = format!("'{}'", expected.stdout);
        }

        // The output should still be captured in the CommandOutput
        assert_eq!(actual.stdout.trim(), expected.stdout.trim());
        assert_eq!(actual.stderr, expected.stderr);
        assert_eq!(actual.success(), expected.success());
    }
}
