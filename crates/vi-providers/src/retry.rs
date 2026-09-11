//! Retries with exponential backoff and jitter.

use std::future::Future;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::error::{ProviderError, Result};

/// Backoff parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetryPolicy {
    /// Retries after the first attempt.
    pub max_retries: u32,
    /// First delay.
    pub base: Duration,
    /// Multiplier per retry.
    pub factor: f64,
    /// Cap on any single delay (also caps `Retry-After`).
    pub max_delay: Duration,
    /// Jitter as a fraction of the delay, applied in `[1 - j, 1 + j]`.
    pub jitter: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 4,
            base: Duration::from_millis(500),
            factor: 2.0,
            max_delay: Duration::from_secs(30),
            jitter: 0.25,
        }
    }
}

impl RetryPolicy {
    /// No retries.
    pub const NONE: RetryPolicy = RetryPolicy {
        max_retries: 0,
        base: Duration::ZERO,
        factor: 1.0,
        max_delay: Duration::ZERO,
        jitter: 0.0,
    };

    /// Delay before retry number `retry` (1-based), given an optional
    /// server hint.
    pub fn delay(&self, retry: u32, hint: Option<Duration>) -> Duration {
        if let Some(h) = hint {
            return h.min(self.max_delay);
        }
        let exp = self.base.as_secs_f64() * self.factor.powi(retry.saturating_sub(1) as i32);
        let jitter = 1.0 + self.jitter * (2.0 * pseudo_random() - 1.0);
        Duration::from_secs_f64((exp * jitter).max(0.0)).min(self.max_delay)
    }
}

/// Cheap jitter source: no `rand` dependency, no reproducibility needed.
fn pseudo_random() -> f64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: AtomicU64 = AtomicU64::new(0);
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    let mut x = STATE.fetch_add(seed | 1, Ordering::Relaxed) ^ seed;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// Run `op` until it succeeds, fails with a non-retryable error, or the
/// policy is exhausted. Returns the result and the number of attempts.
pub async fn run<T, F, Fut>(
    policy: &RetryPolicy,
    cancel: &CancellationToken,
    mut op: F,
) -> Result<(T, u32)>
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut attempt = 0u32;
    loop {
        if cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        attempt += 1;
        match op(attempt).await {
            Ok(v) => return Ok((v, attempt)),
            Err(e) if e.is_retryable() && attempt <= policy.max_retries => {
                let delay = policy.delay(attempt, e.retry_after());
                tracing::warn!(attempt, delay_ms = delay.as_millis() as u64, error = %e, "provider call failed; retrying");
                tokio::select! {
                    _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
                    _ = tokio::time::sleep(delay) => {}
                }
            }
            Err(e) if attempt > 1 => {
                return Err(ProviderError::RetriesExhausted {
                    attempts: attempt,
                    last: Box::new(e),
                })
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    fn http(status: u16) -> ProviderError {
        ProviderError::Http {
            provider: "t".into(),
            status,
            body: String::new(),
            retry_after: None,
        }
    }

    #[tokio::test]
    async fn retries_then_succeeds() {
        let policy = RetryPolicy {
            base: Duration::from_millis(1),
            max_delay: Duration::from_millis(2),
            ..RetryPolicy::default()
        };
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let (v, attempts) = run(&policy, &CancellationToken::new(), move |_| {
            let c = c.clone();
            async move {
                if c.fetch_add(1, Ordering::SeqCst) < 2 {
                    Err(http(503))
                } else {
                    Ok(7)
                }
            }
        })
        .await
        .unwrap();
        assert_eq!((v, attempts), (7, 3));
    }

    #[tokio::test]
    async fn non_retryable_fails_fast_and_budget_is_finite() {
        let policy = RetryPolicy {
            max_retries: 2,
            base: Duration::from_millis(1),
            max_delay: Duration::from_millis(1),
            ..RetryPolicy::default()
        };
        let err = run(&policy, &CancellationToken::new(), |_| async {
            Err::<(), _>(http(400))
        })
        .await
        .unwrap_err();
        assert!(matches!(err, ProviderError::Http { status: 400, .. }));
        let err = run(&policy, &CancellationToken::new(), |_| async {
            Err::<(), _>(http(500))
        })
        .await
        .unwrap_err();
        assert!(matches!(
            err,
            ProviderError::RetriesExhausted { attempts: 3, .. }
        ));
    }

    #[test]
    fn delays_grow_and_cap() {
        let p = RetryPolicy {
            jitter: 0.0,
            ..RetryPolicy::default()
        };
        assert_eq!(p.delay(1, None), Duration::from_millis(500));
        assert_eq!(p.delay(2, None), Duration::from_millis(1000));
        assert_eq!(p.delay(20, None), Duration::from_secs(30));
        assert_eq!(
            p.delay(1, Some(Duration::from_secs(3))),
            Duration::from_secs(3)
        );
    }
}
