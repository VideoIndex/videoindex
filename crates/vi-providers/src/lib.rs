//! `vi-providers`: provider traits + adapters, rate limiting, retries, cost accounting. Arrives in M1/M2.
//!
//! Stub in M0; see `docs/10-roadmap.md` for the milestone that fills it in.

#![cfg_attr(test, allow(clippy::unwrap_used))]

/// Crate version, so the stub exports something and links.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
