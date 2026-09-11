//! Media-layer errors.

/// Result alias for this crate.
pub type Result<T, E = MediaError> = std::result::Result<T, E>;

/// Errors from probing, decoding, or talking to the worker.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MediaError {
    /// libav returned an error.
    #[error("libav: {0}")]
    Libav(String),

    /// The file has no stream of the requested kind.
    #[error("no {0} stream in {1}")]
    NoStream(&'static str, String),

    /// Filesystem or pipe error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Message framing or serialisation error.
    #[error("protocol: {0}")]
    Protocol(String),

    /// The worker exited or crashed.
    #[error("worker exited unexpectedly: {0}")]
    WorkerExited(String),

    /// The worker reported an error.
    #[error("worker: {0}")]
    Worker(String),

    /// No message from the worker within the configured idle timeout.
    #[error("worker idle for more than {0} s")]
    Timeout(u64),

    /// The request was cancelled by the caller.
    #[error("cancelled")]
    Cancelled,

    /// Invalid caller argument.
    #[error("invalid argument: {0}")]
    Invalid(String),

    /// Source could not be acquired.
    #[error("acquire: {0}")]
    Acquire(String),
}

impl From<ffmpeg_next::Error> for MediaError {
    fn from(e: ffmpeg_next::Error) -> Self {
        MediaError::Libav(e.to_string())
    }
}

impl From<serde_json::Error> for MediaError {
    fn from(e: serde_json::Error) -> Self {
        MediaError::Protocol(e.to_string())
    }
}

impl From<MediaError> for vi_core::Error {
    fn from(e: MediaError) -> Self {
        match e {
            MediaError::Io(io) => vi_core::Error::Io(io),
            MediaError::Cancelled => vi_core::Error::Cancelled,
            MediaError::Timeout(s) => vi_core::Error::Timeout(format!("media worker idle {s} s")),
            MediaError::Protocol(p) => vi_core::Error::Protocol(p),
            MediaError::Invalid(p) => vi_core::Error::Invalid(p),
            other => vi_core::Error::Media(other.to_string()),
        }
    }
}
