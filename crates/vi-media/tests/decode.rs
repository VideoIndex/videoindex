//! End-to-end tests of the worker: probe, sequential and seeking video
//! decode, audio chunking. They run the real worker binary built from this
//! crate, so a libav crash would fail the test rather than the test process.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use vi_core::config::WorkerConfig;
use vi_core::Timestamp;
use vi_media::{
    AudioDecodeRequest, LiveDecodeRequest, LiveItem, MediaInput, PixelFormat, SegmentFeed,
    SegmentGap, SegmentIndex, VideoDecodeRequest,
};
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
        (p.duration.unwrap().as_secs_f64() - fx::DURATION_SECS).abs() < 0.6,
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
async fn probe_over_a_segment_feed_reads_index_and_first_segment() {
    let dir = fx::fixture_segments_dir();
    let p = vi_media::probe(&cfg(), SegmentFeed::new(&dir))
        .await
        .unwrap();
    assert_eq!(p.path, dir.display().to_string());
    assert!(
        (p.duration.unwrap().as_secs_f64() - fx::DURATION_SECS).abs() < 0.1,
        "{:?}",
        p.duration
    );
    assert_eq!(p.start_time, vi_core::Timestamp::ZERO);
    assert!(p.format_name.contains("mpegts"), "{}", p.format_name);
    let v = p.video_stream().unwrap();
    assert_eq!(v.codec, "h264");
    assert_eq!((v.width, v.height), (Some(fx::WIDTH), Some(fx::HEIGHT)));
    assert!((v.fps.unwrap() - fx::FPS).abs() < 0.01);
    assert_eq!(v.duration, None, "one segment says nothing about the whole");
    assert_eq!(v.frames, None);
    let a = p.audio_stream().unwrap();
    assert_eq!(a.sample_rate, Some(fx::AUDIO_RATE));
    let seg_bytes: u64 = std::fs::read_dir(dir.join("seg"))
        .unwrap()
        .map(|e| e.unwrap().metadata().unwrap().len())
        .sum();
    assert_eq!(p.size_bytes, seg_bytes, "size is the sum of the segments");
    assert_eq!(
        p.metadata.get("live_ended").map(String::as_str),
        Some("true")
    );
    assert_eq!(p.tracks(vi_core::VideoId::new()).len(), 2);

    // Following: the recording may still grow, so no duration.
    let p = vi_media::probe(&cfg(), SegmentFeed::following(&dir))
        .await
        .unwrap();
    assert_eq!(p.duration, None);
    assert_eq!(p.video_stream().unwrap().codec, "h264");

    // The same through MediaInput, and a file still probes as before.
    let p = vi_media::probe(&cfg(), MediaInput::from(SegmentFeed::new(&dir)))
        .await
        .unwrap();
    assert!(p.duration.is_some());
    let p = vi_media::probe(&cfg(), MediaInput::from(fx::fixture_path()))
        .await
        .unwrap();
    assert!(p.format_name.contains("mp4"));
}

#[tokio::test]
async fn probe_over_a_bad_feed_is_an_error_not_a_crash() {
    let tmp = tempfile::tempdir().unwrap();
    // No index at all.
    let r = vi_media::probe(&cfg(), SegmentFeed::new(tmp.path())).await;
    assert!(r.is_err(), "{r:?}");
    // An index that lists no segments.
    std::fs::write(
        tmp.path().join("index.json"),
        r#"{"schema":1,"timebase":{"num":1,"den":90000},"segments":[]}"#,
    )
    .unwrap();
    let r = vi_media::probe(&cfg(), SegmentFeed::new(tmp.path())).await;
    assert!(r.is_err(), "{r:?}");
    // An index whose segment is missing.
    std::fs::write(
        tmp.path().join("index.json"),
        r#"{"schema":1,"timebase":{"num":1,"den":90000},"segments":[{"seq":1,"file":"seg/000001.ts","t0":{"num":0,"den":1},"t1":{"num":2,"den":1},"bytes":1}]}"#,
    )
    .unwrap();
    let r = vi_media::probe(&cfg(), SegmentFeed::new(tmp.path())).await;
    assert!(r.is_err(), "{r:?}");
    // The worker is still usable.
    vi_media::probe(&cfg(), &fx::fixture_path()).await.unwrap();
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

// ------------------------------------------------------------ live C2 --

/// One source frame of the fixture, the tolerance for times that come
/// through the segment index rather than the file.
const ONE_FRAME: f64 = 1.0 / fx::FPS;

/// Copy the segmented fixture into `dst` (files and index), so a test can
/// edit the index without touching the shared fixture.
fn copy_segments(dst: &Path) -> SegmentIndex {
    let src = fx::fixture_segments_dir();
    std::fs::create_dir_all(dst.join("seg")).unwrap();
    let index = SegmentIndex::load(&SegmentIndex::path_in(&src)).unwrap();
    for e in &index.segments {
        std::fs::copy(src.join(&e.file), dst.join(&e.file)).unwrap();
    }
    index.save(&SegmentIndex::path_in(dst)).unwrap();
    index
}

/// Everything a live session produced, in order.
#[derive(Default)]
struct LiveRun {
    frames: Vec<Arc<vi_media::FrameBuffer>>,
    chunks: Vec<vi_media::AudioChunk>,
    ticks: Vec<Timestamp>,
    gaps: Vec<(Timestamp, Timestamp)>,
    ended: bool,
    /// Kinds in arrival order: `f`, `a`, `t`, `g`, `e`.
    order: Vec<char>,
}

async fn collect_live(stream: &mut vi_media::LiveStream) -> LiveRun {
    let mut run = LiveRun::default();
    while let Some(item) = stream.next().await.unwrap() {
        match item {
            LiveItem::Frame(f) => {
                assert!(f.is_shared());
                run.frames.push(f.to_owned_frame());
                run.order.push('f');
            }
            LiveItem::Audio(c) => {
                run.chunks.push(c);
                run.order.push('a');
            }
            LiveItem::Tick { head } => {
                run.ticks.push(head);
                run.order.push('t');
            }
            LiveItem::Gap { t0, t1 } => {
                run.gaps.push((t0, t1));
                run.order.push('g');
            }
            LiveItem::End => {
                assert!(!run.ended, "End twice");
                run.ended = true;
                run.order.push('e');
            }
        }
    }
    assert!(run.ended, "the stream closed without End");
    assert_eq!(run.order.last(), Some(&'e'), "End must be the last item");
    run
}

#[tokio::test]
async fn live_decode_of_the_segmented_fixture_matches_the_file_decode() {
    let dir = fx::fixture_segments_dir();
    let cfg = cfg();

    // The reference: the file decoded as batch does it, frames at 1 fps and
    // audio in 1 s chunks without overlap.
    let file_frames =
        vi_media::decode_video(&cfg, VideoDecodeRequest::new(fx::fixture_path(), 1.0, 320))
            .await
            .unwrap()
            .collect_owned()
            .await
            .unwrap();
    let file_chunks = vi_media::decode_audio(
        &cfg,
        AudioDecodeRequest {
            chunk_secs: 1.0,
            overlap_secs: 0.0,
            ..AudioDecodeRequest::new(fx::fixture_path())
        },
    )
    .await
    .unwrap()
    .collect()
    .await
    .unwrap();

    let started = Instant::now();
    let mut req = LiveDecodeRequest::new(SegmentFeed::new(&dir), 1.0, 320);
    req.chunk_secs = 1.0;
    req.tick_secs = 2.0;
    let mut stream = vi_media::decode_live(&cfg, req).await.unwrap();
    assert_eq!((stream.info().width, stream.info().height), (320, 180));
    assert!(!stream.info().seeking);
    let run = collect_live(&mut stream).await;
    let wall = started.elapsed().as_secs_f64();
    eprintln!(
        "decode_live over the finished fixture: {} frames, {} chunks, {} ticks in {wall:.2} s ({:.1}x real time)",
        run.frames.len(),
        run.chunks.len(),
        run.ticks.len(),
        fx::DURATION_SECS / wall
    );
    let stats = stream.stats().unwrap();
    assert_eq!(
        stats.items as usize,
        run.frames.len() + run.chunks.len(),
        "items are frames plus chunks"
    );
    // Every frame of every segment is decoded (a live pass has no end to
    // stop short of); the fixture has no non-reference frames to skip.
    assert!(
        stats.decoded >= run.frames.len() as u64 && stats.decoded <= 3600,
        "decoded {}",
        stats.decoded
    );

    // Frames: the same count within one frame (the index places every
    // segment after the first 21 ms late, the audio overhang of the cut, so
    // the last frame can cross the 120 s target), times increasing about
    // one second apart, and the segment's colour at every frame.
    let n = run.frames.len();
    assert!(
        (n as i64 - file_frames.len() as i64).abs() <= 1,
        "live {n} frames, file {}",
        file_frames.len()
    );
    let mut prev = f64::NEG_INFINITY;
    for f in &run.frames {
        assert_eq!((f.width, f.height), (320, 180));
        assert_eq!(f.format, PixelFormat::Rgb24);
        let t = f.t.as_secs_f64();
        assert!(t > prev, "frame times must increase: {t} after {prev}");
        if prev.is_finite() {
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
    for (live, file) in run.frames.iter().zip(&file_frames) {
        let d = (live.t.as_secs_f64() - file.t.as_secs_f64()).abs();
        assert!(
            d <= ONE_FRAME + 1e-6,
            "frame at {} vs file {}",
            live.t,
            file.t
        );
    }

    // Audio: one chunk per second on the same boundaries as the file decode
    // within a frame, all 16 kHz mono, all the 440 Hz tone.
    assert!(
        (run.chunks.len() as i64 - file_chunks.len() as i64).abs() <= 1,
        "live {} chunks, file {}",
        run.chunks.len(),
        file_chunks.len()
    );
    for (i, (live, file)) in run.chunks.iter().zip(&file_chunks).enumerate() {
        assert_eq!(live.sample_rate, 16_000);
        let d0 = (live.t0.as_secs_f64() - file.t0.as_secs_f64()).abs();
        let d1 = (live.t1.as_secs_f64() - file.t1.as_secs_f64()).abs();
        assert!(
            d0 <= ONE_FRAME && d1 <= ONE_FRAME,
            "chunk {i}: live [{}, {}) vs file [{}, {})",
            live.t0,
            live.t1,
            file.t0,
            file.t1
        );
        if i + 1 < file_chunks.len() {
            assert_eq!(live.samples.len(), 16_000, "chunk {i}");
        }
        let secs = live.samples.len() as f64 / 16_000.0;
        let crossings = live
            .samples
            .windows(2)
            .filter(|w| (w[0] < 0) != (w[1] < 0))
            .count() as f64;
        let hz = crossings / secs / 2.0;
        assert!((hz - fx::TONE_HZ).abs() < 8.0, "chunk {i} ~{hz} Hz");
    }
    // Chunks are contiguous: each starts where the previous ended.
    for w in run.chunks.windows(2) {
        assert_eq!(w[0].t1, w[1].t0, "chunks must be contiguous");
    }

    // Ticks: one per 2 s segment, non-decreasing, each at a listed segment's
    // end, the last at the head; no gaps; End last.
    let index = SegmentIndex::load(&SegmentIndex::path_in(&dir)).unwrap();
    assert_eq!(run.ticks.len(), index.segments.len(), "{:?}", run.ticks);
    for w in run.ticks.windows(2) {
        assert!(w[0] < w[1], "ticks must increase");
    }
    for head in &run.ticks {
        assert!(
            index.segments.iter().any(|e| e.t1 == *head),
            "tick {head} is not a segment end"
        );
    }
    assert_eq!(*run.ticks.last().unwrap(), index.head());
    assert!(run.gaps.is_empty(), "{:?}", run.gaps);
    // Every frame and chunk arrives before the tick that covers it.
    let mut covered = Timestamp::ZERO;
    let mut frames_seen = 0usize;
    let mut ticks_seen = 0usize;
    for kind in &run.order {
        match kind {
            'f' => {
                let t = run.frames[frames_seen].t;
                assert!(t >= covered, "frame at {t} after tick {covered}");
                frames_seen += 1;
            }
            't' => {
                covered = run.ticks[ticks_seen];
                ticks_seen += 1;
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn following_a_feed_written_at_20x_ticks_behind_the_head_and_ends() {
    let tmp = tempfile::tempdir().unwrap();
    let store = tmp.path().join("store");
    let cfg = cfg();
    let writer = fx::PacedWriter::new(&store, 20.0).unwrap();
    // The writer marks the index `ended` once every segment is copied.
    let finisher = std::thread::spawn(move || {
        while !writer.is_done() {
            std::thread::sleep(Duration::from_millis(20));
        }
        writer.finish().unwrap();
    });

    // A probe over the growing feed has no duration.
    let feed = SegmentFeed::following(&store);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match vi_media::probe(&cfg, feed.clone()).await {
            Ok(p) => {
                assert_eq!(p.duration, None, "{p:?}");
                assert_eq!(
                    p.metadata.get("live_ended").map(String::as_str),
                    Some("false")
                );
                break;
            }
            // No segment listed yet.
            Err(_) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(20)).await
            }
            Err(e) => panic!("probe over the growing feed failed: {e}"),
        }
    }

    let started = Instant::now();
    let mut stream = vi_media::decode_live(&cfg, LiveDecodeRequest::new(feed.clone(), 1.0, 320))
        .await
        .unwrap();
    let mut ticks = Vec::new();
    let mut frames = 0usize;
    let mut chunks = 0usize;
    let mut ended = false;
    let mut first_tick_at = None;
    while let Some(item) = stream.next().await.unwrap() {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "stream did not end"
        );
        match item {
            LiveItem::Tick { head } => {
                first_tick_at.get_or_insert_with(|| started.elapsed());
                // At the moment of the tick the index listed a segment ending
                // at `head`; the index only grows, so it still does.
                let index = feed.load_index().unwrap();
                assert!(
                    head <= index.head(),
                    "tick {head} past the head {}",
                    index.head()
                );
                assert!(
                    index.segments.iter().any(|e| e.t1 == head),
                    "tick {head} is not a listed segment's end"
                );
                if let Some(prev) = ticks.last() {
                    assert!(*prev < head);
                }
                ticks.push(head);
            }
            LiveItem::Frame(f) => {
                frames += 1;
                assert!(fx::color_close(
                    probe_color(&f),
                    fx::color_at(f.t.as_secs_f64()),
                    16
                ));
            }
            LiveItem::Audio(_) => chunks += 1,
            LiveItem::Gap { t0, t1 } => panic!("unexpected gap {t0}..{t1}"),
            LiveItem::End => ended = true,
        }
    }
    let wall = started.elapsed().as_secs_f64();
    finisher.join().unwrap();
    assert!(ended);
    let index = feed.load_index().unwrap();
    assert!(index.ended);
    assert_eq!(
        ticks.len(),
        index.segments.len(),
        "one tick per 2 s segment"
    );
    assert_eq!(*ticks.last().unwrap(), index.head());
    assert!((118..=121).contains(&frames), "{frames} frames");
    assert!((118..=121).contains(&chunks), "{chunks} chunks");
    // 120 s of media arrive in about 6 s; the decoder must keep up and end
    // within a poll or two of the writer.
    assert!(wall < 15.0, "took {wall:.1} s");
    eprintln!(
        "decode_live following a 20x feed: {frames} frames, {chunks} chunks, {} ticks in {wall:.2} s ({:.1}x real time), first tick after {:?}",
        ticks.len(),
        fx::DURATION_SECS / wall,
        first_tick_at.unwrap()
    );
}

#[tokio::test]
async fn a_hole_in_the_index_is_one_gap_and_times_skip_it() {
    let tmp = tempfile::tempdir().unwrap();
    let store = tmp.path().join("store");
    let mut index = copy_segments(&store);
    // Drop segments 10 and 11: a 4 s hole, recorded as the store would.
    let before = index.segments.iter().find(|e| e.seq == 9).unwrap().t1;
    let after = index.segments.iter().find(|e| e.seq == 12).unwrap().t0;
    assert!((after.sub(before).as_secs_f64() - 4.0).abs() < 1e-6);
    index.segments.retain(|e| e.seq != 10 && e.seq != 11);
    index
        .gaps
        .push(SegmentGap::new(before, after, Some("test")));
    let path = SegmentIndex::path_in(&store);
    index.save(&path).unwrap();
    for seq in [10u64, 11] {
        std::fs::remove_file(store.join(format!("seg/{seq:06}.ts"))).unwrap();
    }

    let mut stream = vi_media::decode_live(
        &cfg(),
        LiveDecodeRequest::new(SegmentFeed::new(&store), 1.0, 320),
    )
    .await
    .unwrap();
    let run = collect_live(&mut stream).await;
    assert_eq!(run.gaps, vec![(before, after)], "{:?}", run.gaps);
    // The gap precedes everything after it and follows everything before it.
    let g = run.order.iter().position(|k| *k == 'g').unwrap();
    let mut frames_before = 0usize;
    for (i, kind) in run.order.iter().enumerate() {
        if *kind == 'f' {
            let t = run.frames[frames_before].t;
            if i < g {
                assert!(
                    t < before,
                    "frame at {t} before the gap marker but after {before}"
                );
            } else {
                assert!(
                    t >= after,
                    "frame at {t} after the gap marker but before {after}"
                );
            }
            frames_before += 1;
        }
    }
    for f in &run.frames {
        let t = f.t;
        assert!(
            !(t >= before && t < after),
            "frame at {t} inside the hole [{before}, {after})"
        );
        assert!(fx::color_close(
            probe_color(f),
            fx::color_at(t.as_secs_f64()),
            16
        ));
    }
    // 4 s of the 120 s are missing at 1 fps: 116 targets, the first frame
    // after the hole serving the target that fell inside it.
    assert!(
        (115..=118).contains(&run.frames.len()),
        "{} frames",
        run.frames.len()
    );
    // Audio stops before the hole and resumes after it.
    for c in &run.chunks {
        assert!(
            c.t1.as_secs_f64() <= before.as_secs_f64() + ONE_FRAME
                || c.t0.as_secs_f64() >= after.as_secs_f64() - ONE_FRAME,
            "chunk [{}, {}) overlaps the hole [{before}, {after})",
            c.t0,
            c.t1
        );
    }
    let resumed = run
        .chunks
        .iter()
        .find(|c| c.t0 >= before)
        .expect("audio after the hole");
    assert!(
        (resumed.t0.as_secs_f64() - after.as_secs_f64()).abs() <= ONE_FRAME,
        "{}",
        resumed.t0
    );
    // Ticks skip the hole too: none inside it, and the one after it is at
    // segment 12's end.
    assert!(run.ticks.iter().all(|h| !(*h > before && *h < after)));
    let seg12_t1 = index.segments.iter().find(|e| e.seq == 12).unwrap().t1;
    assert!(run.ticks.contains(&seg12_t1));
}

/// A `tracing` subscriber writing every event to a shared buffer, so a test
/// can count the debug lines the parent logs for the worker's messages.
#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for LogBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl LogBuffer {
    fn lines_containing(&self, needle: &str) -> usize {
        String::from_utf8_lossy(&self.0.lock().unwrap())
            .lines()
            .filter(|l| l.contains(needle))
            .count()
    }
}

#[tokio::test]
async fn window_decode_over_segments_opens_only_the_covering_segments() {
    let dir = fx::fixture_segments_dir();
    let req = VideoDecodeRequest::new(SegmentFeed::new(&dir), 1.0, 320).range(30.0, Some(35.0));
    // Warm up: run the decode once with no subscriber so the parent's
    // `debug!` callsites are registered before the subscriber below is
    // installed. `tracing` caches each callsite's interest when it is
    // first hit; a callsite first hit on another test's thread at the same
    // moment as `set_default` can keep a stale "never" and drop this
    // thread's events.
    let warm = vi_media::decode_video(&cfg(), req.clone())
        .await
        .unwrap()
        .collect_owned()
        .await
        .unwrap();
    assert_eq!(warm.len(), 5);
    let logs = LogBuffer::default();
    let sink = logs.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(move || sink.clone())
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let stream = vi_media::decode_video(&cfg(), req).await.unwrap();
    assert!(!stream.info().seeking);
    assert_eq!((stream.info().width, stream.info().height), (320, 180));
    let frames = stream.collect_owned().await.unwrap();
    let ts: Vec<f64> = frames.iter().map(|f| f.t.as_secs_f64()).collect();
    assert_eq!(ts.len(), 5, "{ts:?}");
    for (i, f) in frames.iter().enumerate() {
        let target = 30.0 + i as f64;
        let t = f.t.as_secs_f64();
        assert!((t - target).abs() <= ONE_FRAME, "target {target} got {t}");
        assert!(fx::color_close(probe_color(f), fx::color_at(t), 16));
    }
    let opened = logs.lines_containing("opening segment");
    assert!(
        (1..=3).contains(&opened),
        "{opened} segments opened for a 5 s window over 2 s segments"
    );
    // The same window from the file, for the same frames.
    let file = vi_media::decode_video(
        &cfg(),
        VideoDecodeRequest::new(fx::fixture_path(), 1.0, 320).range(30.0, Some(35.0)),
    )
    .await
    .unwrap()
    .collect_owned()
    .await
    .unwrap();
    assert_eq!(file.len(), frames.len());
    for (a, b) in frames.iter().zip(&file) {
        assert!((a.t.as_secs_f64() - b.t.as_secs_f64()).abs() <= ONE_FRAME);
    }

    // A window past the head yields no frames and no error.
    let req = VideoDecodeRequest::new(SegmentFeed::new(&dir), 1.0, 320).range(500.0, Some(505.0));
    let frames = vi_media::decode_video(&cfg(), req)
        .await
        .unwrap()
        .collect_owned()
        .await
        .unwrap();
    assert!(frames.is_empty());
}

#[tokio::test]
async fn audio_window_decode_over_segments_matches_the_file() {
    let dir = fx::fixture_segments_dir();
    let over_segments = vi_media::decode_audio(
        &cfg(),
        AudioDecodeRequest {
            t0_secs: 100.0,
            t1_secs: Some(110.0),
            ..AudioDecodeRequest::new(SegmentFeed::new(&dir))
        },
    )
    .await
    .unwrap()
    .collect()
    .await
    .unwrap();
    let over_file = vi_media::decode_audio(
        &cfg(),
        AudioDecodeRequest {
            t0_secs: 100.0,
            t1_secs: Some(110.0),
            ..AudioDecodeRequest::new(fx::fixture_path())
        },
    )
    .await
    .unwrap()
    .collect()
    .await
    .unwrap();
    assert_eq!(over_segments.len(), 1, "{over_segments:?}");
    assert_eq!(over_file.len(), 1);
    let (s, f) = (&over_segments[0], &over_file[0]);
    assert!(
        (s.t0.as_secs_f64() - f.t0.as_secs_f64()).abs() <= ONE_FRAME,
        "{} vs {}",
        s.t0,
        f.t0
    );
    assert!(
        (s.samples.len() as i64 - f.samples.len() as i64).abs()
            <= (16_000.0 * ONE_FRAME) as i64 + 64,
        "{} vs {} samples",
        s.samples.len(),
        f.samples.len()
    );
    let secs = s.samples.len() as f64 / 16_000.0;
    let crossings = s
        .samples
        .windows(2)
        .filter(|w| (w[0] < 0) != (w[1] < 0))
        .count() as f64;
    assert!((crossings / secs / 2.0 - fx::TONE_HZ).abs() < 5.0);
}
