use std::path::PathBuf;
use std::sync::LazyLock;

use config::ConfigBuilder;
use config::builder::DefaultState;

use crate::ForgeConfig;
use crate::legacy::LegacyConfig;

/// Loads all `.env` files found while walking up from the current working
/// directory to the root, with priority given to closer (lower) directories.
/// Executed at most once per process.
static LOAD_DOT_ENV: LazyLock<()> = LazyLock::new(|| {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut paths = vec![];
    let mut current = PathBuf::new();

    for component in cwd.components() {
        current.push(component);
        paths.push(current.clone());
    }

    paths.reverse();

    for path in paths {
        let env_file = path.join(".env");
        if env_file.is_file() {
            dotenvy::from_path(&env_file).ok();
        }
    }
});

/// Merges [`ForgeConfig`] from layered sources using a builder pattern.
#[derive(Default)]
pub struct ConfigReader {
    builder: ConfigBuilder<DefaultState>,
}

impl ConfigReader {
    /// Returns the path to the legacy JSON config file
    /// (`~/.forge/.config.json`).
    pub fn config_legacy_path() -> PathBuf {
        Self::base_path().join(".config.json")
    }

    /// Returns the path to the primary TOML config file
    /// (`~/.forge/.forge.toml`).
    pub fn config_path() -> PathBuf {
        Self::base_path().join(".forge.toml")
    }

    /// Returns the base directory for all Forge config files.
    /// Checks `FORGE_FOLDER_PATH` env var first, falls back to `~/forge`.
    pub fn base_path() -> PathBuf {
        if let Ok(path) = std::env::var("FORGE_FOLDER_PATH") {
            return PathBuf::from(path);
        }
        dirs::home_dir().unwrap_or(PathBuf::from(".")).join("forge")
    }

    /// Adds the provided TOML string as a config source without touching the
    /// filesystem.
    pub fn read_toml(mut self, contents: &str) -> Self {
        self.builder = self
            .builder
            .add_source(config::File::from_str(contents, config::FileFormat::Toml));

        self
    }

    /// Adds the embedded default config (`../.forge.toml`) as a source.
    pub fn read_defaults(self) -> Self {
        let defaults = include_str!("../.forge.toml");

        self.read_toml(defaults)
    }

    /// Adds `FORGE_`-prefixed environment variables as a config source.
    pub fn read_env(mut self) -> Self {
        self.builder = self.builder.add_source(
            config::Environment::with_prefix("FORGE")
                .prefix_separator("_")
                .separator("__")
                .try_parsing(true)
                .list_separator(",")
                .with_list_parse_key("retry.status_codes")
                .with_list_parse_key("http.root_cert_paths"),
        );

        self
    }

    /// Builds and deserializes all accumulated sources into a [`ForgeConfig`].
    ///
    /// Triggers `.env` file loading (at most once per process) by walking up
    /// the directory tree from the current working directory, with closer
    /// directories taking priority.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration cannot be built or deserialized.
    pub fn build(self) -> crate::Result<ForgeConfig> {
        *LOAD_DOT_ENV;
        let config = self.builder.build()?;
        Ok(config.try_deserialize::<ForgeConfig>()?)
    }

    /// Adds `~/.forge/.forge.toml` as a config source, silently skipping if
    /// absent.
    pub fn read_global(mut self) -> Self {
        let path = Self::config_path();
        self.builder = self
            .builder
            .add_source(config::File::from(path).required(false));
        self
    }

    /// Reads `~/.forge/.config.json` (legacy format) and adds it as a source,
    /// silently skipping errors.
    pub fn read_legacy(self) -> Self {
        let content = LegacyConfig::read(&Self::config_legacy_path());
        if let Ok(content) = content {
            self.read_toml(&content)
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ModelConfig;

    /// Serializes tests that mutate environment variables to prevent races.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    /// Holds env vars set for a test's duration and removes them on drop, while
    /// holding [`ENV_MUTEX`].
    struct EnvGuard {
        keys: Vec<&'static str>,
        _lock: MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        /// Sets each `(key, value)` pair in the environment, returning a guard
        /// that cleans them up on drop.
        #[must_use]
        fn set(pairs: &[(&'static str, &str)]) -> Self {
            let lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
            let keys = pairs.iter().map(|(k, _)| *k).collect();
            for (key, value) in pairs {
                unsafe { std::env::set_var(key, value) };
            }
            Self { keys, _lock: lock }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for key in &self.keys {
                unsafe { std::env::remove_var(key) };
            }
        }
    }

    #[test]
    fn test_base_path_respects_env_var() {
        let _guard = EnvGuard::set(&[("FORGE_FOLDER_PATH", "/custom/forge/path")]);
        assert_eq!(
            ConfigReader::base_path(),
            PathBuf::from("/custom/forge/path")
        );
    }

    #[test]
    fn test_base_path_defaults_to_home_forge() {
        let _guard = EnvGuard::set(&[]);
        let path = ConfigReader::base_path();
        let expected = dirs::home_dir().unwrap_or(PathBuf::from(".")).join("forge");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_read_parses_without_error() {
        let actual = ConfigReader::default().read_defaults().build();
        assert!(actual.is_ok(), "read() failed: {:?}", actual.err());
    }

    #[test]
    fn test_read_session_from_env_vars() {
        let _guard = EnvGuard::set(&[
            ("FORGE_SESSION__PROVIDER_ID", "fake-provider"),
            ("FORGE_SESSION__MODEL_ID", "fake-model"),
        ]);

        let actual = ConfigReader::default()
            .read_defaults()
            .read_env()
            .build()
            .unwrap();

        let expected = Some(ModelConfig {
            provider_id: Some("fake-provider".to_string()),
            model_id: Some("fake-model".to_string()),
        });
        assert_eq!(actual.session, expected);
    }
}
