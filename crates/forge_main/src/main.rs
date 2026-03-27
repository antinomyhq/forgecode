use std::io::Read;
use std::panic;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use forge_api::{ForgeAPI, TensorlakeConfig};
use forge_domain::TitleFormat;
use forge_main::{Cli, Sandbox, TitleDisplayExt, UI, tracker};

#[tokio::main]
async fn main() -> Result<()> {
    // Enable ANSI color support on Windows console
    #[cfg(windows)]
    let _ = enable_ansi_support::enable_ansi_support();

    // Install default rustls crypto provider (ring) before any TLS connections
    // This is required for rustls 0.23+ when multiple crypto providers are
    // available
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Set up panic hook for better error display.
    // Important: do NOT call `std::process::exit` here. Exiting inside the hook
    // skips stack unwinding, which prevents `Drop` implementations (such as the
    // Tensorlake `SandboxGuard`) from running. Returning from the hook lets Rust
    // unwind the stack normally so all destructors fire before the process exits.
    panic::set_hook(Box::new(|panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unexpected error occurred".to_string()
        };

        println!("{}", TitleFormat::error(message.to_string()).display());
        tracker::error_blocking(message);
    }));

    // Initialize and run the UI
    let mut cli = Cli::parse();

    // Check if there's piped input
    if !atty::is(atty::Stream::Stdin) {
        let mut stdin_content = String::new();
        std::io::stdin().read_to_string(&mut stdin_content)?;
        let trimmed_content = stdin_content.trim();
        if !trimmed_content.is_empty() {
            cli.piped_input = Some(trimmed_content.to_string());
        }
    }

    // Handle worktree creation if specified
    let cwd: PathBuf = match (&cli.sandbox, &cli.directory) {
        (Some(sandbox), Some(cli)) => {
            let mut sandbox = Sandbox::new(sandbox).create()?;
            sandbox.push(cli);
            sandbox
        }
        (Some(sandbox), _) => Sandbox::new(sandbox).create()?,
        (_, Some(cli)) => match cli.canonicalize() {
            Ok(cwd) => cwd,
            Err(_) => panic!("Invalid path: {}", cli.display()),
        },
        (_, _) => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    let tensorlake_key = cli.tensorlake.clone();
    let mut ui = UI::init(cli, move || {
        if let Some(api_key) = &tensorlake_key {
            ForgeAPI::init_with_tensorlake(
                cwd.clone(),
                TensorlakeConfig::new(api_key.to_string()),
            )
        } else {
            ForgeAPI::init(cwd.clone())
        }
    })?;
    ui.run().await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use forge_main::TopLevelCommand;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_stdin_detection_logic() {
        // This test verifies that the logic for detecting stdin is correct
        // We can't easily test the actual stdin reading in a unit test,
        // but we can verify the logic flow

        // Test that when prompt is provided, it remains independent of piped input
        let cli_with_prompt = Cli::parse_from(["forge", "--prompt", "existing prompt"]);
        let original_prompt = cli_with_prompt.prompt.clone();

        // The prompt should remain as provided
        assert_eq!(original_prompt, Some("existing prompt".to_string()));

        // Test that when no prompt is provided, piped_input field exists
        let cli_no_prompt = Cli::parse_from(["forge"]);
        assert_eq!(cli_no_prompt.prompt, None);
        assert_eq!(cli_no_prompt.piped_input, None);
    }

    #[test]
    fn test_cli_parsing_with_short_flag() {
        // Test that the short flag -p also works correctly
        let cli_with_short_prompt = Cli::parse_from(["forge", "-p", "short flag prompt"]);
        assert_eq!(
            cli_with_short_prompt.prompt,
            Some("short flag prompt".to_string())
        );
    }

    #[test]
    fn test_cli_parsing_other_flags_work_with_piping() {
        // Test that other CLI flags still work when expecting stdin input
        let cli_with_flags = Cli::parse_from(["forge", "--verbose"]);
        assert_eq!(cli_with_flags.prompt, None);
        assert_eq!(cli_with_flags.verbose, true);
    }

    #[test]
    fn test_commit_command_diff_field_initially_none() {
        // Test that the diff field in CommitCommandGroup starts as None
        let cli = Cli::parse_from(["forge", "commit", "--preview"]);
        if let Some(TopLevelCommand::Commit(commit_group)) = cli.subcommands {
            assert_eq!(commit_group.preview, true);
            assert_eq!(commit_group.diff, None);
        } else {
            panic!("Expected Commit command");
        }
    }
}
