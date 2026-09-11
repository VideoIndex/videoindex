//! `vi probe <file>`

use std::path::PathBuf;

use anyhow::Result;
use vi_core::Config;

use crate::output::{bytes, Output};

/// Arguments.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Media file.
    pub file: PathBuf,
}

pub async fn run(args: Args, config: &Config, out: &Output) -> Result<()> {
    let probe = vi_media::probe(&config.media.worker, &args.file).await?;
    out.emit(&probe, || {
        let mut s = String::new();
        s.push_str(&format!(
            "{}\n  format: {} ({})\n  duration: {}  size: {}  bitrate: {} kb/s\n",
            probe.path,
            probe.format_name,
            probe.format_long_name,
            probe.duration,
            bytes(probe.size_bytes),
            probe.bit_rate / 1000
        ));
        if let Some(kf) = probe.keyframe_interval_secs {
            s.push_str(&format!("  keyframe interval: ~{kf:.2} s\n"));
        }
        if let Some(t) = probe.title() {
            s.push_str(&format!("  title: {t}\n"));
        }
        for st in &probe.streams {
            s.push_str(&format!(
                "  stream #{} {:?} {}",
                st.index, st.kind, st.codec
            ));
            if let (Some(w), Some(h)) = (st.width, st.height) {
                s.push_str(&format!(" {w}x{h}"));
            }
            if let Some(f) = st.fps.or(st.avg_fps) {
                s.push_str(&format!(" {f:.3} fps"));
            }
            if let Some(r) = st.sample_rate {
                s.push_str(&format!(" {r} Hz"));
            }
            if let Some(c) = st.channels {
                s.push_str(&format!(" {c} ch"));
            }
            if let Some(l) = &st.language {
                s.push_str(&format!(" [{l}]"));
            }
            s.push_str(&format!(
                " tb {}/{}{}\n",
                st.time_base_num,
                st.time_base_den,
                if st.is_default { " default" } else { "" }
            ));
        }
        if !probe.chapters.is_empty() {
            s.push_str(&format!("  chapters: {}\n", probe.chapters.len()));
            for c in probe.chapters.iter().take(20) {
                s.push_str(&format!(
                    "    {} - {}  {}\n",
                    c.t0,
                    c.t1,
                    c.title.as_deref().unwrap_or("")
                ));
            }
        }
        s.push_str(&format!("  libav: {}", probe.libav));
        s
    });
    Ok(())
}
