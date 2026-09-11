//! Print per-sample shot metrics for a video: `shot_debug <file> [t0] [t1]`.
//! Columns: time, histogram distance, edge change ratio, pixel change
//! (fraction over 24 levels), combined distance, and `CUT` where the
//! detector fires. Set `VI_MEDIA_WORKER` to the worker binary.
#![allow(clippy::unwrap_used)]
use vi_core::config::WorkerConfig;
use vi_media::VideoDecodeRequest;
use vi_perceive::shot::{FrameSignature, ShotDetector, ShotParams};

#[tokio::main]
async fn main() {
    let path = std::env::args().nth(1).unwrap();
    let t0: f64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let t1: Option<f64> = std::env::args().nth(3).and_then(|s| s.parse().ok());
    let cfg = WorkerConfig {
        path: std::env::var_os("VI_MEDIA_WORKER").map(std::path::PathBuf::from),
        ..WorkerConfig::default()
    };
    let req = VideoDecodeRequest::new(&path, 1.0, 640).range(t0, t1);
    let mut s = vi_media::decode_video(&cfg, req).await.unwrap();
    let mut det = ShotDetector::new(ShotParams::default());
    let mut prev: Option<FrameSignature> = None;
    while let Some(f) = s.next().await.unwrap() {
        let sig = FrameSignature::from_frame(&f).unwrap();
        if let Some(p) = &prev {
            let h = p.hist_distance(&sig);
            let e = p.edge_change_ratio(&sig);
            let px = p.pixel_change(&sig, 24.0);
            let d = p.distance(&sig);
            let cut = det.push(sig.clone());
            println!(
                "{:>8.1}  hist {h:.3}  ecr {e:.3}  px {px:.3}  d {d:.3}{}",
                f.t.as_secs_f64(),
                if cut { "  CUT" } else { "" }
            );
        } else {
            det.push(sig.clone());
        }
        prev = Some(sig);
    }
}
