use crate::formats::Format;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid flavor name: {0:?}")]
    InvalidFlavorName(String),
    #[error("Flavor not found: {0}")]
    FlavorNotFound(String),
    #[error("Invalid configuration: {0}")]
    Config(String),
    #[error("Invalid configuration: {0}")]
    ConfigSource(#[source] BoxError),
    #[error("{format:?} error: {source}")]
    Format {
        format: Format,
        #[source]
        source: BoxError,
    },
    #[error("Script error: {0}")]
    Script(String),
    #[error("Script error: {0}")]
    ScriptSource(#[source] BoxError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Task(#[from] tokio::task::JoinError),
}
