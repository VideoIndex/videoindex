//! Usage and cost accounting. Every provider call produces a [`CallStats`];
//! operators turn it into a [`Provenance`] row with [`provenance_for`].

use chrono::Utc;
use serde::{Deserialize, Serialize};
use vi_core::config::Pricing;
use vi_core::model::Provenance;
use vi_core::ProvenanceId;

/// What a call consumed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    /// Input tokens.
    pub tokens_in: u64,
    /// Output tokens.
    pub tokens_out: u64,
    /// Images sent.
    pub images: u64,
    /// Seconds of audio or video sent.
    pub media_secs: f64,
    /// Requests made (1 per call, more when batching splits).
    pub calls: u64,
}

impl Usage {
    /// Cost of this usage at the given prices.
    pub fn cost_usd(&self, p: &Pricing) -> f64 {
        self.tokens_in as f64 / 1e6 * p.input_per_mtok
            + self.tokens_out as f64 / 1e6 * p.output_per_mtok
            + self.images as f64 * p.per_image
            + self.media_secs * p.per_media_second
            + self.calls as f64 * p.per_call
    }

    /// Sum.
    pub fn add(&mut self, other: &Usage) {
        self.tokens_in += other.tokens_in;
        self.tokens_out += other.tokens_out;
        self.images += other.images;
        self.media_secs += other.media_secs;
        self.calls += other.calls;
    }
}

/// Everything an operator needs to record about one provider call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallStats {
    /// Provider name from config.
    pub provider: String,
    /// Model that answered.
    pub model: String,
    /// Model version when the server reports one.
    pub model_version: Option<String>,
    /// Usage.
    pub usage: Usage,
    /// Cost at the provider's price table.
    pub cost_usd: f64,
    /// Wall-clock milliseconds including retries and queueing.
    pub latency_ms: u64,
    /// Attempts made (1 means no retry).
    pub attempts: u32,
}

impl CallStats {
    /// Stats for a call that consumed `usage`.
    pub fn new(provider: &str, model: &str, usage: Usage, pricing: &Pricing) -> Self {
        Self {
            provider: provider.to_string(),
            model: model.to_string(),
            model_version: None,
            cost_usd: usage.cost_usd(pricing),
            usage,
            latency_ms: 0,
            attempts: 1,
        }
    }
}

/// Build a [`Provenance`] row for a provider-backed operator output.
pub fn provenance_for(
    operator: &str,
    operator_version: u32,
    stats: &CallStats,
    prompt_hash: Option<String>,
    params: serde_json::Value,
) -> Provenance {
    Provenance {
        id: ProvenanceId::new(),
        operator: operator.to_string(),
        operator_version,
        provider: Some(stats.provider.clone()),
        model: Some(stats.model.clone()),
        model_version: stats.model_version.clone(),
        prompt_hash,
        params,
        created_at: Utc::now(),
        cost_usd: stats.cost_usd,
        tokens_in: stats.usage.tokens_in,
        tokens_out: stats.usage.tokens_out,
        latency_ms: stats.latency_ms,
    }
}

/// Hash a prompt (or any versioned asset) for cache keys and provenance.
pub fn prompt_hash(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex()[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_adds_up() {
        let p = Pricing {
            input_per_mtok: 1.0,
            output_per_mtok: 2.0,
            per_image: 0.01,
            per_media_second: 0.001,
            per_call: 0.0,
        };
        let u = Usage {
            tokens_in: 1_000_000,
            tokens_out: 500_000,
            images: 3,
            media_secs: 100.0,
            calls: 1,
        };
        assert!((u.cost_usd(&p) - (1.0 + 1.0 + 0.03 + 0.1)).abs() < 1e-9);
        assert_eq!(Usage::default().cost_usd(&p), 0.0);
    }
}
