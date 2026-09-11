//! `vi-providers`: model capabilities as traits, adapters that implement
//! them, and the cross-cutting behaviour every call gets: per-provider
//! concurrency and rate limits, retries with backoff, cost accounting into
//! [`vi_core::model::Provenance`] rows, and key redaction.
//!
//! See `docs/07-model-providers.md`. M1 ships the traits, the
//! [`registry::ProviderRegistry`] that binds roles from config, and the
//! `openai_compat` adapter's ASR method (Whisper-compatible servers). Chat,
//! embeddings over HTTP, Gemini and Anthropic follow in M2; the `onnx_local`
//! adapter (SigLIP, RapidOCR, bge) is wired from `vi-perceive`.

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod adapters;
pub mod cost;
pub mod error;
pub mod governor;
pub mod registry;
pub mod retry;
pub mod traits;

pub use cost::{provenance_for, CallStats, Usage};
pub use error::{ProviderError, Result};
pub use governor::Governor;
pub use registry::{AdapterKind, ProviderRegistry};
pub use retry::RetryPolicy;
pub use traits::*;
