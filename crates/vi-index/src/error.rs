//! Storage errors.

/// Result alias.
pub type Result<T, E = IndexError> = std::result::Result<T, E>;

/// Errors from the storage layer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IndexError {
    /// SQLite.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Filesystem.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// JSON.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// Manifest says the index is newer than this build understands.
    #[error("index schema version {found} is newer than supported {supported}")]
    SchemaTooNew {
        /// Version found.
        found: u32,
        /// Highest supported.
        supported: u32,
    },
    /// The directory is not an index.
    #[error("not an index directory: {0}")]
    NotAnIndex(String),
    /// The directory already holds an index.
    #[error("index already exists: {0}")]
    AlreadyExists(String),
    /// Bad data in the database.
    #[error("corrupt: {0}")]
    Corrupt(String),
    /// Unsupported operation on this backend.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// Blocking task failed.
    #[error("task: {0}")]
    Task(String),
    /// Caller error.
    #[error("invalid: {0}")]
    Invalid(String),
}

impl From<IndexError> for vi_core::Error {
    fn from(e: IndexError) -> Self {
        match e {
            IndexError::Io(io) => vi_core::Error::Io(io),
            IndexError::Json(j) => vi_core::Error::Json(j),
            IndexError::SchemaTooNew { found, supported } => {
                vi_core::Error::SchemaTooNew { found, supported }
            }
            IndexError::NotAnIndex(p) => vi_core::Error::NotFound(format!("index at {p}")),
            IndexError::Invalid(m) => vi_core::Error::Invalid(m),
            IndexError::Unsupported(m) => vi_core::Error::Unsupported(m),
            other => vi_core::Error::Storage(other.to_string()),
        }
    }
}

impl From<tokio::task::JoinError> for IndexError {
    fn from(e: tokio::task::JoinError) -> Self {
        IndexError::Task(e.to_string())
    }
}
