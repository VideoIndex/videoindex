//! Live S0.5 measurement: how long it takes to open a freshly written
//! segment with the existing per-request `decode_video` and get its first
//! frame. Decides whether C2's `decode_live` can open segments one by one.
//!
//! Replays the segmented fixture with `vi_testkit::PacedWriter` at `rate`
//! (default 1x, so 120 s), watches `index.json`, and for each new segment
//! records two intervals: from the segment file's modification time (the
//! copy finished, a rename follows immediately) to detection, and from
//! detection to the first frame delivered at 1 fps. Prints p50 and p95.
//!
//! ```sh
//! cargo build -p vi-media --bin vi-media-worker
//! cargo run -p vi-media --example segment_open_latency -- [rate] [poll_ms]
//! ```
#![allow(clippy::unwrap_used)]

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use vi_core::config::WorkerConfig;
use vi_media::{SegmentIndex, VideoDecodeRequest};
use vi_testkit::PacedWriter;

fn worker_path() -> PathBuf {
    if let Some(p) = std::env::var_os("VI_MEDIA_WORKER") {
        return PathBuf::from(p);
    }
    // target/<profile>/examples/<this> -> target/<profile>/vi-media-worker
    let exe = std::env::current_exe().unwrap();
    exe.parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("vi-media-worker"))
        .unwrap()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

#[tokio::main]
async fn main() {
    let rate: f64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    let poll_ms: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let cfg = WorkerConfig {
        path: Some(worker_path()),
        ..WorkerConfig::default()
    };
    assert!(
        cfg.path.as_ref().unwrap().is_file(),
        "worker binary missing at {}; build it first",
        cfg.path.as_ref().unwrap().display()
    );
    let tmp = tempfile::tempdir().unwrap();
    let store = tmp.path().join("store");
    let writer = PacedWriter::new(&store, rate).unwrap();
    let index_path = SegmentIndex::path_in(&store);
    eprintln!(
        "replaying the segmented fixture at {rate}x into {} (poll {poll_ms} ms)",
        store.display()
    );

    let mut seen = HashSet::new();
    let mut detect_ms = Vec::new();
    let mut open_ms = Vec::new();
    let mut total_ms = Vec::new();
    let started = Instant::now();
    loop {
        let done = writer.is_done();
        let index = match SegmentIndex::load(&index_path) {
            Ok(i) => i,
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(poll_ms)).await;
                continue;
            }
        };
        for entry in &index.segments {
            if !seen.insert(entry.seq) {
                continue;
            }
            let detected = Instant::now();
            let detected_wall = SystemTime::now();
            let path = store.join(&entry.file);
            let written = std::fs::metadata(&path).unwrap().modified().unwrap();
            let since_write = detected_wall
                .duration_since(written)
                .unwrap_or_default()
                .as_secs_f64()
                * 1000.0;
            // No range: the segment's own PTS already start at `t0`
            // (`-reset_timestamps 0`), so the first frame's `t` shows
            // whether libav preserves the recording's timeline.
            let mut stream = vi_media::decode_video(&cfg, VideoDecodeRequest::new(&path, 1.0, 320))
                .await
                .unwrap();
            let first = stream.next().await.unwrap();
            let opened = detected.elapsed().as_secs_f64() * 1000.0;
            let t = first.map(|f| f.t.as_secs_f64());
            drop(stream);
            detect_ms.push(since_write);
            open_ms.push(opened);
            total_ms.push(since_write + opened);
            println!(
                "seq {:3}  t0 {:7.3}s  write->detect {:6.1} ms  detect->first frame {:6.1} ms  first frame t={:?}  wall {:6.1}s",
                entry.seq,
                entry.t0.as_secs_f64(),
                since_write,
                opened,
                t,
                started.elapsed().as_secs_f64()
            );
        }
        if done && seen.len() >= index.segments.len() && index.segments.len() >= 60 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(poll_ms)).await;
    }
    writer.finish().unwrap();

    for (name, v) in [
        ("write->detect", &mut detect_ms),
        ("detect->first frame (decode_video open)", &mut open_ms),
        ("write->first frame", &mut total_ms),
    ] {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{name}: n={} p50 {:.1} ms  p95 {:.1} ms  max {:.1} ms",
            v.len(),
            percentile(v, 0.5),
            percentile(v, 0.95),
            v.last().copied().unwrap_or(f64::NAN)
        );
    }
}
