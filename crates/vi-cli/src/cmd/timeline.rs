//! `vi timeline <index-dir> <video-id> [--level chapter|scene|shot]`

use std::path::PathBuf;

use anyhow::{Context, Result};
use vi_core::model::SegmentLevel;
use vi_core::{Config, VideoId};
use vi_index::{EmbeddedIndex, Storage};

use crate::output::Output;

/// Arguments.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Index directory.
    pub index_dir: PathBuf,
    /// Video id.
    pub video_id: VideoId,
    /// Segment level.
    #[arg(long, default_value = "chapter")]
    pub level: String,
}

pub async fn run(args: Args, _config: &Config, out: &Output) -> Result<()> {
    let idx = EmbeddedIndex::open(&args.index_dir)
        .with_context(|| format!("opening index at {}", args.index_dir.display()))?;
    let level = SegmentLevel::parse(&args.level)
        .ok_or_else(|| anyhow::anyhow!("level must be chapter, scene or shot"))?;
    let video = idx
        .get_video(args.video_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no video {} in this index", args.video_id))?;
    let segs = idx.segments(args.video_id, level).await?;
    out.emit(&segs, || {
        let mut s = format!(
            "{}  {}  {} {} segment(s)\n",
            video.title.as_deref().unwrap_or("(untitled)"),
            video.duration,
            segs.len(),
            level.as_str()
        );
        for seg in &segs {
            s.push_str(&format!(
                "  {} - {}  {}\n",
                seg.t0,
                seg.t1,
                seg.title
                    .as_deref()
                    .or(seg.summary.as_deref())
                    .unwrap_or("")
            ));
        }
        s.trim_end().to_string()
    });
    Ok(())
}
