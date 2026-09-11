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
    // Every operator of the design's default policy exists now; without
    // provider roles it fails at plan time naming the missing role.
    let err = sched.plan(Some("lecture_default")).unwrap_err();
    assert!(matches!(err, vi_core::Error::Provider(_)), "{err}");
    assert!(err.to_string().contains("role"), "{err}");
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

fn policy(ops: &[&str]) -> vi_core::config::IndexPolicy {
    vi_core::config::IndexPolicy {
        coarse: ops.iter().map(|s| s.to_string()).collect(),
        fine: vec![],
        ..vi_core::config::IndexPolicy::m0()
    }
}

/// Write the fixture with an `.info.json` and an English SRT into
/// `<cache>/incoming/PLtest/` and return the media path.
fn seed_incoming(cache: &std::path::Path) -> std::path::PathBuf {
    let incoming = cache.join("incoming").join("PLtest");
    std::fs::create_dir_all(&incoming).unwrap();
    let media = incoming.join("001-fixture.mp4");
    std::fs::copy(fx::fixture_path(), &media).unwrap();
    std::fs::write(
        incoming.join("001-fixture.info.json"),
        serde_json::json!({
            "id": "fixture",
            "title": "Synthetic workshop",
            "webpage_url": "https://www.youtube.com/watch?v=fixture",
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
    media
}

#[tokio::test]
async fn cached_stages_are_skipped_or_replayed_and_new_ones_run() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    c.media.sample_max_dim = 320;
    c.policy
        .insert("a".into(), policy(&["sample", "phash", "thumbnail"]));
    c.policy.insert(
        "b".into(),
        policy(&["sample", "phash", "thumbnail", "shot_boundary"]),
    );
    let sched = Scheduler::new(idx.clone(), Arc::new(c), EventBus::default());
    let run = |p: &str, force: bool| {
        let sched = sched.clone();
        let p = p.to_string();
        async move {
            sched
                .run(
                    Source::Path(fx::fixture_path()),
                    JobOptions {
                        policy: Some(p),
                        force,
                        ..JobOptions::default()
                    },
                    CancellationToken::new(),
                )
                .await
                .unwrap()
        }
    };
    let first = run("a", false).await;
    assert!(first.ok && !first.skipped, "{first:?}");
    assert!(first.stages.values().all(|s| !s.cached));
    // Markers exist for every stage.
    let markers = std::fs::read_dir(dir.path().join("t.vidx/cache/operators"))
        .unwrap()
        .count();
    assert_eq!(markers, 3);

    // Same policy again: everything cached, nothing runs.
    let again = run("a", false).await;
    assert!(again.skipped, "{again:?}");
    assert!(again
        .stages
        .values()
        .all(|s| s.cached && s.status == StageStatus::Skipped));

    // A policy with one more operator: shot_boundary runs, which needs
    // frames, so sample runs too; phash and thumbnail have no running
    // consumer and are skipped from the cache.
    let more = run("b", false).await;
    assert!(more.ok && !more.skipped, "{more:?}");
    assert_eq!(more.stages["shot_boundary"].status, StageStatus::Complete);
    assert_eq!(more.stages["shot_boundary"].items_done, 12);
    assert_eq!(more.stages["sample"].status, StageStatus::Complete);
    assert!(
        more.stages["sample"].cached,
        "sample was cached but had to run"
    );
    // sample re-running replaces the frame rows, so phash and thumbnail,
    // though cached, run again or the new rows would lack hashes and
    // thumbnails.
    assert_eq!(more.stages["phash"].status, StageStatus::Complete);
    assert!(more.stages["phash"].cached);
    assert_eq!(more.stages["thumbnail"].status, StageStatus::Complete);
    let video = &idx.list_videos().await.unwrap()[0];
    let tracks = idx.tracks(video.id).await.unwrap();
    let vt = tracks
        .iter()
        .find(|t| t.kind == vi_core::model::TrackKind::Video)
        .unwrap();
    let samples = idx.frame_samples(vt.id, None).await.unwrap();
    assert!(!samples.is_empty());
    assert!(samples
        .iter()
        .all(|s| s.phash.is_some() && s.thumbnail_blob.is_some()));

    // Same policy again: all four cached, nothing runs.
    let same = run("b", false).await;
    assert!(same.skipped, "{same:?}");

    // Force re-runs everything.
    let forced = run("b", true).await;
    assert!(forced.ok && !forced.skipped);
    assert!(forced
        .stages
        .values()
        .all(|s| s.status == StageStatus::Complete));
    assert!(forced.stages.values().all(|s| !s.cached));
}

#[tokio::test]
async fn budget_exhaustion_skips_provider_calls_and_failures_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("videos");
    let media = seed_incoming(&cache);
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    c.media.cache_dir = cache.clone();
    // A local text embedder pointing at a directory with no models.
    c.providers.insert(
        "local".into(),
        vi_core::config::ProviderConfig {
            adapter: "onnx_local".into(),
            model_dir: Some(dir.path().join("no-models")),
            ..Default::default()
        },
    );
    c.roles.insert(
        "text_embed".into(),
        vi_core::config::RoleBinding {
            provider: "local".into(),
            ..Default::default()
        },
    );
    let mut tight = policy(&["subtitle_import", "text_embed"]);
    tight.max_wallclock_per_hour = "1ms".into();
    c.policy.insert("tight".into(), tight);
    c.policy
        .insert("loose".into(), policy(&["subtitle_import", "text_embed"]));
    let sched = Scheduler::new(idx.clone(), Arc::new(c), EventBus::default());

    // Wall-clock budget of a millisecond: no provider call is issued, the
    // stage completes with everything skipped, the job is still ok.
    let r = sched
        .run(
            Source::Path(media.clone()),
            JobOptions {
                policy: Some("tight".into()),
                ..JobOptions::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(r.ok, "{r:?}");
    let te = &r.stages["text_embed"];
    assert_eq!(te.status, StageStatus::Complete);
    assert!(te.items_skipped >= 6, "{te:?}");
    assert_eq!(te.items_failed, 0);
    assert_eq!(r.budget.exhausted.as_deref(), Some("wallclock"));
    // A stage that skipped work leaves no cache marker.
    let markers: Vec<String> = std::fs::read_dir(dir.path().join("t.vidx/cache/operators"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert!(markers.iter().any(|m| m.starts_with("subtitle_import")));
    assert!(
        !markers.iter().any(|m| m.starts_with("text_embed")),
        "{markers:?}"
    );

    // Unlimited budget: every batch fails (no model files), the failures
    // are recorded, the stage is failed, the job is not ok.
    let media_cached = cache.join(format!(
        "{}.mp4",
        idx.get_video(r.video_id)
            .await
            .unwrap()
            .unwrap()
            .content_hash
    ));
    let r2 = sched
        .run(
            Source::Path(media_cached),
            JobOptions {
                policy: Some("loose".into()),
                force: true,
                ..JobOptions::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(!r2.ok, "{r2:?}");
    let te = &r2.stages["text_embed"];
    assert_eq!(te.status, StageStatus::Failed);
    assert!(te.items_failed >= 6, "{te:?}");
    assert!(!te.failures.is_empty());
    assert!(
        te.failures[0].error.contains("model file missing"),
        "{:?}",
        te.failures[0]
    );
    assert_eq!(r2.index_state, IndexState::Failed);
    // subtitle_import still completed and is cached.
    assert_eq!(r2.stages["subtitle_import"].status, StageStatus::Complete);
}

#[tokio::test]
async fn scenes_cover_the_video_and_imported_chapters_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("videos");
    let media = seed_incoming(&cache);
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    c.media.sample_max_dim = 320;
    c.media.cache_dir = cache.clone();
    // chapters needs a text_embed role bound; imported chapters mean it is
    // never called, so a model-less local provider is fine.
    c.providers.insert(
        "local".into(),
        vi_core::config::ProviderConfig {
            adapter: "onnx_local".into(),
            model_dir: Some(dir.path().join("no-models")),
            ..Default::default()
        },
    );
    c.roles.insert(
        "text_embed".into(),
        vi_core::config::RoleBinding {
            provider: "local".into(),
            ..Default::default()
        },
    );
    let mut pol = policy(&[
        "subtitle_import",
        "sample",
        "shot_boundary",
        "scenes",
        "chapters",
    ]);
    pol.fine = vec![];
    c.policy.insert("fine".into(), pol);
    // Add chapters to the sidecar.
    let info = cache.join("incoming/PLtest/001-fixture.info.json");
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&info).unwrap()).unwrap();
    v["chapters"] = serde_json::json!([
        {"start_time": 0.0, "end_time": 60.0, "title": "First half"},
        {"start_time": 60.0, "end_time": 120.0, "title": "Second half"}
    ]);
    std::fs::write(&info, v.to_string()).unwrap();
    let sched = Scheduler::new(idx.clone(), Arc::new(c), EventBus::default());
    let r = sched
        .run(
            Source::Path(media),
            JobOptions {
                policy: Some("fine".into()),
                ..JobOptions::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(r.ok, "{r:?}");
    let video = &idx.list_videos().await.unwrap()[0];
    let scenes = idx
        .segments(video.id, vi_core::model::SegmentLevel::Scene)
        .await
        .unwrap();
    // The synthetic captions are 15 s spans crossing every 10 s cut, so
    // transcript continuity merges the shots into few scenes.
    assert!(!scenes.is_empty() && scenes.len() <= 6, "{scenes:?}");
    assert_eq!(scenes[0].t0, vi_core::Timestamp::ZERO);
    for w in scenes.windows(2) {
        assert_eq!(w[0].t1, w[1].t0, "gap between scenes");
    }
    assert!(
        (scenes[scenes.len() - 1].t1.as_secs_f64() - video.duration.as_secs_f64()).abs() < 1e-6
    );
    assert!(
        scenes
            .iter()
            .all(|s| s.t1.as_secs_f64() - s.t0.as_secs_f64() >= 20.0 - 1e-6),
        "{scenes:?}"
    );
    let shots = idx
        .segments(video.id, vi_core::model::SegmentLevel::Shot)
        .await
        .unwrap();
    assert!(
        shots.iter().all(|s| s.parent_id.is_some()),
        "shots point at scenes"
    );
    assert!(shots
        .iter()
        .all(|s| scenes.iter().any(|sc| Some(sc.id) == s.parent_id)));
    let chapters = idx
        .segments(video.id, vi_core::model::SegmentLevel::Chapter)
        .await
        .unwrap();
    assert_eq!(chapters.len(), 2, "imported chapters kept");
    assert_eq!(chapters[0].title.as_deref(), Some("First half"));
    assert_eq!(r.stages["chapters"].items_done, 2);
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
