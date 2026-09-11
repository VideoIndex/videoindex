//! `vi index <index-dir> <source>... [--policy P]`

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use tokio_util::sync::CancellationToken;
use vi_core::{Config, Event};
use vi_index::EmbeddedIndex;
use vi_media::Source;
use vi_pipeline::{JobOptions, JobReport, Scheduler};

use crate::output::Output;

/// Arguments.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Index directory (created if missing).
    pub index_dir: PathBuf,
    /// Local video files. Shell globs expand before `vi` sees them.
    #[arg(required = true)]
    pub sources: Vec<String>,
    /// Policy name from the config (`m0`, `coarse_only`, `lecture_default`, ...).
    #[arg(long)]
    pub policy: Option<String>,
    /// Re-index even if the video is already indexed.
    #[arg(long)]
    pub force: bool,
    /// Resume an unfinished earlier job for the same video when possible.
    #[arg(long)]
    pub resume: bool,
    /// Override the sampling rate of the policy (frames per second).
    #[arg(long)]
    pub fps: Option<f64>,
    /// Vacuum and refresh manifest hashes when done.
    #[arg(long)]
    pub compact: bool,
}

pub async fn run(args: Args, mut config: Config, out: &Output) -> Result<()> {
    if let Some(fps) = args.fps {
        if fps <= 0.0 {
            bail!("--fps must be > 0");
        }
        let name = args
            .policy
            .clone()
            .unwrap_or_else(|| config.default_policy.clone());
        let p = config
            .policy
            .get_mut(&name)
            .ok_or_else(|| anyhow::anyhow!("unknown policy '{name}'"))?;
        p.sample_fps = fps;
    }
    let config = Arc::new(config);
    let idx = Arc::new(
        EmbeddedIndex::open_or_create(&args.index_dir)
            .with_context(|| format!("opening index at {}", args.index_dir.display()))?,
    );
    let events = vi_core::EventBus::default();
    let mut rx = events.subscribe();
    let sched = Scheduler::new(idx.clone(), config, events);
    // Validate the policy before touching any media.
    let (policy_name, _, dag) = sched.plan(args.policy.as_deref())?;
    tracing::info!(policy = %policy_name, stages = ?dag.stage_names(), "plan");

    let cancel = CancellationToken::new();
    {
        let c = cancel.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                eprintln!("\ninterrupted; stopping after the current frames");
                c.cancel();
            }
        });
    }

    // Progress printer.
    let printer_out = *out;
    let printer = tokio::spawn(async move {
        let mut last_line = String::new();
        let mut last_print = Instant::now();
        while let Ok(ev) = rx.recv().await {
            match &ev {
                Event::Progress(p) => {
                    if printer_out.json() {
                        printer_out.event(&ev, String::new);
                    } else if last_print.elapsed().as_millis() >= 500 {
                        last_print = Instant::now();
                        let line = format!(
                            "  {:<10} {:>6.1}%  {}{}",
                            p.stage,
                            p.fraction * 100.0,
                            p.items_done,
                            p.items_total.map(|t| format!("/{t}")).unwrap_or_default()
                        );
                        if line != last_line {
                            eprintln!("{line}");
                            last_line = line;
                        }
                    }
                }
                Event::JobStarted { source, .. } => {
                    printer_out.event(&ev, || format!("indexing {source}"));
                }
                Event::StageFinished {
                    stage,
                    items,
                    elapsed_ms,
                    ..
                } => {
                    printer_out.event(&ev, || {
                        format!(
                            "  {stage:<10} done: {items} items in {:.1}s",
                            *elapsed_ms as f64 / 1000.0
                        )
                    });
                }
                Event::StageFailed { stage, error, .. } => {
                    printer_out.event(&ev, || format!("  {stage:<10} FAILED: {error}"));
                }
                Event::JobFinished { summary, ok, .. } => {
                    printer_out.event(&ev, || {
                        format!("{} {summary}", if *ok { "finished:" } else { "failed:" })
                    });
                }
                _ => {}
            }
        }
    });

    let mut reports: Vec<JobReport> = Vec::new();
    let mut failures = 0usize;
    let started = Instant::now();
    for s in &args.sources {
        if cancel.is_cancelled() {
            break;
        }
        let source = Source::parse(s);
        let opts = JobOptions {
            policy: args.policy.clone(),
            resume: args.resume,
            force: args.force,
        };
        match sched.run(source, opts, cancel.clone()).await {
            Ok(r) => {
                if !r.ok {
                    failures += 1;
                }
                reports.push(r);
            }
            Err(e) => {
                failures += 1;
                out.event(
                    &serde_json::json!({"type": "error", "source": s, "error": e.to_string()}),
                    || format!("error indexing {s}: {e}"),
                );
                if matches!(e, vi_core::Error::Cancelled) {
                    break;
                }
            }
        }
    }
    if args.compact {
        use vi_index::Storage;
        idx.compact().await?;
    }
    drop(sched);
    printer.abort();

    let elapsed = started.elapsed().as_secs_f64();
    out.emit(&reports, || {
        let mut s = String::new();
        for r in &reports {
            let samples = r.stages.get("sample").map(|st| st.items_done).unwrap_or(0);
            s.push_str(&format!(
                "video {}  {}  {} samples  {:.1}s{}\n",
                r.video_id,
                if r.ok {
                    format!("{:?}", r.index_state).to_lowercase()
                } else {
                    "FAILED".into()
                },
                samples,
                r.elapsed_secs,
                if r.skipped {
                    "  (already indexed; use --force)"
                } else {
                    ""
                }
            ));
        }
        s.push_str(&format!(
            "{} source(s), {} failed, {elapsed:.1}s total",
            args.sources.len(),
            failures
        ));
        s
    });
    if failures > 0 {
        bail!("{failures} source(s) failed");
    }
    Ok(())
}
