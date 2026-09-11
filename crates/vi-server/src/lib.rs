//! `vi-server`: axum HTTP/SSE/WebSocket API, MCP server. Arrives in M3.
//!
//! Stub in M0; see `docs/10-roadmap.md` for the milestone that fills it in.

#![cfg_attr(test, allow(clippy::unwrap_used))]

/// Crate version, so the stub exports something and links.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
