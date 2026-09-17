//! `vidx status <index-dir>`

use std::path::PathBuf;

use anyhow::{Context, Result};
use vi_core::Config;
use vi_index::{EmbeddedIndex, Storage};

use crate::output::{bytes, Output};

/// Arguments.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Index directory.
    pub index_dir: PathBuf,
}

pub async fn run(args: Args, _config: &Config, out: &Output) -> Result<()> {
    let idx = EmbeddedIndex::open(&args.index_dir)
        .with_context(|| format!("opening index at {}", args.index_dir.display()))?;
    let stats = idx.stats().await?;
    out.emit(&stats, || {
        let mut s = String::new();
        s.push_str(&format!(
            "{}\n  index {}  schema v{}  created {}\n  size: {} total  ({} sqlite, {} in {} blobs)\n  videos: {}\n",
            stats.path,
            stats.manifest.index_id,
            stats.manifest.schema_version,
            stats.manifest.created_at.format("%Y-%m-%d %H:%M UTC"),
            bytes(stats.dir_bytes),
            bytes(stats.sqlite_bytes),
            bytes(stats.blob_bytes),
            stats.blob_count,
            stats.videos.len()
        ));
        for v in &stats.videos {
            s.push_str(&format!(
                "\n  {}  {:<9} {}\n    {}\n    duration {}  tracks {}  samples {} (hashed {}, thumbnails {})",
                v.video.id,
                v.video.index_state.as_str(),
                v.video.title.as_deref().unwrap_or("(untitled)"),
                v.video.source_uri,
                v.video.duration,
                v.tracks,
                v.frame_samples,
                v.hashed,
                v.thumbnails
            ));
            if v.segments + v.transcript_spans + v.ocr_spans + v.descriptions > 0 {
                s.push_str(&format!(
                    "\n    segments {}  transcript spans {}  ocr spans {}  descriptions {}",
                    v.segments, v.transcript_spans, v.ocr_spans, v.descriptions
                ));
            }
            if v.cost_usd > 0.0 {
                s.push_str(&format!("\n    cost ${:.4}", v.cost_usd));
            }
            s.push('\n');
        }
        if !stats.jobs.is_empty() {
            s.push_str(&format!("\n  jobs: {}\n", stats.jobs.len()));
            for j in stats.jobs.iter().rev().take(10) {
                let done = j
                    .stages
                    .values()
                    .filter(|st| st.status == vi_core::model::StageStatus::Complete)
                    .count();
                s.push_str(&format!(
                    "    {}  {}  policy {}  {}/{} stages  {}\n",
                    j.job_id,
                    if j.finished { "finished" } else { "running " },
                    j.policy,
                    done,
                    j.stages.len(),
                    j.updated_at.format("%Y-%m-%d %H:%M:%S")
                ));
            }
        }
        s.trim_end().to_string()
    });
    Ok(())
}
