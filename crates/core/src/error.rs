//! Error types shared across the workspace.

use thiserror::Error;

#[derive(Debug, Error, uniffi::Error)]
pub enum Error {
    #[error("invalid id: {0}")]
    InvalidId(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("already exists: {0}")]
    AlreadyExists(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("llm error: {0}")]
    Llm(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("tool error: {0}")]
    Tool(String),

    #[error("queue error: {0}")]
    Queue(String),

    #[error("channel error: {0}")]
    Channel(String),

    #[error("workspace error: {0}")]
    Workspace(String),

    #[error("browser error: {0}")]
    Browser(String),

    #[error("io error: {0}")]
    Io(String),

    #[error("json error: {0}")]
    Json(String),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e.to_string())
    }
}
