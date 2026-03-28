mod auto_dump;
mod compact;
mod config;
mod env_registry;
mod error;
mod http;
mod legacy;
mod model;
mod reader;
mod retry;
mod writer;

pub use auto_dump::*;
pub use compact::*;
pub use config::*;
pub use env_registry::*;
pub use error::Error;
pub use http::*;
pub use model::*;
pub use reader::*;
pub use retry::*;
pub use writer::*;

/// A `Result` type alias for this crate's [`Error`] type.
pub type Result<T> = std::result::Result<T, Error>;
