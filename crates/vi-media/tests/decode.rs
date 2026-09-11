//! End-to-end tests of the worker: probe, sequential and seeking video
//! decode, audio chunking. They run the real worker binary built from this
//! crate, so a libav crash would fail the test rather than the test process.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use vi_core::config::WorkerConfig;
use vi_media::{AudioDecodeRequest, PixelFormat, VideoDecodeRequest};
use vi_testkit as fx;

fn cfg() -> WorkerConfig {
    WorkerConfig {
        path: Some(PathBuf::from(env!("CARGO_BIN_EXE_vi-media-worker"))),
        timeout_secs: 120,
        ..WorkerConfig::default()
    }
}

fn probe_color(frame: &vi_media::FrameBuffer) -> [u8; 3] {
    let (x, y) = fx::BACKGROUND_PROBE_XY;
    let sx = x * frame.width / fx::WIDTH;
    let sy = y * frame.height / fx::HEIGHT;
    frame.rgb_at(sx, sy).unwrap()
}

#[tokio::test]
async fn probe_reports_streams_and_duration() {
    let p = vi_media::probe(&cfg(), &fx::fixture_path()).await.unwrap();
    assert!(
        (p.duration.as_secs_f64() - fx::DURATION_SECS).abs() < 0.6,
        "{p:?}"
    );
    let v = p.video_stream().unwrap();
    assert_eq!(v.codec, "h264");
    assert_eq!((v.width, v.height), (Some(fx::WIDTH), Some(fx::HEIGHT)));
    assert!((v.fps.unwrap() - fx::FPS).abs() < 0.01);
    let a = p.audio_stream().unwrap();
    assert_eq!(a.sample_rate, Some(fx::AUDIO_RATE));
    let kf = p.keyframe_interval_secs.unwrap();
    assert!((0.5..=10.0).contains(&kf), "keyframe interval {kf}");
    assert!(p.libav.starts_with("avformat"));
    assert!(p.size_bytes > 0);
}

#[tokio::test]
async fn probe_of_garbage_is_an_error_not_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.mp4");
    std::fs::write(&bad, b"definitely not a video file").unwrap();
    let r = vi_media::probe(&cfg(), &bad).await;
    assert!(r.is_err());
    // The worker must still be usable for the next request.
    vi_media::probe(&cfg(), &fx::fixture_path()).await.unwrap();
}

#[tokio::test]
async fn sequential_decode_at_1fps_covers_the_file_with_correct_colors() {
    let req = VideoDecodeRequest::new(fx::fixture_path(), 1.0, 320);
    let mut stream = vi_media::decode_video(&cfg(), req).await.unwrap();
    assert!(!stream.info().seeking);
    assert_eq!((stream.info().width, stream.info().height), (320, 180));
    let mut frames = Vec::new();
    while let Some(f) = stream.next().await.unwrap() {
        assert!(f.is_shared());
        assert_eq!(f.format, PixelFormat::Rgb24);
        assert_eq!((f.width, f.height), (320, 180));
        assert_eq!(f.stride, 320 * 3);
        frames.push(f.to_owned_frame());
    }
    let n = frames.len();
    assert!((118..=121).contains(&n), "got {n} frames");
    let stats = stream.stats().unwrap();
    assert_eq!(stats.items as usize, n);
    // Non-reference frames were skipped, so fewer than all 3600 frames were decoded.
    assert!(stats.decoded < 3600, "decoded {}", stats.decoded);
    let mut prev = -1.0;
    for f in &frames {
        let t = f.t.as_secs_f64();
        assert!(t > prev, "timestamps must increase");
        if prev >= 0.0 {
            let gap = t - prev;
            assert!((0.8..=1.25).contains(&gap), "gap {gap} at {t}");
        }
        prev = t;
        let expected = fx::color_at(t);
        let actual = probe_color(f);
        assert!(
            fx::color_close(actual, expected, 16),
            "at t={t}: got {actual:?}, expected {expected:?}"
        );
    }
}

#[tokio::test]
async fn range_decode_seeks_to_the_right_place() {
    let req = VideoDecodeRequest::new(fx::fixture_path(), 1.0, 320).range(45.0, Some(55.0));
    let frames = vi_media::decode_video(&cfg(), req)
        .await
        .unwrap()
        .collect_owned()
        .await
        .unwrap();
    assert!((9..=11).contains(&frames.len()), "got {}", frames.len());
    let first = frames[0].t.as_secs_f64();
    assert!((44.9..=45.6).contains(&first), "first frame at {first}");
    for f in &frames {
        let t = f.t.as_secs_f64();
        assert!(t < 55.1, "frame past range at {t}");
        assert!(
            fx::color_close(probe_color(f), fx::color_at(t), 16),
            "at {t}"
        );
    }
}

#[tokio::test]
async fn sparse_sampling_uses_keyframe_seeking_and_lands_near_targets() {
    // One frame every 25 s: interval far above the 2 s GOP, so the worker
    // seeks per target instead of decoding everything.
    let req = VideoDecodeRequest::new(fx::fixture_path(), 1.0 / 25.0, 320);
    let mut stream = vi_media::decode_video(&cfg(), req).await.unwrap();
    assert!(stream.info().seeking);
    let mut frames = Vec::new();
    while let Some(f) = stream.next().await.unwrap() {
        frames.push(f.to_owned_frame());
    }
    let ts: Vec<f64> = frames.iter().map(|f| f.t.as_secs_f64()).collect();
    assert_eq!(ts.len(), 5, "{ts:?}"); // 0, 25, 50, 75, 100
    for (i, t) in ts.iter().enumerate() {
        let target = i as f64 * 25.0;
        assert!(
            *t >= target - 0.05 && *t <= target + 0.6,
            "target {target} got {t}"
        );
        assert!(fx::color_close(
            probe_color(&frames[i]),
            fx::color_at(*t),
            16
        ));
    }
    // Seeking decodes only from the keyframe before each target.
    assert!(stream.stats().unwrap().decoded < 5 * 90);
}

#[tokio::test]
async fn frames_stay_valid_while_held_and_backpressure_does_not_deadlock() {
    let req = VideoDecodeRequest::new(fx::fixture_path(), 2.0, 160).range(0.0, Some(30.0));
    let mut stream = vi_media::decode_video(&cfg(), req).await.unwrap();
    let mut held = Vec::new();
    let mut count = 0;
    while let Some(f) = stream.next().await.unwrap() {
        count += 1;
        // Hold up to 8 shared frames (fewer than the 16 slots) at once.
        if held.len() < 8 {
            held.push(f);
        } else {
            let old: std::sync::Arc<vi_media::FrameBuffer> = held.remove(0);
            // The oldest frame's pixels must still be intact.
            assert!(fx::color_close(
                probe_color(&old),
                fx::color_at(old.t.as_secs_f64()),
                16
            ));
            held.push(f);
        }
    }
    assert!((58..=61).contains(&count), "got {count}");
}

#[tokio::test]
async fn dropping_a_stream_early_stops_the_worker() {
    let req = VideoDecodeRequest::new(fx::fixture_path(), 30.0, 320);
    let mut stream = vi_media::decode_video(&cfg(), req).await.unwrap();
    let f = stream.next().await.unwrap().unwrap();
    drop(stream);
    // The frame outlives the stream and is still readable.
    assert_eq!(f.width, 320);
    let _ = probe_color(&f);
}

#[tokio::test]
async fn audio_decodes_to_16k_mono_chunks() {
    let req = AudioDecodeRequest::new(fx::fixture_path());
    let chunks = vi_media::decode_audio(&cfg(), req)
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    // 30 s chunks with 1 s overlap over 120 s: starts at 0, 29, 58, 87 and a
    // 4 s tail at 116.
    assert_eq!(
        chunks.len(),
        5,
        "{:?}",
        chunks.iter().map(|c| c.t0.to_string()).collect::<Vec<_>>()
    );
    for (i, c) in chunks.iter().enumerate() {
        assert_eq!(c.sample_rate, 16_000);
        let t0 = c.t0.as_secs_f64();
        assert!(
            (t0 - i as f64 * 29.0).abs() < 0.1,
            "chunk {i} starts at {t0}"
        );
        if i < 4 {
            assert_eq!(c.samples.len(), 480_000);
        } else {
            assert!(c.samples.len() > 16_000 && c.samples.len() < 480_000);
        }
        let rms = (c.samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>()
            / c.samples.len() as f64)
            .sqrt();
        assert!(rms > 500.0, "chunk {i} rms {rms}");
        // Dominant frequency: count zero crossings ≈ 2 * 440 * seconds.
        let secs = c.samples.len() as f64 / 16_000.0;
        let crossings = c
            .samples
            .windows(2)
            .filter(|w| (w[0] < 0) != (w[1] < 0))
            .count() as f64;
        let hz = crossings / secs / 2.0;
        assert!((hz - fx::TONE_HZ).abs() < 5.0, "chunk {i} ~{hz} Hz");
    }
}

#[tokio::test]
async fn audio_range_and_missing_stream_errors() {
    let req = AudioDecodeRequest {
        t0_secs: 100.0,
        t1_secs: Some(110.0),
        ..AudioDecodeRequest::new(fx::fixture_path())
    };
    let chunks = vi_media::decode_audio(&cfg(), req)
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(chunks.len(), 1);
    let n = chunks[0].samples.len();
    assert!((150_000..=170_000).contains(&n), "{n} samples");
    assert!((chunks[0].t0.as_secs_f64() - 100.0).abs() < 0.2);

    let bad = VideoDecodeRequest {
        stream_index: Some(99),
        ..VideoDecodeRequest::new(fx::fixture_path(), 1.0, 320)
    };
    assert!(vi_media::decode_video(&cfg(), bad).await.is_err());
}
