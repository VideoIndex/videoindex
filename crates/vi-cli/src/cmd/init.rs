//! `vidx init <index-dir>`

use std::path::PathBuf;

use anyhow::{Context, Result};
use vi_core::Config;
use vi_index::{EmbeddedIndex, Storage};

use crate::output::Output;

/// Arguments.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Index directory to create (conventionally `*.vidx`).
    pub index_dir: PathBuf,
}

pub async fn run(args: Args, _config: &Config, out: &Output) -> Result<()> {
    let idx = EmbeddedIndex::create(&args.index_dir)
        .with_context(|| format!("creating index at {}", args.index_dir.display()))?;
    let manifest = idx.manifest().await?;
    out.emit(&manifest, || {
        format!(
            "created {} (index {}, schema v{})",
            args.index_dir.display(),
            manifest.index_id,
            manifest.schema_version
        )
    });
    Ok(())
}
