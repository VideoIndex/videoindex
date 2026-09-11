//! Runs a job: acquire, probe, register the Video, build the DAG from the
//! policy, run operators as tokio tasks joined by bounded channels, checkpoint
//! every stage, emit progress events.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use vi_core::config::{Config, IndexPolicy};
use vi_core::model::{IndexState, JobState, StageState, StageStatus, Video};
use vi_core::{Error, Event, EventBus, JobId, Result, VideoId};
use vi_index::Storage;
use vi_media::{Acquirer, LocalFile, Source};

use crate::dag::Dag;
use crate::operator::*;
use crate::ops;

/// Per-job options.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct JobOptions {
    /// Policy name; `None` uses `config.default_policy`.
    pub policy: Option<String>,
    /// Skip stages an unfinished earlier job for the same video completed.
    pub resume: bool,
    /// Re-run even if the video is already indexed by this policy.
    pub force: bool,
}

/// What a finished job reports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobReport {
    /// Job id.
    pub job_id: JobId,
    /// Video id (stable across re-runs of the same content).
    pub video_id: VideoId,
    /// Whether every stage completed.
    pub ok: bool,
    /// Per-stage outcome.
    pub stages: BTreeMap<String, StageState>,
    /// Wall-clock seconds for the whole job.
    pub elapsed_secs: f64,
    /// Wall-clock seconds spent in acquire + probe.
    pub acquire_secs: f64,
    /// Resulting index state.
    pub index_state: IndexState,
    /// Whether this run was skipped because the video was already indexed.
    pub skipped: bool,
}

/// The scheduler.
#[derive(Clone)]
pub struct Scheduler {
    storage: Arc<dyn Storage>,
    config: Arc<Config>,
    events: EventBus,
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler").finish()
    }
}

impl Scheduler {
    /// A scheduler over a storage backend.
    pub fn new(storage: Arc<dyn Storage>, config: Arc<Config>, events: EventBus) -> Self {
        Self {
            storage,
            config,
            events,
        }
    }

    /// Event bus, for subscribers.
    pub fn events(&self) -> &EventBus {
        &self.events
    }

    /// Resolve the policy and build the operator DAG for it, without running.
    pub fn plan(&self, policy_name: Option<&str>) -> Result<(String, IndexPolicy, Dag)> {
        let name = policy_name
            .unwrap_or(&self.config.default_policy)
            .to_string();
        let policy = self.config.policy(&name)?.clone();
        let mut operators = Vec::new();
        for op_name in policy.operators() {
            match ops::build(op_name, &self.config) {
                Some(op) => operators.push(op),
                None if ops::PLANNED.contains(&op_name) => {
                    return Err(Error::Unsupported(format!(
                        "operator '{op_name}' in policy '{name}' is not implemented yet; available: {}",
                        ops::AVAILABLE.join(", ")
                    )))
                }
                None => {
                    return Err(Error::invalid(format!(
                        "unknown operator '{op_name}' in policy '{name}'; available: {}",
                        ops::AVAILABLE.join(", ")
                    )))
                }
            }
        }
        if operators.is_empty() {
            return Err(Error::invalid(format!("policy '{name}' has no operators")));
        }
        let dag = Dag::build(operators)?;
        Ok((name, policy, dag))
    }

    /// Index one source.
    pub async fn run(
        &self,
        source: Source,
        opts: JobOptions,
        cancel: CancellationToken,
    ) -> Result<JobReport> {
        let started = Instant::now();
        let job_id = JobId::new();
        let (policy_name, policy, dag) = self.plan(opts.policy.as_deref())?;

        self.events.emit(Event::JobStarted {
            job: job_id,
            video: None,
            source: source.uri(),
        });

        // ---- acquire + probe --------------------------------------------
        let acquirer = LocalFile;
        if !acquirer.handles(&source) {
            return Err(Error::Unsupported(format!(
                "only local files can be indexed in M0; got {}",
                source.uri()
            )));
        }
        let acquired = acquirer.acquire(&source).await?;
        let probe = vi_media::probe(&self.config.media.worker, &acquired.path).await?;
        let vstream = probe.video_stream().ok_or_else(|| {
            Error::media(format!("{} has no video stream", acquired.path.display()))
        })?;
        let expected_samples = (probe.duration.as_secs_f64() * policy.sample_fps).ceil() as u64;
        let _ = vstream;

        // Stable video id across re-indexing of the same content.
        let existing = self
            .storage
            .find_video_by_hash(&acquired.content_hash)
            .await?;
        let (video, tracks, fresh) = match existing {
            Some(mut v) => {
                v.source_uri = acquired.source_uri.clone();
                v.probe = serde_json::to_value(&probe)?;
                v.duration = probe.duration;
                if v.title.is_none() {
                    v.title = acquired.title.clone().or_else(|| probe.title());
                }
                let tracks = self.storage.tracks(v.id).await?;
                let tracks = if tracks.is_empty() {
                    let t = probe.tracks(v.id);
                    self.storage.put_tracks(&t).await?;
                    t
                } else {
                    tracks
                };
                self.storage.put_video(&v).await?;
                (v, tracks, false)
            }
            None => {
                let id = VideoId::new();
                let v = Video {
                    id,
                    source_uri: acquired.source_uri.clone(),
                    content_hash: acquired.content_hash.clone(),
                    title: acquired.title.clone().or_else(|| probe.title()),
                    description: None,
                    channel: None,
                    published_at: None,
                    duration: probe.duration,
                    start_wallclock: None,
                    probe: serde_json::to_value(&probe)?,
                    index_state: IndexState::Acquired,
                    created_at: Utc::now(),
                };
                let tracks = probe.tracks(id);
                self.storage.put_video(&v).await?;
                self.storage.put_tracks(&tracks).await?;
                (v, tracks, true)
            }
        };
        let acquire_secs = started.elapsed().as_secs_f64();
        info!(
            video = %video.id,
            duration = %probe.duration,
            fresh,
            "acquired and probed in {acquire_secs:.2}s"
        );

        // ---- job state / resume -----------------------------------------
        let stage_names = dag.stage_names();
        let mut state = JobState::new(
            job_id,
            video.id,
            acquired.source_uri.clone(),
            policy_name.clone(),
            stage_names.iter().cloned(),
        );
        if !fresh && !opts.force {
            if video.index_state != IndexState::Acquired && video.index_state != IndexState::Failed
            {
                info!(video = %video.id, "already indexed ({:?}); use force to re-run", video.index_state);
                let mut stages = state.stages.clone();
                for s in stages.values_mut() {
                    s.status = StageStatus::Skipped;
                }
                return Ok(JobReport {
                    job_id,
                    video_id: video.id,
                    ok: true,
                    stages,
                    elapsed_secs: started.elapsed().as_secs_f64(),
                    acquire_secs,
                    index_state: video.index_state,
                    skipped: true,
                });
            }
            if opts.resume {
                let prior = self
                    .storage
                    .list_jobs()
                    .await?
                    .into_iter()
                    .filter(|j| j.video_id == video.id && !j.finished && j.policy == policy_name)
                    .max_by_key(|j| j.updated_at);
                if let Some(p) = prior {
                    for (name, st) in &p.stages {
                        if st.status == StageStatus::Complete {
                            // Downstream-only stages can be skipped when their
                            // producer is also complete; `sample` re-runs feed
                            // consumers, so only skip when everything upstream
                            // of a stage is complete too.
                            state.stage_mut(name).status = StageStatus::Complete;
                            state.stage_mut(name).items_done = st.items_done;
                        }
                    }
                    info!(prior = %p.job_id, "resuming from checkpoint");
                }
            }
        }
        self.storage.checkpoint(job_id, &state).await?;

        // A stage can only be skipped if every stage it feeds is complete as
        // well, otherwise consumers would starve. Simplest correct rule for
        // M0: skip all or nothing.
        let all_complete = stage_names.iter().all(|s| state.is_complete(s));
        if all_complete {
            state.finished = true;
            self.storage.checkpoint(job_id, &state).await?;
            self.storage
                .set_index_state(video.id, IndexState::Coarse)
                .await?;
            return Ok(JobReport {
                job_id,
                video_id: video.id,
                ok: true,
                stages: state.stages.clone(),
                elapsed_secs: started.elapsed().as_secs_f64(),
                acquire_secs,
                index_state: IndexState::Coarse,
                skipped: true,
            });
        }
        for s in state.stages.values_mut() {
            *s = StageState::default();
        }

        // ---- wire the DAG -----------------------------------------------
        let media = Arc::new(MediaItem {
            acquired,
            probe,
            video: video.clone(),
            tracks,
            expected_samples,
        });
        let (operators, consumers, roots) = dag.into_parts();
        let n = operators.len();
        let capacity = (self.config.media.worker.max_in_flight_frames / 2).clamp(2, 16);
        let mut senders: Vec<Option<mpsc::Sender<Item>>> = Vec::with_capacity(n);
        let mut receivers: Vec<Option<mpsc::Receiver<Item>>> = Vec::with_capacity(n);
        for _ in 0..n {
            let (tx, rx) = mpsc::channel::<Item>(capacity);
            senders.push(Some(tx));
            receivers.push(Some(rx));
        }
        let emitters: Vec<Emitter> = consumers
            .iter()
            .map(|c| Emitter::new(c.iter().filter_map(|j| senders[*j].clone()).collect()))
            .collect();
        // Seed the roots with the media item.
        let root_senders: Vec<mpsc::Sender<Item>> =
            roots.iter().filter_map(|r| senders[*r].clone()).collect();
        // Drop our copies so channels close once producers finish.
        drop(senders);
        for tx in root_senders {
            let _ = tx.send(Item::Media(media.clone())).await;
        }

        let job_cancel = cancel.child_token();
        let mut handles = Vec::with_capacity(n);
        for (i, op) in operators.into_iter().enumerate() {
            let stage = op.id().to_string();
            let ctx = OpContext {
                job: job_id,
                video: video.id,
                stage: stage.clone(),
                storage: self.storage.clone(),
                config: self.config.clone(),
                policy: policy.clone(),
                worker: self.config.media.worker.clone(),
                emitter: emitters[i].clone(),
                cancel: job_cancel.clone(),
                events: self.events.clone(),
                expected_items: Some(expected_samples),
            };
            let rx = receivers[i]
                .take()
                .ok_or_else(|| Error::Other("receiver already taken".into()))?;
            let events = self.events.clone();
            let cancel_on_fail = job_cancel.clone();
            handles.push(tokio::spawn(async move {
                let result = run_stage(op, ctx, rx, &events).await;
                if result.is_err() {
                    cancel_on_fail.cancel();
                }
                (stage, result)
            }));
        }
        // Emitters are cloned into contexts; drop ours so channels can close.
        drop(emitters);

        let mut ok = true;
        for h in handles {
            let (stage, result) = h
                .await
                .map_err(|e| Error::Other(format!("stage task panicked: {e}")))?;
            let st = state.stage_mut(&stage);
            match result {
                Ok(items) => {
                    st.status = StageStatus::Complete;
                    st.items_done = items;
                }
                Err(e) => {
                    ok = false;
                    st.status = StageStatus::Failed;
                    st.error = Some(e.to_string());
                }
            }
            state.updated_at = Utc::now();
            self.storage.checkpoint(job_id, &state).await?;
        }

        let index_state = if ok {
            IndexState::Coarse
        } else {
            IndexState::Failed
        };
        self.storage.set_index_state(video.id, index_state).await?;
        state.finished = true;
        state.updated_at = Utc::now();
        self.storage.checkpoint(job_id, &state).await?;

        let elapsed = started.elapsed();
        let summary = format!(
            "{} stages, {} samples, {:.1}s",
            state.stages.len(),
            state
                .stages
                .get("sample")
                .map(|s| s.items_done)
                .unwrap_or(0),
            elapsed.as_secs_f64()
        );
        self.events.emit(Event::JobFinished {
            job: job_id,
            ok,
            elapsed_ms: elapsed.as_millis() as u64,
            summary,
        });
        Ok(JobReport {
            job_id,
            video_id: video.id,
            ok,
            stages: state.stages,
            elapsed_secs: elapsed.as_secs_f64(),
            acquire_secs,
            index_state,
            skipped: false,
        })
    }
}

/// Drive one operator over its input channel. Returns items emitted.
async fn run_stage(
    op: Box<dyn Operator>,
    ctx: OpContext,
    mut rx: mpsc::Receiver<Item>,
    events: &EventBus,
) -> Result<u64> {
    let started = Instant::now();
    events.emit(Event::StageStarted {
        job: ctx.job,
        stage: ctx.stage.clone(),
    });
    let mut emitted = 0u64;
    let result: Result<()> = async {
        loop {
            let item = tokio::select! {
                biased;
                _ = ctx.cancel.cancelled() => return Err(Error::Cancelled),
                item = rx.recv() => item,
            };
            let Some(item) = item else { break };
            let out = op.run(&ctx, OpInput { item }).await?;
            emitted += out.emitted;
        }
        let out = op.finish(&ctx).await?;
        emitted += out.emitted;
        Ok(())
    }
    .await;
    // Close our end so upstream producers see the consumer gone.
    rx.close();
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(()) => {
            debug!(stage = %ctx.stage, emitted, elapsed_ms, "stage finished");
            events.emit(Event::StageFinished {
                job: ctx.job,
                stage: ctx.stage.clone(),
                items: emitted,
                elapsed_ms,
            });
            Ok(emitted)
        }
        Err(e) => {
            warn!(stage = %ctx.stage, error = %e, "stage failed");
            events.emit(Event::StageFailed {
                job: ctx.job,
                stage: ctx.stage.clone(),
                error: e.to_string(),
            });
            Err(e)
        }
    }
}
