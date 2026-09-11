//! End-to-end: index the synthetic fixture with the M0 policy and check what
//! landed in the index.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use vi_core::config::Config;
use vi_core::model::{IndexState, StageStatus};
use vi_core::{Event, EventBus};
use vi_index::{EmbeddedIndex, Storage};
use vi_media::Source;
use vi_perceive::{hamming, PHASH_DEDUP_DISTANCE};
use vi_pipeline::{JobOptions, Scheduler};
use vi_testkit as fx;

fn config() -> Arc<Config> {
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    c.media.sample_max_dim = 320;
    Arc::new(c)
}

#[tokio::test]
async fn m0_policy_indexes_the_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let events = EventBus::default();
    let mut rx = events.subscribe();
    let sched = Scheduler::new(idx.clone(), config(), events);

    let report = sched
        .run(
            Source::Path(fx::fixture_path()),
            JobOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(report.ok, "{report:?}");
    assert!(!report.skipped);
    assert_eq!(report.index_state, IndexState::Coarse);
    for stage in ["sample", "phash", "thumbnail"] {
        let s = &report.stages[stage];
        assert_eq!(s.status, StageStatus::Complete, "{stage}: {s:?}");
        assert!(
            (118..=121).contains(&s.items_done),
            "{stage}: {}",
            s.items_done
        );
    }

    // Storage contents.
    let videos = idx.list_videos().await.unwrap();
    assert_eq!(videos.len(), 1);
    let video = &videos[0];
    assert_eq!(video.index_state, IndexState::Coarse);
    assert!((video.duration.as_secs_f64() - fx::DURATION_SECS).abs() < 0.6);
    let tracks = idx.tracks(video.id).await.unwrap();
    assert_eq!(tracks.len(), 2);
    let vtrack = tracks
        .iter()
        .find(|t| t.kind == vi_core::model::TrackKind::Video)
        .unwrap();
    let samples = idx.frame_samples(vtrack.id, None).await.unwrap();
    assert_eq!(samples.len() as u64, report.stages["sample"].items_done);
    assert!(
        samples.iter().all(|s| s.phash.is_some()),
        "every sample hashed"
    );
    assert!(
        samples.iter().all(|s| s.thumbnail_blob.is_some()),
        "every sample thumbnailed"
    );
    assert!(samples
        .iter()
        .all(|s| s.width == fx::WIDTH && s.height == fx::HEIGHT));

    // Thumbnails are WebP blobs.
    let key = vi_index::BlobKey::parse(samples[0].thumbnail_blob.as_deref().unwrap()).unwrap();
    let blob = idx.get_blob(&key).await.unwrap().unwrap();
    assert_eq!(&blob[0..4], b"RIFF");
    assert_eq!(&blob[8..12], b"WEBP");

    // Shot-relevant hashing: within a 10 s segment consecutive frames are
    // near-duplicates; across a hard cut they are far apart.
    let mut within = 0;
    let mut across = 0;
    for pair in samples.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let d = hamming(a.phash.unwrap(), b.phash.unwrap());
        let same_segment = fx::segment_at(a.t.as_secs_f64()) == fx::segment_at(b.t.as_secs_f64());
        if same_segment {
            assert!(
                d <= PHASH_DEDUP_DISTANCE,
                "t={} -> {}: distance {d}",
                a.t,
                b.t
            );
            within += 1;
        } else {
            assert!(
                d > PHASH_DEDUP_DISTANCE,
                "cut at {} -> {}: distance {d}",
                a.t,
                b.t
            );
            across += 1;
        }
    }
    assert!(
        within > 100 && across == 11,
        "within={within} across={across}"
    );

    // Checkpoint and stats.
    let jobs = idx.list_jobs().await.unwrap();
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0].finished);
    assert!(jobs[0].is_complete("thumbnail"));
    let stats = idx.stats().await.unwrap();
    assert_eq!(stats.videos[0].frame_samples, samples.len() as u64);
    assert_eq!(stats.videos[0].thumbnails, samples.len() as u64);
    assert_eq!(stats.blob_count as usize, samples.len());
    assert!(stats.blob_bytes > 0 && stats.dir_bytes > stats.blob_bytes);

    // Events: started, progress, stage events, finished.
    let mut kinds = std::collections::BTreeSet::new();
    while let Ok(ev) = rx.try_recv() {
        kinds.insert(match ev {
            Event::JobStarted { .. } => "job_started",
            Event::StageStarted { .. } => "stage_started",
            Event::Progress(_) => "progress",
            Event::StageFinished { .. } => "stage_finished",
            Event::JobFinished { ok: true, .. } => "job_finished",
            _ => "other",
        });
    }
    for k in [
        "job_started",
        "stage_started",
        "progress",
        "stage_finished",
        "job_finished",
    ] {
        assert!(kinds.contains(k), "missing {k} in {kinds:?}");
    }

    // Re-running the same file is a no-op with the same video id.
    let again = sched
        .run(
            Source::Path(fx::fixture_path()),
            JobOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(again.skipped);
    assert_eq!(again.video_id, video.id);
    assert_eq!(idx.list_videos().await.unwrap().len(), 1);

    // Forcing re-runs without duplicating samples.
    let forced = sched
        .run(
            Source::Path(fx::fixture_path()),
            JobOptions {
                force: true,
                ..JobOptions::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(!forced.skipped && forced.ok, "{forced:?}");
    assert_eq!(forced.video_id, video.id);
    let samples2 = idx.frame_samples(vtrack.id, None).await.unwrap();
    assert_eq!(samples2.len(), samples.len());
}

#[tokio::test]
async fn shot_boundaries_cover_the_fixture_with_one_shot_per_segment() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    c.media.sample_max_dim = 320;
    c.policy.insert(
        "shots".into(),
        vi_core::config::IndexPolicy {
            coarse: vec!["sample".into(), "shot_boundary".into()],
            fine: vec![],
            ..vi_core::config::IndexPolicy::m0()
        },
    );
    let sched = Scheduler::new(idx.clone(), Arc::new(c), EventBus::default());
    let report = sched
        .run(
            Source::Path(fx::fixture_path()),
            JobOptions {
                policy: Some("shots".into()),
                ..JobOptions::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(report.ok, "{report:?}");
    let video = &idx.list_videos().await.unwrap()[0];
    let shots = idx
        .segments(video.id, vi_core::model::SegmentLevel::Shot)
        .await
        .unwrap();
    // The fixture has a hard cut every 10 s: twelve shots.
    assert_eq!(shots.len(), 12, "{shots:?}");
    // The level covers the whole video with no gaps.
    assert_eq!(shots[0].t0, vi_core::Timestamp::ZERO);
    for w in shots.windows(2) {
        assert_eq!(w[0].t1, w[1].t0, "gap between {:?} and {:?}", w[0], w[1]);
    }
    assert!((shots[11].t1.as_secs_f64() - video.duration.as_secs_f64()).abs() < 1e-6);
    // Each cut lands within one sample (1 s) of the true cut.
    for (i, s) in shots.iter().enumerate().skip(1) {
        let expected = i as f64 * fx::SEGMENT_SECS;
        assert!(
            (s.t0.as_secs_f64() - expected).abs() <= 1.05,
            "shot {i} starts at {} expected {expected}",
            s.t0
        );
        assert!(s.keyframe_sample_id.is_some());
    }
    assert_eq!(report.stages["shot_boundary"].items_done, 12);
}

#[tokio::test]
async fn unknown_and_planned_operators_are_clear_errors() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let sched = Scheduler::new(idx, config(), EventBus::default());
    let err = sched.plan(Some("lecture_default")).unwrap_err();
    assert!(matches!(err, vi_core::Error::Unsupported(_)), "{err}");
    assert!(sched.plan(Some("does_not_exist")).is_err());
    let (name, _, dag) = sched.plan(None).unwrap();
    assert_eq!(name, "coarse_local");
    let stages = dag.stage_names();
    assert!(
        stages.contains(&"subtitle_import".to_string()) && stages.contains(&"sample".to_string())
    );
    let (name, _, dag) = sched.plan(Some("m0")).unwrap();
    assert_eq!(name, "m0");
    assert_eq!(dag.stage_names()[0], "sample");
}

#[tokio::test]
async fn provider_backed_operators_need_a_bound_role_at_plan_time() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    let pol = vi_core::config::IndexPolicy {
        coarse: vec!["vad".into(), "asr".into()],
        fine: vec![],
        ..vi_core::config::IndexPolicy::m0()
    };
    c.policy.insert("speech".into(), pol);
    let sched = Scheduler::new(idx.clone(), Arc::new(c.clone()), EventBus::default());
    let err = sched.plan(Some("speech")).unwrap_err();
    assert!(matches!(err, vi_core::Error::Provider(_)), "{err}");
    assert!(err.to_string().contains("role 'asr'"), "{err}");
    // Bound role: plans, and the DAG runs vad before asr.
    let mut c2 = c.clone();
    c2.providers.insert(
        "w".into(),
        vi_core::config::ProviderConfig {
            adapter: "openai_compat".into(),
            base_url: Some("http://127.0.0.1:9".into()),
            ..Default::default()
        },
    );
    c2.roles.insert(
        "asr".into(),
        vi_core::config::RoleBinding {
            provider: "w".into(),
            ..Default::default()
        },
    );
    let sched = Scheduler::new(idx, Arc::new(c2), EventBus::default());
    let (_, _, dag) = sched.plan(Some("speech")).unwrap();
    assert_eq!(
        dag.stage_names(),
        vec!["vad".to_string(), "asr".to_string()]
    );
}

#[tokio::test]
async fn cancellation_stops_a_job_quickly() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let sched = Scheduler::new(idx.clone(), config(), EventBus::default());
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        c2.cancel();
    });
    let started = std::time::Instant::now();
    let report = sched
        .run(
            Source::Path(fx::fixture_path()),
            JobOptions::default(),
            cancel,
        )
        .await
        .unwrap();
    assert!(!report.ok);
    assert_eq!(report.index_state, IndexState::Failed);
    assert!(
        started.elapsed().as_secs_f64() < 5.0,
        "took {:?}",
        started.elapsed()
    );
    assert!(report
        .stages
        .values()
        .any(|s| s.status == StageStatus::Failed && s.error.as_deref() == Some("cancelled")));
}

#[tokio::test]
async fn sidecars_from_incoming_are_imported_and_media_moves_into_the_cache() {
    use vi_core::model::{SegmentLevel, TrackKind};
    use vi_index::{Kind, TextQuery};

    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("videos");
    let incoming = cache.join("incoming").join("PLtest");
    std::fs::create_dir_all(&incoming).unwrap();
    let media = incoming.join("001-fixture.mp4");
    std::fs::copy(fx::fixture_path(), &media).unwrap();
    std::fs::write(
        incoming.join("001-fixture.info.json"),
        serde_json::json!({
            "id": "fixture",
            "title": "Synthetic workshop",
            "description": "twelve colour segments",
            "channel": "VideoIndex tests",
            "webpage_url": "https://www.youtube.com/watch?v=fixture",
            "upload_date": "20250101",
            "chapters": [
                {"start_time": 0.0, "end_time": 60.0, "title": "First half"},
                {"start_time": 60.0, "end_time": 120.0, "title": "Second half"}
            ],
            "subtitles": {"en": [{"ext": "srt"}]},
            "automatic_captions": {}
        })
        .to_string(),
    )
    .unwrap();
    let mut srt = String::new();
    for i in 0..24 {
        let t0 = i * 5;
        srt.push_str(&format!(
            "{}\n00:0{}:{:02},000 --> 00:0{}:{:02},500\ncaption kw{i} about segment {}\n\n",
            i + 1,
            t0 / 60,
            t0 % 60,
            (t0 + 4) / 60,
            (t0 + 4) % 60,
            t0 / 10
        ));
    }
    std::fs::write(incoming.join("001-fixture.en.srt"), srt).unwrap();

    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    c.media.sample_max_dim = 320;
    c.media.cache_dir = cache.clone();
    let sched = Scheduler::new(idx.clone(), Arc::new(c), EventBus::default());

    // Directory expansion finds the one video.
    let expanded = sched.expand(&Source::Path(incoming.clone())).await.unwrap();
    assert_eq!(expanded.len(), 1);

    let report = sched
        .run(
            expanded[0].clone(),
            JobOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(report.ok, "{report:?}");
    assert_eq!(
        report.stages["subtitle_import"].status,
        StageStatus::Complete
    );
    assert!(
        report.stages["subtitle_import"].items_done >= 6,
        "{report:?}"
    );

    // Media and sidecars moved into the content-addressed cache.
    assert!(!media.exists());
    let video = idx.get_video(report.video_id).await.unwrap().unwrap();
    assert!(cache.join(format!("{}.mp4", video.content_hash)).is_file());
    assert!(cache
        .join(format!("{}.info.json", video.content_hash))
        .is_file());
    assert!(cache
        .join(format!("{}.en.srt", video.content_hash))
        .is_file());

    // Metadata from info.json.
    assert_eq!(video.title.as_deref(), Some("Synthetic workshop"));
    assert_eq!(video.channel.as_deref(), Some("VideoIndex tests"));
    assert_eq!(video.source_uri, "https://www.youtube.com/watch?v=fixture");
    assert_eq!(
        video.published_at.unwrap().format("%Y-%m-%d").to_string(),
        "2025-01-01"
    );

    // Chapters became chapter segments.
    let chapters = idx.segments(video.id, SegmentLevel::Chapter).await.unwrap();
    assert_eq!(chapters.len(), 2);
    assert_eq!(chapters[1].title.as_deref(), Some("Second half"));
    assert_eq!(chapters[1].t0, vi_core::Timestamp::from_secs(60));

    // Subtitles became a subtitle track with searchable spans.
    let tracks = idx.tracks(video.id).await.unwrap();
    let sub = tracks
        .iter()
        .find(|t| t.kind == TrackKind::Subtitle)
        .unwrap();
    assert_eq!(sub.language.as_deref(), Some("en"));
    assert!(sub.stream_index >= 1000);
    let hits = idx
        .text_search(&TextQuery {
            kinds: vec![Kind::Transcript],
            ..TextQuery::new("kw14", 5)
        })
        .await
        .unwrap();
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(
        hits[0].text.contains("kw14 about segment 7"),
        "{:?}",
        hits[0]
    );
    assert!(
        hits[0].t0.as_secs_f64() >= 60.0 && hits[0].t0.as_secs_f64() < 80.0,
        "{:?}",
        hits[0]
    );
    let w = idx
        .time_window(
            video.id,
            vi_core::Timestamp::from_secs(30),
            vi_core::Timestamp::from_secs(45),
            &[Kind::Transcript],
        )
        .await
        .unwrap();
    assert!(!w.transcript.is_empty());
    assert!(
        w.transcript.iter().all(|s| s.confidence == Some(1.0)),
        "human subtitles"
    );

    // Re-indexing the cached file (forced) does not duplicate subtitle tracks or chapters.
    let cached = cache.join(format!("{}.mp4", video.content_hash));
    let again = sched
        .run(
            Source::Path(cached),
            JobOptions {
                force: true,
                ..JobOptions::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(again.ok && again.video_id == video.id);
    let tracks = idx.tracks(video.id).await.unwrap();
    assert_eq!(
        tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Subtitle)
            .count(),
        1
    );
    assert_eq!(
        idx.segments(video.id, SegmentLevel::Chapter)
            .await
            .unwrap()
            .len(),
        2
    );
    let stats = idx.stats().await.unwrap();
    assert_eq!(stats.videos.len(), 1);
    assert!(stats.videos[0].transcript_spans >= 6);
    assert_eq!(stats.videos[0].segments, 2);
}
