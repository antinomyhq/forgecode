//! Shell type detection and command argument derivation.
//!
//! This module provides type-safe shell handling with platform-specific support
//! for PowerShell, cmd.exe, bash, zsh, and other shells.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Represents the type of shell being used for command execution.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum ShellType {
    /// Z shell (common on modern macOS)
    Zsh,
    /// Bash shell (common on Linux)
    Bash,
    /// PowerShell (Windows PowerShell or PowerShell Core)
    PowerShell,
    /// Bourne shell (POSIX-compliant fallback)
    Sh,
    /// Windows Command Prompt
    Cmd,
}

impl ShellType {
    /// Returns the canonical name of the shell type.
    pub fn name(&self) -> &'static str {
        match self {
            ShellType::Zsh => "zsh",
            ShellType::Bash => "bash",
            ShellType::PowerShell => "powershell",
            ShellType::Sh => "sh",
            ShellType::Cmd => "cmd",
        }
    }

    /// Derives the full command arguments for executing a shell command.
    ///
    /// # Arguments
    /// * `shell_path` - Path to the shell executable
    /// * `command` - The command string to execute
    /// * `use_login_shell` - Whether to use login shell mode (loads profile/rc
    ///   files)
    ///
    /// # Returns
    /// A vector of arguments suitable for passing to `Command::args()`
    pub fn derive_exec_args(
        &self,
        shell_path: &Path,
        command: &str,
        use_login_shell: bool,
    ) -> Vec<String> {
        match self {
            ShellType::Zsh | ShellType::Bash | ShellType::Sh => {
                let arg = if use_login_shell { "-lc" } else { "-c" };
                vec![
                    shell_path.to_string_lossy().to_string(),
                    arg.to_string(),
                    command.to_string(),
                ]
            }
            ShellType::PowerShell => {
                let mut args = vec![shell_path.to_string_lossy().to_string()];
                if !use_login_shell {
                    args.push("-NoProfile".to_string());
                }
                args.push("-Command".to_string());
                args.push(command.to_string());
                args
            }
            ShellType::Cmd => {
                vec![
                    shell_path.to_string_lossy().to_string(),
                    "/c".to_string(),
                    command.to_string(),
                ]
            }
        }
    }

    /// Detects the shell type from a shell path.
    ///
    /// # Arguments
    /// * `shell_path` - Path to the shell executable
    ///
    /// # Returns
    /// `Some(ShellType)` if the shell could be detected, `None` otherwise
    pub fn detect(shell_path: &Path) -> Option<ShellType> {
        let file_name = shell_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        match file_name.to_lowercase().as_str() {
            "zsh" => Some(ShellType::Zsh),
            "bash" => Some(ShellType::Bash),
            "sh" => Some(ShellType::Sh),
            "pwsh" | "powershell" => Some(ShellType::PowerShell),
            "cmd" => Some(ShellType::Cmd),
            _ => None,
        }
    }
}

/// Discovers the shell path and type for the current system.
///
/// # Arguments
/// * `restricted` - If true, prefer restricted shells (rbash) on Unix systems
///
/// # Returns
/// A tuple of (shell_path, shell_type)
pub fn discover_shell(restricted: bool) -> (PathBuf, ShellType) {
    if cfg!(target_os = "windows") {
        discover_windows_shell()
    } else {
        discover_unix_shell(restricted)
    }
}

fn discover_windows_shell() -> (PathBuf, ShellType) {
    // Try PowerShell first (modern Windows)
    if let Some(path) = find_powershell() {
        return (path, ShellType::PowerShell);
    }

    // Fallback to cmd.exe
    let cmd_path = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
    (PathBuf::from(cmd_path), ShellType::Cmd)
}

fn discover_unix_shell(restricted: bool) -> (PathBuf, ShellType) {
    if restricted {
        // Use restricted bash if available
        if let Ok(path) = which::which("rbash") {
            return (path, ShellType::Bash);
        }
        // Fallback to restricted sh
        return (PathBuf::from("/bin/rbash"), ShellType::Bash);
    }

    // Try user's preferred shell from SHELL environment variable
    if let Ok(shell_var) = std::env::var("SHELL") {
        let shell_path = PathBuf::from(&shell_var);
        if let Some(shell_type) = ShellType::detect(&shell_path) {
            return (shell_path, shell_type);
        }
    }

    // Platform-specific defaults
    if cfg!(target_os = "macos") {
        // macOS defaults to zsh since Catalina
        if let Ok(path) = which::which("zsh") {
            return (path, ShellType::Zsh);
        }
    }

    // Try bash as common fallback
    if let Ok(path) = which::which("bash") {
        return (path, ShellType::Bash);
    }

    // Ultimate fallback to POSIX sh
    (PathBuf::from("/bin/sh"), ShellType::Sh)
}

fn find_powershell() -> Option<PathBuf> {
    // Try PowerShell Core first (cross-platform, newer)
    if let Ok(path) = which::which("pwsh") {
        return Some(path);
    }

    // Try Windows PowerShell (Windows 5.1)
    if let Ok(path) = which::which("powershell") {
        return Some(path);
    }

    None
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_shell_type_detection() {
        assert_eq!(
            ShellType::detect(&PathBuf::from("zsh")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            ShellType::detect(&PathBuf::from("bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            ShellType::detect(&PathBuf::from("pwsh")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            ShellType::detect(&PathBuf::from("powershell")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(ShellType::detect(&PathBuf::from("sh")), Some(ShellType::Sh));
        assert_eq!(
            ShellType::detect(&PathBuf::from("cmd")),
            Some(ShellType::Cmd)
        );
        assert_eq!(ShellType::detect(&PathBuf::from("fish")), None);
    }

    #[test]
    fn test_shell_type_detection_with_full_paths() {
        assert_eq!(
            ShellType::detect(&PathBuf::from("/bin/zsh")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            ShellType::detect(&PathBuf::from("/bin/bash")),
            Some(ShellType::Bash)
        );

        // Only test Windows paths on Windows since path parsing is platform-specific
        #[cfg(windows)]
        {
            assert_eq!(
                ShellType::detect(&PathBuf::from(
                    "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"
                )),
                Some(ShellType::PowerShell)
            );
        }

        assert_eq!(
            ShellType::detect(&PathBuf::from("/usr/local/bin/pwsh")),
            Some(ShellType::PowerShell)
        );
    }

    #[test]
    fn test_derive_exec_args_bash() {
        let shell_type = ShellType::Bash;
        let shell_path = PathBuf::from("/bin/bash");

        assert_eq!(
            shell_type.derive_exec_args(&shell_path, "echo hello", false),
            vec!["/bin/bash", "-c", "echo hello"]
        );

        assert_eq!(
            shell_type.derive_exec_args(&shell_path, "echo hello", true),
            vec!["/bin/bash", "-lc", "echo hello"]
        );
    }

    #[test]
    fn test_derive_exec_args_powershell() {
        let shell_type = ShellType::PowerShell;
        let shell_path = PathBuf::from("pwsh.exe");

        assert_eq!(
            shell_type.derive_exec_args(&shell_path, "echo hello", false),
            vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"]
        );

        assert_eq!(
            shell_type.derive_exec_args(&shell_path, "echo hello", true),
            vec!["pwsh.exe", "-Command", "echo hello"]
        );
    }

    #[test]
    fn test_derive_exec_args_cmd() {
        let shell_type = ShellType::Cmd;
        let shell_path = PathBuf::from("cmd.exe");

        assert_eq!(
            shell_type.derive_exec_args(&shell_path, "echo hello", false),
            vec!["cmd.exe", "/c", "echo hello"]
        );
    }

    #[test]
    fn test_shell_type_names() {
        assert_eq!(ShellType::Zsh.name(), "zsh");
        assert_eq!(ShellType::Bash.name(), "bash");
        assert_eq!(ShellType::PowerShell.name(), "powershell");
        assert_eq!(ShellType::Sh.name(), "sh");
        assert_eq!(ShellType::Cmd.name(), "cmd");
    }

    #[test]
    fn test_discover_shell_returns_valid_shell() {
        let (path, shell_type) = discover_shell(false);
        assert!(!path.as_os_str().is_empty());

        // Verify the detected type matches the path
        if let Some(detected_type) = ShellType::detect(&path) {
            assert_eq!(detected_type, shell_type);
        }
    }
}
