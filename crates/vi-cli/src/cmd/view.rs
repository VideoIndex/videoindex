//! `vidx view <index-dir> <video-id> --t0 .. --t1 .. [--fps ..] -o grid.png`

use std::path::PathBuf;

use anyhow::{Context, Result};
use vi_agent::{render_view, ViewRequest};
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
    /// Start, seconds.
    #[arg(long)]
    pub t0: f64,
    /// End, seconds.
    #[arg(long)]
    pub t1: f64,
    /// Frames per second (at most 16 frames per grid).
    #[arg(long, default_value_t = 1.0)]
    pub fps: f64,
    /// Grid columns.
    #[arg(long, default_value_t = 3)]
    pub cols: u32,
    /// Output PNG.
    #[arg(short, long, default_value = "grid.png")]
    pub out: PathBuf,
}

pub async fn run(args: Args, config: &Config, out: &Output) -> Result<()> {
    let idx = EmbeddedIndex::open(&args.index_dir)
        .with_context(|| format!("opening index at {}", args.index_dir.display()))?;
    let video = idx
        .get_video(args.video_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no video {} in this index", args.video_id))?;
    let req = ViewRequest {
        t0: args.t0,
        t1: args.t1,
        fps: args.fps,
        cols: args.cols,
        ..ViewRequest::default()
    };
    let started = std::time::Instant::now();
    let view = render_view(&config.media.worker, &video, req).await?;
    std::fs::write(&args.out, &view.png)?;
    let ms = started.elapsed().as_millis();
    out.emit(
        &serde_json::json!({
            "out": args.out, "width": view.width, "height": view.height,
            "frames": view.timestamps.len(), "distinct": view.distinct,
            "timestamps": view.timestamps, "ms": ms,
        }),
        || {
            format!(
                "wrote {} ({}x{}, {} frames, {} distinct) in {ms} ms",
                args.out.display(),
                view.width,
                view.height,
                view.timestamps.len(),
                view.distinct
            )
        },
    );
    Ok(())
}
