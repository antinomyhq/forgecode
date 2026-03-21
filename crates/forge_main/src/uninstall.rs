use colored::Colorize;
use forge_select::ForgeWidget;
use forge_tracker::{EventKind, UninstallReason};

use crate::TRACKER;

/// All selectable uninstall reasons shown to the user.
const REASONS: &[(&str, UninstallReason)] = &[
    ("Found a better tool", UninstallReason::FoundBetterTool),
    (
        "Too slow or resource-heavy",
        UninstallReason::TooSlowOrHeavy,
    ),
    ("Missing features I need", UninstallReason::MissingFeatures),
    ("Too many bugs or crashes", UninstallReason::TooManyBugs),
    (
        "Only needed it temporarily",
        UninstallReason::OnlyNeededTemporarily,
    ),
    ("Other", UninstallReason::Other(String::new())),
];

/// Prompts the user for an uninstall reason, sends it as a PostHog event, then
/// removes the Forge binary and any package-manager-managed installations.
pub async fn on_uninstall() {
    let options: Vec<&str> = REASONS.iter().map(|(label, _)| *label).collect();

    let selected = ForgeWidget::select("Why are you uninstalling Forge?", options)
        .with_help_message("Use arrow keys / type to filter, Enter to confirm, Esc to cancel")
        .prompt();

    let reason = match selected {
        Ok(Some(label)) => {
            if label == "Other" {
                // Ask for a short free-form description
                let description = ForgeWidget::input("Please briefly describe your reason:")
                    .allow_empty(true)
                    .prompt()
                    .unwrap_or_default()
                    .unwrap_or_default();
                UninstallReason::Other(description)
            } else {
                REASONS
                    .iter()
                    .find(|(l, _)| *l == label)
                    .map(|(_, r)| r.clone())
                    .unwrap_or(UninstallReason::Other(label.to_string()))
            }
        }
        // User cancelled or non-interactive — proceed without a reason
        _ => UninstallReason::Other("cancelled".to_string()),
    };

    // Confirm before removing anything
    let confirmed = ForgeWidget::confirm(format!(
        "Are you sure you want to uninstall Forge? (reason: {})",
        reason.to_string().bold()
    ))
    .with_default(false)
    .prompt()
    .unwrap_or_default()
    .unwrap_or_default();

    if !confirmed {
        println!("{}", "Uninstall cancelled.".yellow());
        return;
    }

    // Fire the PostHog event before removing the binary so tracking still works
    let _ = TRACKER.dispatch(EventKind::Uninstall(reason)).await;

    println!("{}", "Uninstalling Forge...".blue());
    run_uninstall_script().await;
}

/// Returns the platform-specific paths where the Forge binary may be installed.
fn forge_binary_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();

    // Path written by the official install script on Unix / macOS
    #[cfg(not(windows))]
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".local").join("bin").join("forge"));
    }

    // Path written by the official install script on Windows
    #[cfg(windows)]
    {
        // %LOCALAPPDATA%\Programs\Forge\forge.exe
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            paths.push(
                std::path::PathBuf::from(local_app_data)
                    .join("Programs")
                    .join("Forge")
                    .join("forge.exe"),
            );
        }
        // Fallback: %USERPROFILE%\.local\bin\forge.exe
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join(".local").join("bin").join("forge.exe"));
        }
    }

    paths
}

/// Removes the Forge binary and cleans up package-manager shims.
///
/// Uses pure Rust file removal and per-command `tokio::process::Command` calls
/// so this works on Windows, macOS, and Linux without relying on `sh`.
async fn run_uninstall_script() {
    let mut removed_any = false;

    // Remove every known install location for the forge binary
    for path in forge_binary_paths() {
        if path.exists() {
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    println!("Removed {}", path.display());
                    removed_any = true;
                }
                Err(err) => {
                    eprintln!("Could not remove {}: {err}", path.display());
                }
            }
        }
    }

    // Clean up package-manager-managed installations.
    // Each command is spawned independently; failures are silently ignored so
    // that missing tools do not abort the uninstall.
    let pm_commands: &[(&str, &[&str])] = &[
        ("volta", &["uninstall", "forgecode"]),
        ("npm", &["uninstall", "-g", "forgecode"]),
        ("asdf", &["reshim", "nodejs"]),
        ("mise", &["reshim"]),
        ("nodenv", &["rehash"]),
    ];

    for (program, args) in pm_commands {
        let _ = tokio::process::Command::new(program)
            .args(*args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;
    }

    if removed_any {
        println!("{}", "Forge was successfully uninstalled.".green());
    } else {
        println!(
            "{}",
            "No Forge binary found at the expected locations — it may already be removed.".yellow()
        );
    }

    println!("{}", "You can reinstall at any time with:".dimmed());
    println!("  {}", "curl -fsSL https://forgecode.dev/cli | sh".bold());
    println!(
        "{}",
        "We'd love your feedback: https://github.com/antinomyhq/forge/issues".dimmed()
    );
}
