use std::io::Read;
use std::panic;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use forge_api::ForgeAPI;
use forge_domain::TitleFormat;
use forge_main::{Cli, Sandbox, TitleDisplayExt, UI, rprompt_fast, tracker, utils};

#[tokio::main]
async fn main() -> Result<()> {
    // Enable ANSI color support on Windows console
    #[cfg(windows)]
    let _ = enable_ansi_support::enable_ansi_support();

    // Install default rustls crypto provider (ring) before any TLS connections
    // This is required for rustls 0.23+ when multiple crypto providers are
    // available
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Set up panic hook for better error display
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
        std::process::exit(1);
    }));

    // Initialize and run the UI

    // Fast path: zsh rprompt without conversation ID - check BEFORE Cli::parse()
    let args: Vec<String> = std::env::args().collect();
    let has_conv = std::env::var("_FORGE_CONVERSATION_ID")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .is_some();
    if args.len() >= 3 && args[1] == "zsh" && args[2] == "rprompt" && !has_conv {
        println!(" %B%F{{240}}󱙺 FORGE%f%b");
        return Ok(());
    }

    // Fast path: zsh rprompt WITH conversation ID - direct SQLite query
    if args.len() >= 3
        && args[1] == "zsh"
        && args[2] == "rprompt"
        && has_conv
        && let Ok(conv_id) = std::env::var("_FORGE_CONVERSATION_ID")
    {
        let conv_id = conv_id.trim();
        if !conv_id.is_empty() {
            // Try fast path - if it fails, fall through to normal path
            if let Some(data) = rprompt_fast::fetch_rprompt_data(conv_id) {
                let use_nerd_font = std::env::var("NERD_FONT")
                    .or_else(|_| std::env::var("USE_NERD_FONT"))
                    .map(|v| v == "1")
                    .unwrap_or(true);

                // Check if we have token count (active state) or just show inactive
                if let Some(token_count) = data.token_count {
                    let icon = if use_nerd_font { "󱙺" } else { "" };
                    let count_str = utils::humanize_number(token_count);

                    // Active state: bright colors
                    print!(" %B%F{{15}}{} FORGE%f%b %B%F{{15}}{}%f%b", icon, count_str);

                    if let Some(cost) = data.cost {
                        let currency = std::env::var("FORGE_CURRENCY_SYMBOL")
                            .unwrap_or_else(|_| "$".to_string());
                        let ratio: f64 = std::env::var("FORGE_CURRENCY_CONVERSION_RATE")
                            .ok()
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(1.0);
                        print!(" %B%F{{2}}{}{:.2}%f%b", currency, cost * ratio);
                    }

                    if let Some(ref model) = data.model {
                        let model_icon = if use_nerd_font { "󰑙" } else { "" };
                        print!(" %F{{134}}{}{}", model_icon, model);
                    }

                    println!();
                    return Ok(());
                } else {
                    // No token count - show inactive/dimmed state
                    let icon = if use_nerd_font { "󱙺" } else { "" };
                    let model_str = data.model.as_deref().unwrap_or("forge");
                    let model_icon = if use_nerd_font { "󰑙" } else { "" };

                    print!(
                        " %B%F{{240}}{} FORGE%f%b %F{{240}}{}{}%f",
                        icon, model_icon, model_str
                    );
                    println!();
                    return Ok(());
                }
            }
        }
    }

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

    let mut ui = UI::init(cli, move || ForgeAPI::init(cwd.clone()))?;
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
