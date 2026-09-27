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
    if fx::fixture_has_text() {
        assert_eq!(stats.blob_count as usize, samples.len());
    } else {
        // Without the timestamp overlay the frames of a segment are identical
        // and the content-addressed blob store deduplicates them.
        let blobs = stats.blob_count as usize;
        assert!((12..=samples.len()).contains(&blobs), "{blobs} blobs");
    }
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

/// A fine pass over a coarse-indexed video must not re-decode it: `scenes`
/// needs shots, `shot_boundary` replays them from storage, and a replaying
/// consumer is not a reason for `sample` (which cannot replay) to run.
#[tokio::test]
async fn a_fine_stage_over_a_cached_coarse_pass_replays_instead_of_redecoding() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    c.media.sample_max_dim = 320;
    c.policy.insert(
        "coarse".into(),
        policy(&["sample", "phash", "thumbnail", "shot_boundary"]),
    );
    let mut fine = policy(&["sample", "phash", "thumbnail", "shot_boundary"]);
    fine.fine = vec!["scenes".into()];
    c.policy.insert("fine".into(), fine);
    let sched = Scheduler::new(idx.clone(), Arc::new(c), EventBus::default());
    let run = |p: &str| {
        let sched = sched.clone();
        let p = p.to_string();
        async move {
            sched
                .run(
                    Source::Path(fx::fixture_path()),
                    JobOptions {
                        policy: Some(p),
                        ..JobOptions::default()
                    },
                    CancellationToken::new(),
                )
                .await
                .unwrap()
        }
    };
    let first = run("coarse").await;
    assert!(first.ok, "{first:?}");
    let second = run("fine").await;
    assert!(second.ok && !second.skipped, "{second:?}");
    assert_eq!(second.stages["scenes"].status, StageStatus::Complete);
    assert!(second.stages["scenes"].items_done > 0);
    assert!(
        second.stages["shot_boundary"].replayed,
        "{:?}",
        second.stages["shot_boundary"]
    );
    for st in ["sample", "phash", "thumbnail"] {
        assert_eq!(
            second.stages[st].status,
            StageStatus::Skipped,
            "{st} should be skipped: {:?}",
            second.stages[st]
        );
    }
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

// ------------------------------------------------------------ live C3 --

/// What a batch run of the model-free coarse stages leaves in the index
/// and the operator cache: the regression guard for live changes to the
/// pipeline. The numbers were measured before C3 (at `c0ce635`, whose
/// pipeline is `3bfa6fd`'s) on the GPU box's fixture and must not move.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct GuardNumbers {
    frame_samples: u64,
    hashed: u64,
    thumbnailed: u64,
    shots: Vec<(i64, i64)>,
    transcript_spans: u64,
    blob_count: u64,
    /// `(stage, items)` per cache marker, sorted by stage.
    markers: Vec<(String, u64)>,
    /// Marker file names, sorted; they hash the fixture's content hash, so
    /// they are compared only where the fixture is byte-identical.
    marker_files: Vec<String>,
    /// The fixture's content hash: the same bytes (same ffmpeg build) mean
    /// the marker names must match too.
    #[serde(default)]
    content_hash: String,
}

async fn guard_numbers(idx: &EmbeddedIndex, dir: &std::path::Path) -> GuardNumbers {
    use vi_core::model::{SegmentLevel, TrackKind};
    let video = &idx.list_videos().await.unwrap()[0];
    let tracks = idx.tracks(video.id).await.unwrap();
    let vt = tracks.iter().find(|t| t.kind == TrackKind::Video).unwrap();
    let samples = idx.frame_samples(vt.id, None).await.unwrap();
    let shots = idx.segments(video.id, SegmentLevel::Shot).await.unwrap();
    let stats = idx.stats().await.unwrap();
    let mut markers = Vec::new();
    let mut marker_files = Vec::new();
    for e in std::fs::read_dir(dir.join("t.vidx/cache/operators")).unwrap() {
        let e = e.unwrap();
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(e.path()).unwrap()).unwrap();
        markers.push((
            v["stage"].as_str().unwrap().to_string(),
            v["items"].as_u64().unwrap(),
        ));
        marker_files.push(e.file_name().to_string_lossy().to_string());
    }
    markers.sort();
    marker_files.sort();
    GuardNumbers {
        content_hash: video.content_hash.clone(),
        frame_samples: samples.len() as u64,
        hashed: samples.iter().filter(|s| s.phash.is_some()).count() as u64,
        thumbnailed: samples
            .iter()
            .filter(|s| s.thumbnail_blob.is_some())
            .count() as u64,
        shots: shots
            .iter()
            .map(|s| (s.t0.rescale(1000).num, s.t1.rescale(1000).num))
            .collect(),
        transcript_spans: stats.videos[0].transcript_spans,
        blob_count: stats.blob_count,
        markers,
        marker_files,
    }
}

#[tokio::test]
async fn batch_guard_row_counts_and_cache_markers_are_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    c.media.sample_max_dim = 320;
    // `coarse_local` plus `shot_boundary`: every coarse stage that runs
    // without a model or a provider. `lecture_default` itself needs the
    // asr, ocr, image_embed and text_embed roles and is covered by the
    // dev set.
    c.policy.insert(
        "guard".into(),
        policy(&[
            "subtitle_import",
            "sample",
            "phash",
            "thumbnail",
            "shot_boundary",
        ]),
    );
    let sched = Scheduler::new(idx.clone(), Arc::new(c), EventBus::default());
    let report = sched
        .run(
            Source::Path(fx::fixture_path()),
            JobOptions {
                policy: Some("guard".into()),
                ..JobOptions::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(report.ok && !report.skipped, "{report:?}");
    let now = guard_numbers(&idx, dir.path()).await;
    eprintln!(
        "batch guard numbers: {}",
        serde_json::to_string(&now).unwrap()
    );

    // Structure that holds on every platform.
    assert_eq!(now.frame_samples, now.hashed);
    assert_eq!(now.frame_samples, now.thumbnailed);
    assert_eq!(now.shots.len(), 12, "{:?}", now.shots);
    assert_eq!(now.transcript_spans, 0);
    assert_eq!(now.markers.len(), 5, "{:?}", now.markers);
    assert_eq!(
        now.markers.iter().find(|m| m.0 == "sample").unwrap().1,
        now.frame_samples
    );
    for stage in ["phash", "thumbnail"] {
        assert_eq!(
            now.markers.iter().find(|m| m.0 == stage).unwrap().1,
            now.frame_samples,
            "{stage}"
        );
    }
    assert!(
        report.stages.values().all(|s| s.last_t.is_none()),
        "batch jobs record no last_t"
    );

    // The numbers measured before C3. The marker names hash the fixture's
    // content hash, which follows the ffmpeg build that generated it, so
    // they are compared only when the fixture is the same bytes; the blob
    // count follows `drawtext` (without it the frames of a segment are
    // identical and the blob store deduplicates them). Everything else is
    // compared everywhere.
    let before: GuardNumbers = serde_json::from_str(BEFORE_C3).unwrap();
    let same_bytes = now.content_hash == before.content_hash;
    let comparable = GuardNumbers {
        marker_files: if same_bytes {
            now.marker_files.clone()
        } else {
            before.marker_files.clone()
        },
        content_hash: before.content_hash.clone(),
        blob_count: if fx::fixture_has_text() {
            now.blob_count
        } else {
            before.blob_count
        },
        ..now.clone()
    };
    assert_eq!(comparable, before, "same fixture bytes: {same_bytes}");
    for (stage, _) in &before.markers {
        assert!(
            now.marker_files
                .iter()
                .any(|f| f.starts_with(&format!("{stage}-"))),
            "no marker for {stage}: {:?}",
            now.marker_files
        );
    }
}

/// `GuardNumbers` of the run before C3 (see the test).
const BEFORE_C3: &str = r#"{"frame_samples":120,"hashed":120,"thumbnailed":120,"shots":[[0,10000],[10000,20000],[20000,30000],[30000,40000],[40000,50000],[50000,60000],[60000,70000],[70000,80000],[80000,90000],[90000,100000],[100000,110000],[110000,120000]],"transcript_spans":0,"blob_count":120,"markers":[["phash",120],["sample",120],["shot_boundary",12],["subtitle_import",0],["thumbnail",120]],"marker_files":["phash-349524285b47a43abf53dd5e.json","sample-be70178b8d3bf177cc4694c4.json","shot_boundary-cd060417e00a9f172fd5b9f6.json","subtitle_import-f5fede16dcc7dfca7da66fd2.json","thumbnail-f24f40c69ef9876673f76aad.json"],"content_hash":"7078db70872ab887527dc9b3818fba5f5c005967b9373c33cf4fd1b802ddb29f"}"#;

// ---- live jobs ---------------------------------------------------------

use async_trait::async_trait;
use std::sync::Mutex;
use std::time::Duration;
use vi_core::model::{FrameSample, IndexState as State, SegmentLevel, Video};
use vi_core::{FrameSampleId, Timestamp, VideoId};
use vi_media::{Acquired, LiveDecodeRequest, LiveItem, SegmentFeed};
use vi_pipeline::ops::vad_stream::{
    SpeechScorer, VadStream, FLUSH_AFTER_SILENCE_SECS, MAX_UTTERANCE_SECS,
};
use vi_pipeline::{
    CostEstimate, FrameItem, InputSummary, Item, ItemKind, MediaItem, OpContext, OpInput, OpOutput,
    Operator,
};

/// A scripted VAD: a window is speech when its level is above a threshold.
/// Silero is not a test dependency; the fixtures' tones are "speech".
#[derive(Default)]
struct RmsScorer {
    pending: Vec<i16>,
}

impl SpeechScorer for RmsScorer {
    fn push(&mut self, samples: &[i16]) -> vi_core::Result<Vec<f32>> {
        self.pending.extend_from_slice(samples);
        let mut out = Vec::new();
        let win = vi_perceive::vad::WINDOW;
        while self.pending.len() >= win {
            let w: Vec<i16> = self.pending.drain(..win).collect();
            let rms = (w.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / win as f64).sqrt();
            out.push(if rms > 1000.0 { 0.95 } else { 0.05 });
        }
        Ok(out)
    }
}

fn rms_vad_stream() -> Box<dyn Operator> {
    Box::new(VadStream::with_scorer(Arc::new(|_| {
        Ok(Box::new(RmsScorer::default()) as Box<dyn SpeechScorer>)
    })))
}

/// The live root for tests: `decode_live` over a feed, a `FrameSample` row
/// per frame (as `sample` writes them), frames, audio and ticks emitted.
/// With `stop_at`, the root lets the pipeline drain for a moment after
/// the first tick at or past it, then cancels the job.
struct LiveRoot {
    feed: SegmentFeed,
    tick_secs: f64,
    stop_at: Option<f64>,
    cancel: CancellationToken,
    head_seen: Arc<Mutex<Option<Timestamp>>>,
}

#[async_trait]
impl Operator for LiveRoot {
    fn id(&self) -> &'static str {
        "live_root"
    }
    fn version(&self) -> u32 {
        1
    }
    fn inputs(&self) -> &[ItemKind] {
        &[ItemKind::Media]
    }
    fn outputs(&self) -> &[ItemKind] {
        &[ItemKind::Frame, ItemKind::AudioChunk, ItemKind::Tick]
    }
    fn cost_estimate(&self, _: &InputSummary) -> CostEstimate {
        CostEstimate::default()
    }
    async fn run(&self, ctx: &OpContext, input: OpInput) -> vi_core::Result<OpOutput> {
        let Item::Media(media) = input.item else {
            return Err(ctx.err("expected the media item"));
        };
        let track = media
            .video_track()
            .ok_or_else(|| ctx.err("no video track"))?
            .clone();
        let mut req = LiveDecodeRequest::new(
            self.feed.clone(),
            ctx.policy.sample_fps,
            ctx.config.media.sample_max_dim,
        );
        req.tick_secs = self.tick_secs;
        let mut stream = vi_media::decode_live(&ctx.worker, req).await?;
        let (mut emitted, mut stored) = (0u64, 0u64);
        while let Some(item) = stream.next().await? {
            match item {
                LiveItem::Frame(frame) => {
                    let sample = FrameSample {
                        id: FrameSampleId::new(),
                        track_id: track.id,
                        t: frame.t,
                        pts: frame.pts,
                        is_keyframe: frame.is_keyframe,
                        phash: None,
                        thumbnail_blob: None,
                        width: frame.source_width,
                        height: frame.source_height,
                    };
                    ctx.storage
                        .put_frame_samples(std::slice::from_ref(&sample))
                        .await?;
                    stored += 1;
                    ctx.emit(Item::Frame(FrameItem { sample, frame })).await?;
                    emitted += 1;
                }
                LiveItem::Audio(chunk) => {
                    ctx.emit(Item::AudioChunk(Arc::new(chunk))).await?;
                    emitted += 1;
                }
                LiveItem::Tick { head } => {
                    ctx.emit(Item::Tick { head }).await?;
                    emitted += 1;
                    *self.head_seen.lock().unwrap() = Some(head);
                    if self.stop_at.is_some_and(|s| head.as_secs_f64() >= s) {
                        // The stream is being stopped: give the consumers a
                        // moment to take what is queued, then cancel.
                        tokio::time::sleep(Duration::from_millis(700)).await;
                        self.cancel.cancel();
                        return Err(vi_core::Error::Cancelled);
                    }
                }
                LiveItem::Gap { .. } => {}
                LiveItem::End => break,
            }
        }
        Ok(OpOutput { emitted, stored })
    }
}

/// A `MediaItem` for a live recording, the way the live indexer builds it:
/// the feed probed, a `Video` in the `live` state with the identity hash,
/// no expected sample count.
async fn live_media(cfg: &Config, feed: &SegmentFeed) -> MediaItem {
    let probe = vi_media::probe(&cfg.media.worker, feed.clone())
        .await
        .unwrap();
    let start = chrono::Utc::now();
    let id = VideoId::new();
    let video = Video {
        id,
        source_uri: "live://fixture".into(),
        content_hash: vi_core::model::live_identity_hash("fixture", start),
        title: Some("live fixture".into()),
        description: None,
        channel: None,
        published_at: None,
        duration: Timestamp::ZERO,
        start_wallclock: Some(start),
        probe: serde_json::to_value(&probe).unwrap(),
        index_state: State::Live,
        created_at: start,
        watermark: None,
        live_ended_at: None,
    };
    let tracks = probe.tracks(id);
    MediaItem {
        acquired: Acquired {
            path: feed.dir.clone(),
            source_uri: video.source_uri.clone(),
            content_hash: video.content_hash.clone(),
            size_bytes: probe.size_bytes,
            title: None,
            description: None,
            channel: None,
            published_at: None,
            chapters: Vec::new(),
            subtitle_files: Vec::new(),
            subtitle_languages: Vec::new(),
            info: None,
        },
        probe,
        video,
        tracks,
        expected_samples: None,
    }
}

fn bind_model_less_text_embed(c: &mut Config, dir: &std::path::Path) {
    c.providers.insert(
        "local".into(),
        vi_core::config::ProviderConfig {
            adapter: "onnx_local".into(),
            model_dir: Some(dir.join("no-models")),
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
}

#[tokio::test]
async fn a_live_job_over_the_fixture_ticks_closes_shots_and_records_last_t() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("live.vidx")).unwrap());
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    c.media.sample_max_dim = 320;
    bind_model_less_text_embed(&mut c, dir.path());
    let c = Arc::new(c);
    let feed = SegmentFeed::new(fx::fixture_segments_dir());
    let media = live_media(&c, &feed).await;
    let video_id = media.video.id;

    let cancel = CancellationToken::new();
    let head_seen = Arc::new(Mutex::new(None));
    let tick_secs = 2.0;
    let mut sched = Scheduler::new(idx.clone(), c.clone(), EventBus::default());
    {
        let (feed, cancel, head_seen) = (feed.clone(), cancel.clone(), head_seen.clone());
        sched.register_operator(
            "live_root",
            Arc::new(move |_| {
                Box::new(LiveRoot {
                    feed: feed.clone(),
                    tick_secs,
                    stop_at: Some(90.0),
                    cancel: cancel.clone(),
                    head_seen: head_seen.clone(),
                }) as Box<dyn Operator>
            }),
        );
    }
    sched.register_operator("vad_stream", Arc::new(|_| rms_vad_stream()));
    let live_policy = policy(&[
        "live_root",
        "vad_stream",
        "phash",
        "thumbnail",
        "shot_boundary",
        "text_embed",
    ]);
    let started = std::time::Instant::now();
    let report = sched
        .run_with_media(
            media,
            JobOptions {
                policy: Some("live".into()),
                inline_policy: Some(live_policy),
                live: true,
                ..JobOptions::default()
            },
            cancel.clone(),
        )
        .await
        .unwrap();
    eprintln!(
        "live job: {:.1} s wall, stopped_at {:?}, stages {:?}",
        started.elapsed().as_secs_f64(),
        report.stopped_at,
        report
            .stages
            .iter()
            .map(|(k, v)| (k.clone(), v.status, v.items_done, v.last_t))
            .collect::<Vec<_>>()
    );
    let head = head_seen.lock().unwrap().expect("a tick was seen");
    assert!(head.as_secs_f64() >= 90.0, "stopped at {head}");
    assert!(report.ok, "{report:?}");
    assert!(!report.skipped);
    assert_eq!(report.index_state, State::Live, "the caller owns the state");
    assert_eq!(
        idx.get_video(video_id).await.unwrap().unwrap().index_state,
        State::Live
    );
    assert_eq!(report.stopped_at, Some(head), "{report:?}");
    for (name, st) in &report.stages {
        assert_eq!(st.status, StageStatus::Complete, "{name}: {st:?}");
        assert!(!st.cached);
    }
    // `last_t` per stage within one tick of the head.
    for stage in [
        "vad_stream",
        "phash",
        "thumbnail",
        "shot_boundary",
        "text_embed",
    ] {
        let t = report.stages[stage]
            .last_t
            .unwrap_or_else(|| panic!("{stage} has no last_t"));
        let lag = head.as_secs_f64() - t.as_secs_f64();
        assert!(
            (0.0..=tick_secs + 1e-6).contains(&lag),
            "{stage}: last_t {t} is {lag:.3} s behind the head {head}"
        );
    }
    // Operators that received the tick record the head itself.
    for stage in [
        "vad_stream",
        "phash",
        "thumbnail",
        "shot_boundary",
        "text_embed",
    ] {
        assert_eq!(report.stages[stage].last_t, Some(head), "{stage}");
    }
    // No cache markers for a live job.
    let markers = std::fs::read_dir(dir.path().join("live.vidx/cache/operators"))
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(markers, 0);

    // Every frame up to the head has its row, hash and thumbnail.
    let tracks = idx.tracks(video_id).await.unwrap();
    let vt = tracks
        .iter()
        .find(|t| t.kind == vi_core::model::TrackKind::Video)
        .unwrap();
    let samples = idx.frame_samples(vt.id, None).await.unwrap();
    assert!(
        (88..=92).contains(&samples.len()),
        "{} samples",
        samples.len()
    );
    assert!(samples
        .iter()
        .all(|s| s.phash.is_some() && s.thumbnail_blob.is_some()));
    assert!(samples.iter().all(|s| s.t <= head));

    // Shots: the closed ones match the batch run within one sample; the
    // open one ends at the head.
    let shots = idx.segments(video_id, SegmentLevel::Shot).await.unwrap();
    let batch_dir = tempfile::tempdir().unwrap();
    let batch_idx = Arc::new(EmbeddedIndex::create(&batch_dir.path().join("b.vidx")).unwrap());
    let mut bc = Config::default();
    bc.media.worker.path = Some(fx::worker_path());
    bc.media.sample_max_dim = 320;
    bc.policy
        .insert("shots".into(), policy(&["sample", "shot_boundary"]));
    let batch = Scheduler::new(batch_idx.clone(), Arc::new(bc), EventBus::default());
    let br = batch
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
    assert!(br.ok);
    let bvideo = &batch_idx.list_videos().await.unwrap()[0];
    let batch_shots = batch_idx
        .segments(bvideo.id, SegmentLevel::Shot)
        .await
        .unwrap();
    assert_eq!(batch_shots.len(), 12);
    let (open, closed) = shots.split_last().expect("shots written");
    assert!(
        (7..=9).contains(&closed.len()),
        "{} closed shots by {head}: {shots:?}",
        closed.len()
    );
    let one_sample = 1.0 / fx::FPS.min(1.0);
    for (i, s) in closed.iter().enumerate() {
        let b = &batch_shots[i];
        assert!(
            (s.t0.as_secs_f64() - b.t0.as_secs_f64()).abs() <= one_sample,
            "shot {i} starts at {} vs batch {}",
            s.t0,
            b.t0
        );
        assert!(
            (s.t1.as_secs_f64() - b.t1.as_secs_f64()).abs() <= one_sample,
            "shot {i} ends at {} vs batch {}",
            s.t1,
            b.t1
        );
        assert!(s.keyframe_sample_id.is_some());
    }
    assert_eq!(open.t1, head, "the open shot ends at the head: {open:?}");
    assert_eq!(
        open.t0,
        closed.last().unwrap().t1,
        "no gap before the open shot"
    );
    for w in shots.windows(2) {
        assert_eq!(w[0].t1, w[1].t0, "shots are contiguous");
    }
    // The closed shots were emitted downstream once each.
    assert_eq!(
        report.stages["shot_boundary"].items_done as usize,
        closed.len()
    );
    // The tone is "speech" to the scripted VAD: utterances of at most 15 s
    // cover the stream up to the head, minus what is still open.
    let utterances = report.stages["vad_stream"].items_done;
    assert!(
        (5..=7).contains(&utterances),
        "{utterances} utterances by {head}"
    );
}

/// Collects the speech ranges it receives with the last tick head seen
/// when each arrived, so a test can measure how far behind the stream an
/// utterance was released.
type Collected = Arc<Mutex<Vec<(vi_pipeline::SpeechItem, Option<Timestamp>)>>>;

struct Collector {
    got: Collected,
    head: Mutex<Option<Timestamp>>,
}

#[async_trait]
impl Operator for Collector {
    fn id(&self) -> &'static str {
        "collector"
    }
    fn version(&self) -> u32 {
        1
    }
    fn inputs(&self) -> &[ItemKind] {
        &[ItemKind::SpeechRange, ItemKind::Tick]
    }
    fn optional_inputs(&self) -> &[ItemKind] {
        &[ItemKind::Tick]
    }
    fn outputs(&self) -> &[ItemKind] {
        &[ItemKind::TranscriptSpan]
    }
    fn cost_estimate(&self, _: &InputSummary) -> CostEstimate {
        CostEstimate::default()
    }
    async fn run(&self, _ctx: &OpContext, input: OpInput) -> vi_core::Result<OpOutput> {
        match input.item {
            Item::SpeechRange(s) => {
                let head = *self.head.lock().unwrap();
                self.got.lock().unwrap().push(((*s).clone(), head));
            }
            Item::Tick { head } => *self.head.lock().unwrap() = Some(head),
            _ => {}
        }
        Ok(OpOutput::default())
    }
}

/// A root that plays an audio file as one-second chunks with a tick after
/// each, the way the live decoder delivers audio.
struct AudioRoot {
    path: std::path::PathBuf,
}

#[async_trait]
impl Operator for AudioRoot {
    fn id(&self) -> &'static str {
        "audio_root"
    }
    fn version(&self) -> u32 {
        1
    }
    fn inputs(&self) -> &[ItemKind] {
        &[ItemKind::Media]
    }
    fn outputs(&self) -> &[ItemKind] {
        &[ItemKind::AudioChunk, ItemKind::Tick]
    }
    fn cost_estimate(&self, _: &InputSummary) -> CostEstimate {
        CostEstimate::default()
    }
    async fn run(&self, ctx: &OpContext, _input: OpInput) -> vi_core::Result<OpOutput> {
        // The fixture is a plain 16-bit mono WAV: read its `data` chunk.
        let bytes = std::fs::read(&self.path)?;
        let data = bytes
            .windows(4)
            .position(|w| w == b"data")
            .map(|i| i + 8)
            .ok_or_else(|| ctx.err("no data chunk"))?;
        let samples: Vec<i16> = bytes[data..]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        let rate = fx::TONE_SILENCE_RATE;
        let mut n = 0;
        for (i, chunk) in samples.chunks(rate as usize).enumerate() {
            // Paced, so the collector's tick clock tracks the audio clock
            // within a chunk (an unpaced root would run minutes ahead).
            tokio::time::sleep(Duration::from_millis(25)).await;
            let t0 = Timestamp::new(i as i64, 1);
            let t1 = Timestamp::new((i * rate as usize + chunk.len()) as i64, rate);
            let head = t1;
            ctx.emit(Item::AudioChunk(Arc::new(vi_media::AudioChunk {
                t0,
                t1,
                sample_rate: rate,
                samples: chunk.to_vec(),
            })))
            .await?;
            ctx.emit(Item::Tick { head }).await?;
            n += 2;
        }
        Ok(OpOutput {
            emitted: n,
            stored: 0,
        })
    }
}

#[tokio::test]
async fn vad_stream_caps_utterances_and_flushes_soon_after_silence() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    let c = Arc::new(c);
    let got: Collected = Arc::new(Mutex::new(Vec::new()));
    let mut sched = Scheduler::new(idx.clone(), c.clone(), EventBus::default());
    let path = fx::tone_silence_path();
    sched.register_operator(
        "audio_root",
        Arc::new(move |_| Box::new(AudioRoot { path: path.clone() }) as Box<dyn Operator>),
    );
    sched.register_operator("vad_stream", Arc::new(|_| rms_vad_stream()));
    {
        let got = got.clone();
        sched.register_operator(
            "collector",
            Arc::new(move |_| {
                Box::new(Collector {
                    got: got.clone(),
                    head: Mutex::new(None),
                }) as Box<dyn Operator>
            }),
        );
    }
    // The media item: the tone file probed, no video track needed here.
    let probe = vi_media::probe(&c.media.worker, &fx::tone_silence_path())
        .await
        .unwrap();
    let mut media = live_media(&c, &SegmentFeed::new(fx::fixture_segments_dir())).await;
    media.tracks = probe.tracks(media.video.id);
    media.probe = probe;
    let report = sched
        .run_with_media(
            media,
            JobOptions {
                policy: Some("speech".into()),
                inline_policy: Some(policy(&["audio_root", "vad_stream", "collector"])),
                live: true,
                ..JobOptions::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(report.ok, "{report:?}");
    let got = got.lock().unwrap().clone();
    // 3 s of silence then 5 s of tone in every 8 s period: fifteen
    // utterances, each the tone plus padding.
    assert_eq!(
        got.len(),
        15,
        "{:?}",
        got.iter()
            .map(|(s, _)| (s.t0.to_string(), s.t1.to_string()))
            .collect::<Vec<_>>()
    );
    let pad = f64::from(c.models.vad.pad_ms) / 1000.0;
    for (i, (s, head)) in got.iter().enumerate() {
        let (t0, t1) = (s.t0.as_secs_f64(), s.t1.as_secs_f64());
        let period = i as f64 * fx::TONE_SILENCE_PERIOD_SECS;
        let tone_start = period + fx::TONE_SILENCE_SILENCE_SECS;
        let tone_end = period + fx::TONE_SILENCE_PERIOD_SECS;
        assert!(
            t1 - t0 <= MAX_UTTERANCE_SECS + 1e-9,
            "utterance {i} lasts {}",
            t1 - t0
        );
        assert!(
            (t0 - (tone_start - pad)).abs() < 0.1,
            "utterance {i} starts at {t0}"
        );
        assert!(
            (t1 - (tone_end + pad).min(fx::DURATION_SECS)).abs() < 0.1,
            "utterance {i} ends at {t1}"
        );
        assert_eq!(s.sample_rate, 16_000);
        let secs = s.samples.len() as f64 / 16_000.0;
        assert!(
            (secs - (t1 - t0)).abs() < 0.01,
            "utterance {i}: {secs} s of samples"
        );
        assert_eq!(s.index, i as u64);
        // Released within the flush wait of the tone's end (plus the one
        // second chunk it arrived in); the last one at the end of the file.
        if i + 1 < got.len() {
            let released = head
                .expect("a tick before the first utterance")
                .as_secs_f64();
            let wait = released - tone_end;
            assert!(
                wait <= FLUSH_AFTER_SILENCE_SECS + 1.0 + 1e-9,
                "utterance {i} released {wait:.2} s after its speech ended"
            );
        }
    }
    for w in got.windows(2) {
        assert!(w[0].0.t1 <= w[1].0.t0, "utterances are disjoint");
    }
    assert_eq!(report.stages["vad_stream"].items_done, 15);
    assert!(report.stopped_at.is_none(), "the stream ended on its own");
    assert!(report.stages["vad_stream"].last_t.is_some());
}

/// What the fake VLM answers for a shot (the request carries the shot
/// prompt) and for a scene.
const FAKE_SHOT_JSON: &str = r#"{"people":["3 people: a host, two guests"],"objects":[{"name":"chairs","count":2},{"name":"laptop","count":1}],"actions":["host waves"],"on_screen_text":["LIVE"],"summary":"A host greets two guests at a desk."}"#;
const FAKE_SCENE_JSON: &str = r#"{"summary":"A synthetic test pattern.","visible":"Coloured bars.","on_screen_text":[],"actions":[],"topics":["test"]}"#;

/// Request bodies a fake VLM received, and the most requests it had open
/// at once.
type Bodies = Arc<std::sync::Mutex<Vec<String>>>;
type Peak = Arc<std::sync::atomic::AtomicUsize>;

/// A fake OpenAI-compatible VLM: every chat request gets a fixed
/// description back after 150 ms as a stream with usage (1,000 prompt
/// tokens); the request bodies are kept, and the peak number of requests
/// in flight. It also transcribes (`audio/transcriptions`), always the
/// same sentence.
async fn fake_vlm_server() -> (std::net::SocketAddr, Bodies, Peak) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let bodies: Bodies = Arc::default();
    let peak: Peak = Arc::default();
    let open = Arc::new(AtomicUsize::new(0));
    let (seen, top) = (bodies.clone(), peak.clone());
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            let (seen, top, open) = (seen.clone(), top.clone(), open.clone());
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = vec![0u8; 1 << 16];
                let body_start = loop {
                    let r = sock.read(&mut chunk).await.unwrap_or(0);
                    if r == 0 {
                        break None;
                    }
                    buf.extend_from_slice(&chunk[..r]);
                    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..pos]).to_ascii_lowercase();
                        let len: usize = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .and_then(|v| v.trim().parse().ok())
                            .unwrap_or(0);
                        if buf.len() >= pos + 4 + len {
                            break Some(pos + 4);
                        }
                    }
                };
                let Some(start) = body_start else { return };
                if buf.starts_with(b"POST /v1/audio/transcriptions") {
                    // ASR: one segment, whatever the audio.
                    let json = r#"{"language":"en","duration":10.0,"text":"the speaker counts three apples","segments":[{"start":0.0,"end":4.0,"text":" the speaker counts three apples"}]}"#;
                    let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}", json.len());
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.shutdown().await;
                    return;
                }
                let now = open.fetch_add(1, Ordering::SeqCst) + 1;
                top.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                open.fetch_sub(1, Ordering::SeqCst);
                let body = String::from_utf8_lossy(&buf[start..]).to_string();
                let answer = if body.contains("80 words") {
                    FAKE_SHOT_JSON
                } else {
                    FAKE_SCENE_JSON
                };
                seen.lock().unwrap().push(body);
                let delta = serde_json::json!({"choices":[{"delta":{"content":answer},"finish_reason":"stop"}]});
                let usage = serde_json::json!({"choices":[],"usage":{"prompt_tokens":1000,"completion_tokens":0}});
                let sse = format!("data: {delta}\n\ndata: {usage}\n\ndata: [DONE]\n\n");
                let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{sse}", sse.len());
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    (addr, bodies, peak)
}

/// A config over `cache` whose `vlm_describe` role is the fake VLM at
/// `addr`, $0.01 a call (1,000 input tokens at $10 per million), taking
/// `concurrency` calls at once.
fn fake_vlm_config(
    cache: &std::path::Path,
    addr: std::net::SocketAddr,
    concurrency: u32,
) -> Config {
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    c.media.sample_max_dim = 320;
    c.media.cache_dir = cache.to_path_buf();
    c.providers.insert(
        "fake".into(),
        vi_core::config::ProviderConfig {
            adapter: "openai_compat".into(),
            base_url: Some(format!("http://{addr}/v1")),
            model: Some("fake-vl".into()),
            max_retries: Some(0),
            concurrency: Some(concurrency),
            pricing: Some(vi_core::config::Pricing {
                input_per_mtok: 10.0,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    c.roles.insert(
        "vlm_describe".into(),
        vi_core::config::RoleBinding {
            provider: "fake".into(),
            ..Default::default()
        },
    );
    c
}

/// A VAD stand-in: one speech range over the first ten seconds (silent
/// PCM; the fake ASR does not listen).
struct OneSpeechRange;

#[async_trait]
impl Operator for OneSpeechRange {
    fn id(&self) -> &'static str {
        "vad"
    }
    fn version(&self) -> u32 {
        1
    }
    fn inputs(&self) -> &[ItemKind] {
        &[ItemKind::Media]
    }
    fn outputs(&self) -> &[ItemKind] {
        &[ItemKind::SpeechRange]
    }
    fn cost_estimate(&self, _: &InputSummary) -> CostEstimate {
        CostEstimate::default()
    }
    async fn run(&self, ctx: &OpContext, input: OpInput) -> vi_core::Result<OpOutput> {
        let Item::Media(_) = input.item else {
            return Err(ctx.err("expected the media item"));
        };
        ctx.emit(Item::SpeechRange(Arc::new(vi_pipeline::SpeechItem {
            t0: Timestamp::ZERO,
            t1: Timestamp::from_secs(10),
            samples: vec![0i16; 160_000].into(),
            sample_rate: 16_000,
            index: 0,
        })))
        .await?;
        Ok(OpOutput {
            emitted: 1,
            stored: 0,
        })
    }
}

/// A fine pass that replays `asr` and `subtitle_import` leaves their rows
/// alone and hands each line on once: a replaying stage re-emits what is
/// stored and is not also run on the media item (which for `asr` deletes
/// its transcript and for `subtitle_import` imports the sidecars again
/// under new ids and emits them twice).
#[tokio::test]
async fn a_fine_pass_replays_asr_and_subtitles_without_rewriting_them() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("videos");
    let media = seed_incoming(&cache);
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let (addr, bodies, _) = fake_vlm_server().await;
    let mut c = fake_vlm_config(&cache, addr, 1);
    c.providers.insert(
        "whisper".into(),
        vi_core::config::ProviderConfig {
            adapter: "openai_compat".into(),
            base_url: Some(format!("http://{addr}/v1")),
            model: Some("fake-whisper".into()),
            max_retries: Some(0),
            ..Default::default()
        },
    );
    c.roles.insert(
        "asr".into(),
        vi_core::config::RoleBinding {
            provider: "whisper".into(),
            ..Default::default()
        },
    );
    let coarse = ["subtitle_import", "vad", "asr", "sample", "shot_boundary"];
    c.policy.insert("coarse".into(), policy(&coarse));
    let mut fine = policy(&coarse);
    fine.fine = vec!["scenes".into(), "vlm_describe".into()];
    c.policy.insert("fine".into(), fine);
    let mut sched = Scheduler::new(idx.clone(), Arc::new(c), EventBus::default());
    sched.register_operator(
        "vad",
        Arc::new(|_: &Config| Box::new(OneSpeechRange) as Box<dyn Operator>),
    );
    let run = |p: &str, src: std::path::PathBuf| {
        let sched = sched.clone();
        let p = p.to_string();
        async move {
            sched
                .run(
                    Source::Path(src),
                    JobOptions {
                        policy: Some(p),
                        ..JobOptions::default()
                    },
                    CancellationToken::new(),
                )
                .await
                .unwrap()
        }
    };
    let first = run("coarse", media).await;
    assert!(first.ok, "{first:?}");
    let video = idx.list_videos().await.unwrap()[0].clone();
    let rows = |op: &'static str| {
        let idx = idx.clone();
        async move {
            idx.spans_by_operator(video.id, op)
                .await
                .unwrap()
                .into_iter()
                .filter_map(|s| match s {
                    vi_core::model::Span::Transcript(t) => Some((t.id, t.text)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        }
    };
    let asr_before = rows("asr").await;
    let subs_before = rows("subtitle_import").await;
    assert_eq!(asr_before.len(), 1, "{asr_before:?}");
    assert!(asr_before[0].1.contains("three apples"));
    assert!(!subs_before.is_empty());

    let media = cache.join(format!("{}.mp4", video.content_hash));
    let second = run("fine", media).await;
    assert!(second.ok && !second.skipped, "{second:?}");
    assert!(second.stages["asr"].replayed, "{:?}", second.stages["asr"]);
    assert_eq!(second.stages["asr"].items_done, 1);
    assert!(second.stages["subtitle_import"].replayed);
    assert_eq!(second.stages["vad"].status, StageStatus::Skipped);
    assert!(
        second.stages["vlm_describe"].items_done >= 1,
        "{:?}",
        second.stages["vlm_describe"]
    );
    // Same rows, same ids.
    assert_eq!(rows("asr").await, asr_before);
    assert_eq!(rows("subtitle_import").await, subs_before);
    // The first scene's prompt has the ASR line and each subtitle line once.
    let bodies = bodies.lock().unwrap();
    let first_shot = bodies
        .iter()
        .find(|b| b.contains("caption kw0 "))
        .expect("a request carrying the first caption");
    assert_eq!(
        first_shot.matches("three apples").count(),
        1,
        "{first_shot}"
    );
    assert_eq!(
        first_shot.matches("caption kw0 ").count(),
        1,
        "{first_shot}"
    );
}
