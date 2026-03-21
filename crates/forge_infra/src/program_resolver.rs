//! Platform-specific program resolution for executable paths.
//!
//! This module provides a unified interface for resolving executable paths
//! across different operating systems. The key challenge it addresses is that
//! Windows cannot execute script files (e.g., `.cmd`, `.bat`) directly without
//! their file extensions, while Unix systems handle scripts natively through
//! shebangs.

use std::collections::HashMap;
use std::ffi::OsString;

/// Resolves a program to its executable path on Unix systems.
///
/// Unix systems handle PATH resolution and script execution natively through
/// the kernel's shebang (`#!`) mechanism, so this function simply returns the
/// program name unchanged.
#[cfg(unix)]
pub fn resolve_program(
    program: OsString,
    _env: &HashMap<String, String>,
) -> std::io::Result<OsString> {
    Ok(program)
}

/// Resolves a program to its executable path on Windows systems.
///
/// Windows requires explicit file extensions for script execution. This
/// function uses the `which` crate to search the `PATH` environment variable
/// and find the full path to the executable, including necessary script
/// extensions (`.cmd`, `.bat`, etc.) defined in `PATHEXT`.
///
/// This enables tools like `npx`, `pnpm`, and `yarn` to work correctly on
/// Windows without requiring users to specify full paths or extensions in their
/// configuration.
#[cfg(windows)]
pub fn resolve_program(
    program: OsString,
    env: &HashMap<String, String>,
) -> std::io::Result<OsString> {
    use tracing::debug;

    // Get current directory for relative path resolution
    let cwd = std::env::current_dir()
        .map_err(|e| std::io::Error::other(format!("Failed to get current directory: {e}")))?;

    // Extract PATH from environment for search locations
    let search_path = env.get("PATH");

    // Attempt resolution via which crate
    match which::which_in(&program, search_path, &cwd) {
        Ok(resolved) => {
            debug!("Resolved {:?} to {:?}", program, resolved);
            Ok(resolved.into_os_string())
        }
        Err(e) => {
            debug!(
                "Failed to resolve {:?}: {}. Using original path",
                program, e
            );
            // Fallback to original program - let Command::new() handle the error
            Ok(program)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_program_empty() {
        let env = HashMap::new();
        let program = OsString::from("test_program");
        let result = resolve_program(program.clone(), &env);
        assert!(result.is_ok());

        // On Unix, should return unchanged
        #[cfg(unix)]
        assert_eq!(result.unwrap(), program);
    }

    #[test]
    fn test_resolve_program_with_path() {
        let mut env = HashMap::new();
        env.insert(
            "PATH".to_string(),
            std::env::var("PATH").unwrap_or_default(),
        );

        // Test with a command that should exist on the system
        #[cfg(unix)]
        let program = OsString::from("ls");
        #[cfg(windows)]
        let program = OsString::from("cmd");

        let result = resolve_program(program, &env);
        assert!(result.is_ok());
    }
}
