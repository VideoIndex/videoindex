//! Measure decode throughput through the worker: `decode_bench <file> [fps] [max_dim]`.
#![allow(clippy::unwrap_used)]
use std::time::Instant;
use vi_core::config::WorkerConfig;
use vi_media::VideoDecodeRequest;

#[tokio::main]
async fn main() {
    let path = std::env::args().nth(1).unwrap();
    let fps: f64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    let max_dim: u32 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(640);
    let cfg = WorkerConfig {
        path: std::env::var_os("VI_MEDIA_WORKER").map(std::path::PathBuf::from),
        ..WorkerConfig::default()
    };
    let start = Instant::now();
    let mut s = vi_media::decode_video(&cfg, VideoDecodeRequest::new(&path, fps, max_dim))
        .await
        .unwrap();
    println!("info: {:?}", s.info());
    let mut n = 0u64;
    let mut sum = 0u64;
    while let Some(f) = s.next().await.unwrap() {
        n += 1;
        sum += f.data()[0] as u64;
        if n % 600 == 0 {
            eprintln!(
                "{n} frames, t={} elapsed {:.1}s",
                f.t,
                start.elapsed().as_secs_f64()
            );
        }
    }
    let el = start.elapsed().as_secs_f64();
    println!(
        "{n} frames in {el:.1}s ({:.1} fps delivered), stats={:?}, checksum {sum}",
        n as f64 / el,
        s.stats()
    );
}
