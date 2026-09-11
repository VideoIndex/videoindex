//! `vi-query`: retrieval over an index (`docs/06-query-and-agents.md`).
//!
//! M1: hybrid search. BM25 over transcript, OCR and description rows, a
//! text-vector list over the same rows and an image-vector list over frame
//! embeddings (SigLIP text tower), fused with reciprocal rank fusion and
//! grouped into temporal units (scenes, else shots cut to about 60 s, else
//! chapters, else windows) with evidence and a thumbnail. Reranking joins
//! in M2.

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod search;

pub use search::{search, Evidence, SearchHit, SearchRequest, SearchResponse};
