//! `vi-perceive`: CPU-bound, in-process perception operators' algorithms.
//!
//! M0 ships perceptual hashing ([`phash`]) and thumbnail encoding
//! ([`thumbnail`]). Shot boundaries, VAD, and embeddings arrive in M1. These
//! functions are synchronous and must be called from the rayon pool
//! (`vi_core::cpu::run`), never from a tokio worker thread.

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod phash;
pub mod thumbnail;

pub use phash::{hamming, phash_rgb, PHASH_DEDUP_DISTANCE};
pub use thumbnail::{encode_webp_thumbnail, ThumbnailError};
