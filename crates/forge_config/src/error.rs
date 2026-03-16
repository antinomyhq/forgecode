/// Errors that can occur when reading configuration.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The configuration source failed to load or a value could not be
    /// deserialized.
    #[error(transparent)]
    Config(#[from] config::ConfigError),
}
