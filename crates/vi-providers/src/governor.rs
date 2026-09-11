//! Per-provider concurrency and rate limiting: a semaphore plus token
//! buckets for requests and tokens per minute.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use vi_core::config::ProviderConfig;

use crate::error::{ProviderError, Result};
use crate::retry::RetryPolicy;

/// A token bucket refilled continuously at `per_minute / 60` per second.
#[derive(Debug)]
pub struct Bucket {
    state: Mutex<BucketState>,
    capacity: f64,
    per_sec: f64,
}

#[derive(Debug)]
struct BucketState {
    tokens: f64,
    last: Instant,
}

impl Bucket {
    /// Bucket allowing `per_minute` units per minute, with a burst of one
    /// minute's worth.
    pub fn per_minute(per_minute: u32) -> Self {
        let capacity = f64::from(per_minute.max(1));
        Self {
            state: Mutex::new(BucketState {
                tokens: capacity,
                last: Instant::now(),
            }),
            capacity,
            per_sec: capacity / 60.0,
        }
    }

    /// Wait until `n` units are available and take them. Requests larger
    /// than the capacity take the whole bucket and wait proportionally.
    pub async fn take(&self, n: f64) {
        let n = n.max(0.0);
        loop {
            let wait = {
                let mut s = self.state.lock().await;
                let now = Instant::now();
                s.tokens =
                    (s.tokens + s.last.elapsed().as_secs_f64() * self.per_sec).min(self.capacity);
                s.last = now;
                let need = n.min(self.capacity);
                if s.tokens >= need {
                    s.tokens -= n; // may go negative for oversize requests
                    return;
                }
                Duration::from_secs_f64((need - s.tokens) / self.per_sec)
            };
            tokio::time::sleep(wait).await;
        }
    }
}

/// Everything a provider's calls pass through.
#[derive(Debug)]
pub struct Governor {
    /// Provider name, for logs.
    pub name: String,
    semaphore: Arc<Semaphore>,
    requests: Option<Bucket>,
    tokens: Option<Bucket>,
    /// Retry policy for this provider.
    pub retry: RetryPolicy,
    /// Per-request timeout.
    pub timeout: Duration,
}

/// Held for the duration of a call.
#[derive(Debug)]
pub struct Permit {
    _permit: OwnedSemaphorePermit,
}

impl Governor {
    /// From a provider's config table.
    pub fn from_config(name: &str, cfg: &ProviderConfig) -> Self {
        Self {
            name: name.to_string(),
            semaphore: Arc::new(Semaphore::new(cfg.concurrency.unwrap_or(4).max(1) as usize)),
            requests: cfg.requests_per_minute.map(Bucket::per_minute),
            tokens: cfg.tokens_per_minute.map(Bucket::per_minute),
            retry: RetryPolicy {
                max_retries: cfg.max_retries.unwrap_or(4),
                ..RetryPolicy::default()
            },
            timeout: Duration::from_secs(cfg.timeout_secs.unwrap_or(300).max(1)),
        }
    }

    /// Unlimited, for tests and local adapters.
    pub fn unlimited(name: &str) -> Self {
        Self {
            name: name.to_string(),
            semaphore: Arc::new(Semaphore::new(Semaphore::MAX_PERMITS)),
            requests: None,
            tokens: None,
            retry: RetryPolicy::default(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Wait for a concurrency slot and for the rate limits, given an
    /// estimate of the tokens the call will use.
    pub async fn acquire(&self, estimated_tokens: u64) -> Result<Permit> {
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ProviderError::Cancelled)?;
        if let Some(b) = &self.requests {
            b.take(1.0).await;
        }
        if let Some(b) = &self.tokens {
            if estimated_tokens > 0 {
                b.take(estimated_tokens as f64).await;
            }
        }
        Ok(Permit { _permit: permit })
    }

    /// Permits currently free.
    pub fn available(&self) -> usize {
        self.semaphore.available_permits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bucket_paces_requests() {
        let b = Bucket::per_minute(6000); // 100 per second, burst 6000
        b.take(6000.0).await; // drain the burst
        let t = Instant::now();
        b.take(5.0).await; // 5 units at 100/s: about 50 ms
        let waited = t.elapsed();
        assert!(
            waited >= Duration::from_millis(40) && waited < Duration::from_secs(2),
            "{waited:?}"
        );
    }

    #[tokio::test]
    async fn governor_limits_concurrency() {
        let cfg = ProviderConfig {
            concurrency: Some(1),
            ..ProviderConfig::default()
        };
        let g = Governor::from_config("t", &cfg);
        let p = g.acquire(0).await.unwrap();
        assert_eq!(g.available(), 0);
        drop(p);
        assert_eq!(g.available(), 1);
    }
}
