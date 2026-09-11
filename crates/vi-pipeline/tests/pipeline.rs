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
async fn unknown_and_planned_operators_are_clear_errors() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let sched = Scheduler::new(idx, config(), EventBus::default());
    let err = sched.plan(Some("lecture_default")).unwrap_err();
    assert!(matches!(err, vi_core::Error::Unsupported(_)), "{err}");
    assert!(sched.plan(Some("does_not_exist")).is_err());
    let (name, _, dag) = sched.plan(None).unwrap();
    assert_eq!(name, "m0");
    assert_eq!(dag.stage_names()[0], "sample");
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
