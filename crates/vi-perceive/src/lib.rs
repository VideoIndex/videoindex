//! `vi-perceive`: CPU-bound, in-process perception operators' algorithms.
//!
//! M0 shipped perceptual hashing ([`phash`]) and thumbnail encoding
//! ([`thumbnail`]). M1 adds the ONNX Runtime session wrapper ([`onnx`]),
//! Silero voice activity detection ([`vad`]), shot boundary detection
//! ([`shot`]), SigLIP image-text embeddings ([`siglip`]), a BERT-style text
//! embedder ([`bge`]) and RapidOCR, exposed to the pipeline as the
//! `onnx_local` provider adapter ([`onnx_local`]). These functions are synchronous and must be called from the
//! rayon pool (`vi_core::cpu::run`), never from a tokio worker thread.

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod bge;
pub mod onnx;
pub mod onnx_local;
pub mod phash;
pub mod shot;
pub mod siglip;
pub mod thumbnail;
pub mod vad;

pub use onnx::{Device, OnnxSession};
pub use onnx_local::OnnxLocal;
pub use phash::{hamming, phash_rgb, PHASH_DEDUP_DISTANCE};
pub use shot::{FrameSignature, ShotDetector, ShotParams};
pub use thumbnail::{encode_webp_thumbnail, ThumbnailError};
pub use vad::{SpeechSegment, Vad};

/// Errors from perception code.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PerceiveError {
    /// A model file is not where the config says.
    #[error("model file missing: {path} ({hint})")]
    ModelMissing {
        /// Expected path.
        path: String,
        /// How to get it.
        hint: String,
    },
    /// ONNX Runtime failed.
    #[error("onnx runtime: {0}")]
    Onnx(String),
    /// Bad input (wrong pixel format, empty audio, unsupported size).
    #[error("invalid input: {0}")]
    Invalid(String),
    /// Filesystem.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl From<PerceiveError> for vi_core::Error {
    fn from(e: PerceiveError) -> Self {
        match e {
            PerceiveError::Invalid(m) => vi_core::Error::Invalid(m),
            PerceiveError::Io(e) => vi_core::Error::Io(e),
            other => vi_core::Error::Provider(other.to_string()),
        }
    }
}
