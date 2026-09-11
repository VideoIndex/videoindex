//! Provider errors and their mapping to [`vi_core::Error`].

use std::time::Duration;

/// Result alias for this crate.
pub type Result<T, E = ProviderError> = std::result::Result<T, E>;

/// What can go wrong talking to a provider.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProviderError {
    /// No `[roles]` entry or `[providers]` table for what was asked.
    #[error("{0}")]
    NotConfigured(String),

    /// The adapter exists but does not implement this capability (yet).
    #[error("provider '{provider}' does not support {capability}")]
    Unsupported {
        /// Provider name from config.
        provider: String,
        /// Capability name, e.g. `asr`.
        capability: &'static str,
    },

    /// The server answered with a non-success status.
    #[error("{provider}: HTTP {status}: {body}")]
    Http {
        /// Provider name.
        provider: String,
        /// Status code.
        status: u16,
        /// Response body, truncated and with any key-like strings removed.
        body: String,
        /// `Retry-After`, when the server sent one.
        retry_after: Option<Duration>,
    },

    /// Connection, DNS, TLS, or read failure.
    #[error("{provider}: transport: {message}")]
    Transport {
        /// Provider name.
        provider: String,
        /// What failed.
        message: String,
    },

    /// The per-request timeout elapsed.
    #[error("{provider}: request timed out after {secs}s")]
    Timeout {
        /// Provider name.
        provider: String,
        /// Timeout that elapsed.
        secs: u64,
    },

    /// The response could not be parsed.
    #[error("{provider}: bad response: {message}")]
    Decode {
        /// Provider name.
        provider: String,
        /// What was wrong.
        message: String,
    },

    /// The caller's input was invalid (empty audio, oversized image).
    #[error("invalid request: {0}")]
    Invalid(String),

    /// A local model file is missing.
    #[error("model file missing: {path} ({hint})")]
    ModelMissing {
        /// Expected path.
        path: String,
        /// How to get it.
        hint: String,
    },

    /// A local inference runtime failed.
    #[error("{provider}: inference: {message}")]
    Inference {
        /// Provider name.
        provider: String,
        /// What failed.
        message: String,
    },

    /// Cancelled by the caller.
    #[error("cancelled")]
    Cancelled,

    /// The retry budget was exhausted; carries the last error.
    #[error("gave up after {attempts} attempts: {last}")]
    RetriesExhausted {
        /// Attempts made.
        attempts: u32,
        /// The last error.
        last: Box<ProviderError>,
    },
}

impl ProviderError {
    /// Whether a retry could succeed: 429, 5xx, transport and timeout.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http { status, .. } => *status == 429 || *status == 408 || *status >= 500,
            Self::Transport { .. } | Self::Timeout { .. } => true,
            _ => false,
        }
    }

    /// Server-suggested wait before retrying, when known.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Http { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}

impl From<ProviderError> for vi_core::Error {
    fn from(e: ProviderError) -> Self {
        match e {
            ProviderError::Cancelled => vi_core::Error::Cancelled,
            ProviderError::Unsupported { .. } => vi_core::Error::Unsupported(e.to_string()),
            ProviderError::Invalid(m) => vi_core::Error::Invalid(m),
            ProviderError::NotConfigured(m) => vi_core::Error::Provider(m),
            other => vi_core::Error::Provider(other.to_string()),
        }
    }
}

/// Remove anything that looks like a bearer token or API key from text that
/// might be logged or stored: `sk-...`, the word after `Bearer`, `key=...`,
/// and long opaque alphanumeric runs.
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut after_bearer = false;
    for word in text.split_inclusive(char::is_whitespace) {
        let trimmed = word.trim_end();
        let ws = &word[trimmed.len()..];
        let lower = trimmed.to_ascii_lowercase();
        let looks_like_key = after_bearer
            || (lower.starts_with("sk-") && trimmed.len() > 8)
            || ((lower.contains("key=") || lower.contains("token=") || lower.contains("api_key="))
                && trimmed.len() > 12)
            || (trimmed.len() >= 32
                && trimmed
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        after_bearer = lower == "bearer";
        if looks_like_key && !trimmed.is_empty() {
            out.push_str("[redacted]");
        } else {
            out.push_str(trimmed);
        }
        out.push_str(ws);
    }
    out
}

/// Truncate a response body for error messages, redacting keys.
pub fn short_body(body: &str) -> String {
    let mut s = redact(body.trim());
    if s.len() > 500 {
        let mut end = 500;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push('…');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_keys() {
        let r = redact("error for sk-abcdefghijklmnop please retry");
        assert!(r.contains("[redacted]") && !r.contains("sk-abc"));
        assert_eq!(redact("plain words only"), "plain words only");
        assert!(redact("Authorization: Bearer xyz").contains("[redacted]"));
    }

    #[test]
    fn retryable_classification() {
        let http = |status| ProviderError::Http {
            provider: "p".into(),
            status,
            body: String::new(),
            retry_after: None,
        };
        assert!(http(429).is_retryable());
        assert!(http(503).is_retryable());
        assert!(!http(400).is_retryable());
        assert!(!http(401).is_retryable());
        assert!(!ProviderError::Invalid("x".into()).is_retryable());
    }
}
