//! Shipped operators and the registry that maps policy names to them.

use vi_core::config::Config;

use crate::operator::Operator;

pub mod phash;
pub mod sample;
pub mod thumbnail;

pub use phash::PHash;
pub use sample::Sample;
pub use thumbnail::Thumbnail;

/// Operators this build knows how to construct.
pub const AVAILABLE: &[&str] = &["sample", "phash", "thumbnail"];

/// Operators named in the design but not yet implemented; listing them lets
/// error messages distinguish "not yet" from "typo".
pub const PLANNED: &[&str] = &[
    "vad",
    "asr",
    "shot_boundary",
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
        "sample" => Some(Box::new(Sample::new())),
        "phash" => Some(Box::new(PHash::new())),
        "thumbnail" => Some(Box::new(Thumbnail::new(
            config.index.thumbnail_px,
            config.index.thumbnail_quality,
        ))),
        _ => None,
    }
}
