//! FFI error type exposed to Kotlin.

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiError {
    #[error("{0}")]
    Core(String),
}

impl From<openbot_core::Error> for FfiError {
    fn from(e: openbot_core::Error) -> Self {
        FfiError::Core(e.to_string())
    }
}
