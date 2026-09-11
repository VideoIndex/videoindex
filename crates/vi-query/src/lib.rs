//! `vi-query`: retrieval over an index (`docs/06-query-and-agents.md`).
//!
//! M1 so far: BM25 text search over transcript, OCR and description rows,
//! fused across kinds with reciprocal rank fusion and grouped into chapter
//! (or fixed-window) segments with evidence and a thumbnail. Vector search
//! and reranking join once embeddings exist.

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod search;

pub use search::{search, Evidence, SearchHit, SearchRequest, SearchResponse};
