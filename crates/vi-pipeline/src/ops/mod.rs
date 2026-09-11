//! Shipped operators and the registry that maps policy names to them.

use vi_core::config::Config;

use crate::operator::Operator;

pub mod asr;
pub mod chapters;
pub mod entities_events;
pub mod image_embed;
pub mod ocr;
pub mod phash;
pub mod sample;
pub mod scenes;
pub mod shot_boundary;
pub mod subtitle_import;
pub mod text_embed;
pub mod thumbnail;
pub mod vad;
pub mod vlm_describe;

pub use asr::Asr;
pub use chapters::Chapters;
pub use entities_events::EntitiesEvents;
pub use image_embed::ImageEmbed;
pub use ocr::Ocr;
pub use phash::PHash;
pub use sample::Sample;
pub use scenes::Scenes;
pub use shot_boundary::ShotBoundary;
pub use subtitle_import::SubtitleImport;
pub use text_embed::TextEmbed;
pub use thumbnail::Thumbnail;
pub use vad::Vad;
pub use vlm_describe::VlmDescribe;

/// Operators this build knows how to construct.
pub const AVAILABLE: &[&str] = &[
    "subtitle_import",
    "sample",
    "phash",
    "thumbnail",
    "vad",
    "asr",
    "shot_boundary",
    "image_embed",
    "text_embed",
    "ocr",
    "scenes",
    "chapters",
    "vlm_describe",
    "entities_events",
];

/// Operators named in the design but not yet implemented; listing them lets
/// error messages distinguish "not yet" from "typo".
pub const PLANNED: &[&str] = &["diarize"];

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
        "image_embed" => Some(Box::new(ImageEmbed::new())),
        "text_embed" => Some(Box::new(TextEmbed::new())),
        "ocr" => Some(Box::new(Ocr::new())),
        "scenes" => Some(Box::new(Scenes::new())),
        "chapters" => Some(Box::new(Chapters::new())),
        "vlm_describe" => Some(Box::new(VlmDescribe::new())),
        "entities_events" => Some(Box::new(EntitiesEvents::new())),
        _ => None,
    }
}
