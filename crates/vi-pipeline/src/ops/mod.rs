//! Shipped operators and the registry that maps policy names to them.

use vi_core::config::Config;

use crate::operator::Operator;

pub mod asr;
pub mod phash;
pub mod sample;
pub mod shot_boundary;
pub mod subtitle_import;
pub mod thumbnail;
pub mod vad;

pub use asr::Asr;
pub use phash::PHash;
pub use sample::Sample;
pub use shot_boundary::ShotBoundary;
pub use subtitle_import::SubtitleImport;
pub use thumbnail::Thumbnail;
pub use vad::Vad;

/// Operators this build knows how to construct.
pub const AVAILABLE: &[&str] = &[
    "subtitle_import",
    "sample",
    "phash",
    "thumbnail",
    "vad",
    "asr",
    "shot_boundary",
];

/// Operators named in the design but not yet implemented; listing them lets
/// error messages distinguish "not yet" from "typo".
pub const PLANNED: &[&str] = &[
    "image_embed",
    "ocr",
    "scenes",
    "chapters",
    "vlm_describe",
    "entities_events",
    "text_embed",
];

/// Construct an operator by policy name.
pub fn build(name: &str, config: &Config) -> Option<Box<dyn Operator>> {
    match name {
        "subtitle_import" => Some(Box::new(SubtitleImport::new())),
        "sample" => Some(Box::new(Sample::new())),
        "phash" => Some(Box::new(PHash::new())),
        "thumbnail" => Some(Box::new(Thumbnail::new(
            config.index.thumbnail_px,
            config.index.thumbnail_quality,
        ))),
        "vad" => Some(Box::new(Vad::new())),
        "asr" => Some(Box::new(Asr::new())),
        "shot_boundary" => Some(Box::new(ShotBoundary::new())),
        _ => None,
    }
}
