//! The shared error type. Library crates return `vi_core::Error` (or a
//! crate-local `thiserror` type that converts into it); only `vi-cli` uses
//! `anyhow`.

/// Result alias used across the workspace.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors that can cross crate boundaries.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Configuration file or environment override is invalid.
    #[error("config: {0}")]
    Config(String),

    /// Filesystem or pipe error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialisation.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    /// The media layer (probe, decode, worker) failed.
    #[error("media: {0}")]
    Media(String),

    /// The storage layer failed.
    #[error("storage: {0}")]
    Storage(String),

    /// A pipeline operator failed.
    #[error("operator {operator}: {message}")]
    Operator {
        /// Operator id.
        operator: String,
        /// What went wrong.
        message: String,
    },

    /// The worker protocol was violated (truncated frame, unknown message).
    #[error("protocol: {0}")]
    Protocol(String),

    /// A wall-clock or budget limit was hit.
    #[error("timeout: {0}")]
    Timeout(String),

    /// The operation was cancelled.
    #[error("cancelled")]
    Cancelled,

    /// A budget (tokens, cost, time) was exhausted.
    #[error("budget exhausted: {0}")]
    Budget(String),

    /// A feature is not implemented for this backend or platform.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// A model provider call failed after retries, or no provider is bound
    /// to the role an operator needs.
    #[error("provider: {0}")]
    Provider(String),

    /// Caller passed something invalid.
    #[error("invalid argument: {0}")]
    Invalid(String),

    /// Something was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// The index on disk has a schema this build cannot read.
    #[error("index schema version {found} is newer than supported {supported}")]
    SchemaTooNew {
        /// Version in the manifest.
        found: u32,
        /// Highest version this binary understands.
        supported: u32,
    },

    /// Anything else.
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Convenience constructor for [`Error::Media`].
    pub fn media(msg: impl Into<String>) -> Self {
        Self::Media(msg.into())
    }

    /// Convenience constructor for [`Error::Storage`].
    pub fn storage(msg: impl Into<String>) -> Self {
        Self::Storage(msg.into())
    }

    /// Convenience constructor for [`Error::Invalid`].
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(msg.into())
    }

    /// Convenience constructor for [`Error::Protocol`].
    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(msg.into())
    }

    /// Convenience constructor for [`Error::Provider`].
    pub fn provider(msg: impl Into<String>) -> Self {
        Self::Provider(msg.into())
    }

    /// Convenience constructor for [`Error::Operator`].
    pub fn operator(operator: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Operator {
            operator: operator.into(),
            message: message.into(),
        }
    }
}
