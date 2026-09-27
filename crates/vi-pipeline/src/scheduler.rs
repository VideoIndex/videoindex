//! Runs a job: acquire, probe, register the Video, build the DAG from the
//! policy, run operators as tokio tasks joined by bounded channels, checkpoint
//! every stage, emit progress events.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use vi_core::config::{Config, IndexPolicy};
use vi_core::model::{
    IndexState, JobState, Provenance, Segment, SegmentLevel, StageState, StageStatus, Video,
};
use vi_core::SegmentId;
use vi_core::{Error, Event, EventBus, JobId, Result, VideoId};
use vi_index::Storage;
use vi_media::{Acquired, Acquirer, Http, LocalFile, ObjectStore, Source, YtDlp};
use vi_providers::ProviderRegistry;

use crate::dag::Dag;
use crate::operator::*;
use crate::ops;

/// Per-job options.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct JobOptions {
    /// Policy name; `None` uses `config.default_policy`.
    pub policy: Option<String>,
    /// Kept for compatibility: stages whose outputs are in the operator
    /// cache are always skipped or replayed unless `force` is set.
    pub resume: bool,
    /// Ignore the operator output cache and re-run every stage.
    pub force: bool,
    /// A policy supplied by the caller instead of one named in the config
    /// (Python-defined policies). `policy` then only names it in reports.
    pub inline_policy: Option<IndexPolicy>,
    /// A live job (live C3): the media is a growing recording with no
    /// duration. Nothing is cached or replayed (the recording is unique),
    /// the budget is [`Budget::for_live`], `index_state` is left to the
    /// caller, every stage's `last_t` is checkpointed while the job runs,
    /// and cancelling the job is its normal end: stages complete and
    /// `JobReport.stopped_at` carries the head reached.
    #[serde(default)]
    pub live: bool,
}

/// Builds an operator the config does not know about: the extension point
/// for operators implemented in Python or JS, or registered by an embedding
/// application. Called once per job.
pub type OperatorFactory = Arc<dyn Fn(&Config) -> Box<dyn Operator> + Send + Sync>;

/// Budget outcome for a job.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BudgetReport {
    /// Provider spend in USD.
    pub cost_usd: f64,
    /// Cost ceiling, if any.
    pub max_cost_usd: Option<f64>,
    /// Wall-clock ceiling in seconds, if any.
    pub max_wallclock_secs: Option<f64>,
    /// Which limit stopped provider calls, if one did (for a live job: at
    /// the end).
    pub exhausted: Option<String>,
    /// Live jobs: the rolling cost ceiling per hour of stream time.
    #[serde(default)]
    pub max_cost_usd_per_hour: Option<f64>,
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
    /// Budget outcome.
    pub budget: BudgetReport,
    /// Live jobs that were cancelled: the stream time reached, the latest
    /// `last_t` over the stages. `None` otherwise.
    #[serde(default)]
    pub stopped_at: Option<vi_core::Timestamp>,
}

/// The scheduler.
#[derive(Clone)]
pub struct Scheduler {
    storage: Arc<dyn Storage>,
    config: Arc<Config>,
    events: EventBus,
    providers: Arc<ProviderRegistry>,
    custom: HashMap<String, OperatorFactory>,
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler").finish()
    }
}

impl Scheduler {
    /// A scheduler over a storage backend.
    pub fn new(storage: Arc<dyn Storage>, config: Arc<Config>, events: EventBus) -> Self {
        let providers = Arc::new(ProviderRegistry::new(
            config.clone(),
            CancellationToken::new(),
        ));
        vi_perceive::OnnxLocal::register(&providers);
        Self::with_providers(storage, config, events, providers)
    }

    /// A scheduler sharing an existing provider registry (one per process
    /// keeps rate limits global).
    pub fn with_providers(
        storage: Arc<dyn Storage>,
        config: Arc<Config>,
        events: EventBus,
        providers: Arc<ProviderRegistry>,
    ) -> Self {
        Self {
            storage,
            config,
            events,
            providers,
            custom: HashMap::new(),
        }
    }

    /// Register an operator built outside this crate under the name policies
    /// use for it. A custom operator with a built-in operator's name replaces
    /// the built-in one, which is how an experiment swaps one stage.
    pub fn register_operator(&mut self, name: impl Into<String>, factory: OperatorFactory) {
        self.custom.insert(name.into(), factory);
    }

    /// Names of registered custom operators.
    pub fn custom_operators(&self) -> impl Iterator<Item = &str> {
        self.custom.keys().map(String::as_str)
    }

    /// The provider registry.
    pub fn providers(&self) -> &Arc<ProviderRegistry> {
        &self.providers
    }

    /// Event bus, for subscribers.
    pub fn events(&self) -> &EventBus {
        &self.events
    }

    /// Resolve the policy and build the operator DAG for it, without running.
    pub fn plan(&self, policy_name: Option<&str>) -> Result<(String, IndexPolicy, Dag)> {
        self.plan_with(policy_name, None)
    }

    /// Like [`Self::plan`], with an inline policy taking precedence over the
    /// config's named ones.
    pub fn plan_with(
        &self,
        policy_name: Option<&str>,
        inline: Option<&IndexPolicy>,
    ) -> Result<(String, IndexPolicy, Dag)> {
        let (name, policy) = match inline {
            Some(p) => (policy_name.unwrap_or("inline").to_string(), p.clone()),
            None => {
                let name = policy_name
                    .map(str::to_string)
                    .unwrap_or_else(|| self.config.resolve_default_policy());
                let policy = self.config.policy(&name)?.clone();
                (name, policy)
            }
        };
        let mut operators = Vec::new();
        for op_name in policy.operators() {
            let built = match self.custom.get(op_name) {
                Some(factory) => Some(factory(&self.config)),
                None => ops::build(op_name, &self.config),
            };
            match built {
                Some(op) => operators.push(op),
                None if ops::PLANNED.contains(&op_name) => {
                    return Err(Error::Unsupported(format!(
                        "operator '{op_name}' in policy '{name}' is not implemented yet; available: {}",
                        ops::AVAILABLE.join(", ")
                    )))
                }
                None => {
                    let mut available: Vec<&str> = ops::AVAILABLE.to_vec();
                    available.extend(self.custom_operators());
                    return Err(Error::invalid(format!(
                        "unknown operator '{op_name}' in policy '{name}'; available: {}",
                        available.join(", ")
                    )))
                }
            }
        }
        if operators.is_empty() {
            return Err(Error::invalid(format!("policy '{name}' has no operators")));
        }
        for op in &operators {
            for role in op.required_roles() {
                if !self.providers.has_role(role) {
                    return Err(Error::Provider(format!(
                        "operator '{}' in policy '{name}' needs a provider for role '{role}'; add `[roles] {role} = {{ provider = \"...\" }}` to the config (see config/gcp-a100.toml)",
                        op.id()
                    )));
                }
            }
        }
        let dag = Dag::build(operators)?;
        Ok((name, policy, dag))
    }

    /// The acquirer for a source, or an error naming why none applies.
    fn acquirer_for(&self, source: &Source) -> Result<Box<dyn Acquirer>> {
        let local = LocalFile::with_cache(&self.config.media.cache_dir);
        if local.handles(source) {
            return Ok(Box::new(local));
        }
        let yt = YtDlp::new(&self.config.media.cache_dir);
        if yt.handles(source) {
            return Ok(Box::new(yt));
        }
        let http = Http::new(
            &self.config.media.cache_dir,
            self.config.media.download.clone(),
        );
        if http.handles(source) {
            return Ok(Box::new(http));
        }
        let obj = ObjectStore::new(
            &self.config.media.cache_dir,
            self.config.media.download.clone(),
        );
        if obj.handles(source) {
            return Ok(Box::new(obj));
        }
        Err(Error::Unsupported(format!(
            "no acquirer for {}; local files and directories, video-site URLs (yt-dlp), https://…/file.mp4, and s3:// gs:// az:// r2:// objects are supported",
            source.uri()
        )))
    }

    /// Expand a directory or playlist into individual sources.
    pub async fn expand(&self, source: &Source) -> Result<Vec<Source>> {
        Ok(self.acquirer_for(source)?.expand(source).await?)
    }

    /// Write chapter segments from sidecar metadata, else container chapters.
    async fn import_chapters(
        &self,
        video: &Video,
        acquired: &Acquired,
        probe: &vi_media::Probe,
    ) -> Result<u64> {
        let chapters: Vec<(vi_core::Timestamp, vi_core::Timestamp, Option<String>)> =
            if !acquired.chapters.is_empty() {
                acquired
                    .chapters
                    .iter()
                    .map(|c| (c.t0, c.t1, c.title.clone()))
                    .collect()
            } else {
                probe
                    .chapters
                    .iter()
                    .map(|c| (c.t0, c.t1, c.title.clone()))
                    .collect()
            };
        self.storage
            .delete_segments(video.id, SegmentLevel::Chapter)
            .await?;
        if chapters.is_empty() {
            return Ok(0);
        }
        let prov = Provenance::local(
            "chapters_import",
            1,
            serde_json::json!({
                "source": if acquired.chapters.is_empty() { "container" } else { "info.json" },
                "count": chapters.len(),
            }),
        );
        self.storage.put_provenance(&prov).await?;
        let segments: Vec<Segment> = chapters
            .into_iter()
            .filter(|(t0, t1, _)| t1 > t0)
            .map(|(t0, t1, title)| Segment {
                id: SegmentId::new(),
                video_id: video.id,
                level: SegmentLevel::Chapter,
                parent_id: None,
                t0,
                t1,
                keyframe_sample_id: None,
                title,
                summary: None,
                provenance_id: prov.id,
            })
            .collect();
        self.storage.put_segments(&segments).await?;
        Ok(segments.len() as u64)
    }

    /// Index one source: acquire, probe, register the video, then run the
    /// policy over it (the same machinery as [`Self::run_with_media`]).
    pub async fn run(
        &self,
        source: Source,
        opts: JobOptions,
        cancel: CancellationToken,
    ) -> Result<JobReport> {
        let started = Instant::now();
        let job_id = JobId::new();
        let (policy_name, policy, dag) =
            self.plan_with(opts.policy.as_deref(), opts.inline_policy.as_ref())?;

        self.events.emit(Event::JobStarted {
            job: job_id,
            video: None,
            source: source.uri(),
        });

        // ---- acquire + probe --------------------------------------------
        let acquirer = self.acquirer_for(&source)?;
        let acquired = acquirer.acquire(&source).await?;
        let probe = vi_media::probe(&self.config.media.worker, &acquired.path).await?;
        let vstream = probe.video_stream().ok_or_else(|| {
            Error::media(format!("{} has no video stream", acquired.path.display()))
        })?;
        // A file has a duration; a feed under `follow` does not (live C2).
        let duration = probe.duration.unwrap_or_default();
        let expected_samples = (duration.as_secs_f64() * policy.sample_fps).ceil() as u64;
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
                v.duration = duration;
                if acquired.title.is_some() {
                    v.title = acquired.title.clone();
                } else if v.title.is_none() {
                    v.title = probe.title();
                }
                if acquired.description.is_some() {
                    v.description = acquired.description.clone();
                }
                if acquired.channel.is_some() {
                    v.channel = acquired.channel.clone();
                }
                if acquired.published_at.is_some() {
                    v.published_at = acquired.published_at;
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
                    description: acquired.description.clone(),
                    channel: acquired.channel.clone(),
                    published_at: acquired.published_at,
                    duration,
                    start_wallclock: None,
                    probe: serde_json::to_value(&probe)?,
                    index_state: IndexState::Acquired,
                    created_at: Utc::now(),
                    watermark: None,
                    live_ended_at: None,
                };
                let tracks = probe.tracks(id);
                self.storage.put_video(&v).await?;
                self.storage.put_tracks(&tracks).await?;
                (v, tracks, true)
            }
        };
        let chapters = self.import_chapters(&video, &acquired, &probe).await?;
        let acquire_secs = started.elapsed().as_secs_f64();
        info!(chapters, "metadata imported");
        info!(
            video = %video.id,
            duration = %duration,
            fresh,
            "acquired and probed in {acquire_secs:.2}s"
        );
        let media = MediaItem {
            acquired,
            probe,
            video,
            tracks,
            expected_samples: Some(expected_samples),
        };
        self.run_prepared(Prepared {
            job_id,
            started,
            acquire_secs,
            policy_name,
            policy,
            dag,
            media,
            opts,
            cancel,
        })
        .await
    }

    /// Run the policy over media the caller has already acquired, probed
    /// and described (live C3): the live indexer's entry point, where the
    /// media is a growing recording and the caller owns the `Video` row
    /// (identity hash, `index_state = live`). The video and its tracks are
    /// upserted so the rows operators reference exist; with
    /// `JobOptions.live` nothing is cached, the budget rolls per hour of
    /// stream time, `index_state` is not written at the end, and every
    /// stage's `last_t` is checkpointed every second and reported.
    pub async fn run_with_media(
        &self,
        media: MediaItem,
        opts: JobOptions,
        cancel: CancellationToken,
    ) -> Result<JobReport> {
        let started = Instant::now();
        let job_id = JobId::new();
        let (policy_name, policy, dag) =
            self.plan_with(opts.policy.as_deref(), opts.inline_policy.as_ref())?;
        self.events.emit(Event::JobStarted {
            job: job_id,
            video: Some(media.video.id),
            source: media.acquired.source_uri.clone(),
        });
        self.storage.put_video(&media.video).await?;
        if !media.tracks.is_empty() {
            self.storage.put_tracks(&media.tracks).await?;
        }
        self.run_prepared(Prepared {
            job_id,
            started,
            acquire_secs: 0.0,
            policy_name,
            policy,
            dag,
            media,
            opts,
            cancel,
        })
        .await
    }

    /// The part of a job after acquisition: plan the cache, wire the DAG,
    /// run the stages, checkpoint, report.
    async fn run_prepared(&self, prepared: Prepared) -> Result<JobReport> {
        let Prepared {
            job_id,
            started,
            acquire_secs,
            policy_name,
            policy,
            dag,
            media,
            opts,
            cancel,
        } = prepared;
        let live = opts.live;
        let video = media.video.clone();
        let duration = media.probe.duration.unwrap_or_default();
        let expected_samples = media.expected_samples;

        // ---- operator output cache ---------------------------------------
        // Every completed stage leaves a marker keyed by the content hash,
        // operator id and version, and the operator's cache parameters
        // (provider, model, thresholds). A marker means "these outputs are
        // in the index"; a stage with a marker is skipped when no running
        // consumer needs its items, replayed from storage when one does and
        // the operator can, and re-run otherwise. A live recording is
        // unique and unfinished: nothing is cached or replayed.
        let stage_names = dag.stage_names();
        let mut state = JobState::new(
            job_id,
            video.id,
            media.acquired.source_uri.clone(),
            policy_name.clone(),
            stage_names.iter().cloned(),
        );
        let budget = Arc::new(if live {
            Budget::for_live(&policy)?
        } else {
            Budget::for_policy(&policy, duration.as_secs_f64())?
        });
        let media = Arc::new(media);
        let (operators, consumers, roots) = dag.into_parts();
        let n = operators.len();

        // Cache keys and markers per stage.
        let mut keys: Vec<String> = Vec::with_capacity(n);
        let mut cached: Vec<bool> = Vec::with_capacity(n);
        let stage_failures: Vec<Arc<StageFailures>> =
            (0..n).map(|_| Arc::new(StageFailures::default())).collect();
        let stage_clocks: Vec<Arc<StageClock>> =
            (0..n).map(|_| Arc::new(StageClock::default())).collect();
        let make_ctx = |i: usize, op: &dyn Operator, emitter: Emitter| OpContext {
            job: job_id,
            video: video.id,
            stage: op.id().to_string(),
            storage: self.storage.clone(),
            config: self.config.clone(),
            providers: self.providers.clone(),
            policy: policy.clone(),
            worker: self.config.media.worker.clone(),
            emitter,
            cancel: cancel.child_token(),
            events: self.events.clone(),
            expected_items: expected_samples,
            budget: budget.clone(),
            failures: stage_failures[i].clone(),
            live,
            clock: stage_clocks[i].clone(),
        };
        for (i, op) in operators.iter().enumerate() {
            let probe_ctx = make_ctx(i, op.as_ref(), Emitter::none());
            let params = op.cache_params(&probe_ctx);
            let key = cache_key(&media.acquired.content_hash, op.as_ref(), &params);
            let has = !live && !opts.force && self.cache_marker_exists(&key);
            keys.push(key);
            cached.push(has);
        }
        // A cached stage runs anyway when a running consumer needs its
        // items and it cannot replay them. Resolve in reverse topological
        // order so consumers are decided first.
        #[derive(Clone, Copy, PartialEq, Debug)]
        enum Plan {
            Run,
            Replay,
            Skip,
        }
        let mut plan = vec![Plan::Run; n];
        for i in (0..n).rev() {
            if !cached[i] {
                plan[i] = Plan::Run;
                continue;
            }
            // Only a consumer that *runs* needs this stage's items: a
            // replaying consumer re-emits its own stored rows and reads
            // nothing but the media item. Counting replays here made a
            // fine pass re-decode and re-OCR every video, because `scenes`
            // needed shots, shots replayed, and the replay was taken as a
            // need for frames from `sample`, which cannot replay.
            let consumer_needs_items = consumers[i].iter().any(|c| plan[*c] == Plan::Run);
            plan[i] = if !consumer_needs_items {
                Plan::Skip
            } else if operators[i].replay_supported() {
                Plan::Replay
            } else {
                Plan::Run
            };
        }
        // A replay whose own producers re-run would duplicate nothing (the
        // operator replaces its outputs), but a running producer means the
        // inputs may differ, so run instead.
        for i in 0..n {
            if plan[i] == Plan::Replay {
                let producer_runs =
                    (0..n).any(|p| consumers[p].contains(&i) && plan[p] == Plan::Run);
                if producer_runs {
                    plan[i] = Plan::Run;
                }
            }
        }
        // A stage whose producer re-runs sees new inputs: its cached outputs
        // (rows keyed by the producer's rows) are gone, so it must run too.
        // Operators are in topological order, so one forward pass settles it.
        for i in 0..n {
            if plan[i] == Plan::Run {
                continue;
            }
            let producer_runs = consumers
                .iter()
                .enumerate()
                .any(|(p, c)| c.contains(&i) && plan[p] == Plan::Run);
            if producer_runs {
                plan[i] = Plan::Run;
            }
        }
        let all_skipped = plan.iter().all(|p| *p == Plan::Skip);
        if all_skipped {
            info!(video = %video.id, policy = %policy_name, "every stage is cached; nothing to do (use force to re-run)");
            for (i, name) in stage_names.iter().enumerate() {
                let st = state.stage_mut(name);
                st.status = StageStatus::Skipped;
                st.cached = cached[i];
            }
            state.finished = true;
            self.storage.checkpoint(job_id, &state).await?;
            if video.index_state == IndexState::Acquired || video.index_state == IndexState::Failed
            {
                self.storage
                    .set_index_state(video.id, IndexState::Coarse)
                    .await?;
            }
            return Ok(JobReport {
                job_id,
                video_id: video.id,
                ok: true,
                stages: state.stages.clone(),
                elapsed_secs: started.elapsed().as_secs_f64(),
                acquire_secs,
                index_state: if video.index_state == IndexState::Fine {
                    IndexState::Fine
                } else {
                    IndexState::Coarse
                },
                skipped: true,
                budget: BudgetReport::default(),
                stopped_at: None,
            });
        }
        for (i, name) in stage_names.iter().enumerate() {
            let st = state.stage_mut(name);
            st.cached = cached[i];
            if plan[i] == Plan::Skip {
                st.status = StageStatus::Skipped;
            }
        }
        info!(
            run = ?stage_names.iter().enumerate().filter(|(i, _)| plan[*i] == Plan::Run).map(|(_, s)| s.as_str()).collect::<Vec<_>>(),
            replay = ?stage_names.iter().enumerate().filter(|(i, _)| plan[*i] == Plan::Replay).map(|(_, s)| s.as_str()).collect::<Vec<_>>(),
            skip = ?stage_names.iter().enumerate().filter(|(i, _)| plan[*i] == Plan::Skip).map(|(_, s)| s.as_str()).collect::<Vec<_>>(),
            "stage plan"
        );
        self.storage.checkpoint(job_id, &state).await?;

        // ---- wire the DAG -----------------------------------------------
        let capacity = (self.config.media.worker.max_in_flight_frames / 2).clamp(2, 16);
        let mut senders: Vec<Option<mpsc::Sender<Item>>> = Vec::with_capacity(n);
        let mut receivers: Vec<Option<mpsc::Receiver<Item>>> = Vec::with_capacity(n);
        for p in &plan {
            if *p == Plan::Skip {
                senders.push(None);
                receivers.push(None);
            } else {
                let (tx, rx) = mpsc::channel::<Item>(capacity);
                senders.push(Some(tx));
                receivers.push(Some(rx));
            }
        }
        // Each consumer receives only the kinds it declares (plus the media
        // item), so a root that emits frames, audio and ticks reaches each
        // consumer with its own kinds. Every shipped batch producer emits one
        // kind, so batch DAGs deliver exactly what they did before.
        let accepted: Vec<Vec<ItemKind>> = operators
            .iter()
            .map(|op| {
                let mut kinds: Vec<ItemKind> = op.inputs().to_vec();
                kinds.extend(op.optional_inputs().iter().copied());
                if !kinds.contains(&ItemKind::Media) {
                    kinds.push(ItemKind::Media);
                }
                kinds
            })
            .collect();
        let emitters: Vec<Emitter> = consumers
            .iter()
            .map(|c| {
                Emitter::with_kinds(
                    c.iter()
                        .filter_map(|j| senders[*j].clone().map(|s| (s, accepted[*j].clone())))
                        .collect(),
                )
            })
            .collect();
        // Seed the running roots with the media item (replayed stages need
        // it too, for the video id).
        let root_senders: Vec<mpsc::Sender<Item>> = roots
            .iter()
            .filter(|r| plan[**r] != Plan::Skip)
            .filter_map(|r| senders[*r].clone())
            .collect();
        // Non-root replayed stages also get the media item so they know the
        // video; they ignore everything else.
        let replay_senders: Vec<mpsc::Sender<Item>> = (0..n)
            .filter(|i| plan[*i] == Plan::Replay && !roots.contains(i))
            .filter_map(|i| senders[i].clone())
            .collect();
        drop(senders);
        for tx in root_senders.iter().chain(replay_senders.iter()) {
            let _ = tx.send(Item::Media(media.clone())).await;
        }
        drop(root_senders);
        drop(replay_senders);

        let job_cancel = cancel.child_token();
        let mut handles = Vec::with_capacity(n);
        for (i, op) in operators.into_iter().enumerate() {
            if plan[i] == Plan::Skip {
                continue;
            }
            let stage = op.id().to_string();
            let mut ctx = make_ctx(i, op.as_ref(), emitters[i].clone());
            ctx.cancel = job_cancel.clone();
            let rx = receivers[i]
                .take()
                .ok_or_else(|| Error::Other("receiver already taken".into()))?;
            let events = self.events.clone();
            let cancel_on_fail = job_cancel.clone();
            let replay = plan[i] == Plan::Replay;
            handles.push(tokio::spawn(async move {
                let outcome = run_stage(op, ctx, rx, &events, replay, live).await;
                if outcome.result.is_err() {
                    cancel_on_fail.cancel();
                }
                (stage, i, outcome)
            }));
        }
        drop(emitters);

        // Live jobs: checkpoint `last_t` per stage while the job runs, so
        // the live indexer can read the watermark from the job state.
        let checkpoint_done = CancellationToken::new();
        let checkpointer = live.then(|| {
            let storage = self.storage.clone();
            let base = state.clone();
            let names = stage_names.clone();
            let clocks = stage_clocks.clone();
            let done = checkpoint_done.clone();
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(LIVE_CHECKPOINT_INTERVAL);
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    tokio::select! {
                        _ = done.cancelled() => break,
                        _ = tick.tick() => {}
                    }
                    let mut snap = base.clone();
                    for (i, name) in names.iter().enumerate() {
                        let st = snap.stage_mut(name);
                        st.status = StageStatus::Running;
                        st.last_t = clocks[i].get();
                    }
                    snap.updated_at = Utc::now();
                    if let Err(e) = storage.checkpoint(snap.job_id, &snap).await {
                        debug!("live checkpoint failed: {e}");
                    }
                }
            })
        });

        let mut ok = true;
        let mut any_failures = false;
        let mut stopped = false;
        for h in handles {
            let (stage, i, outcome) = h
                .await
                .map_err(|e| Error::Other(format!("stage task panicked: {e}")))?;
            let failures = &stage_failures[i];
            let st = state.stage_mut(&stage);
            st.items_failed = failures.count();
            st.items_skipped = failures.skipped_count();
            st.failures = failures.ranges();
            st.replayed = plan[i] == Plan::Replay;
            st.last_t = stage_clocks[i].get();
            let StageOutcome {
                result,
                emitted,
                stored,
            } = outcome;
            match result {
                Ok(()) => {
                    let items = emitted;
                    st.items_done = items;
                    // A stage that stores without emitting (embeddings,
                    // extraction) did work when `stored` is non-zero.
                    if st.items_failed > 0 && items == 0 && stored == 0 {
                        // Nothing succeeded: the stage failed, even though
                        // it kept going past each failure.
                        ok = false;
                        st.status = StageStatus::Failed;
                        st.error = st
                            .failures
                            .first()
                            .map(|f| format!("every input failed; first: {}", f.error));
                    } else {
                        st.status = StageStatus::Complete;
                        if st.items_failed > 0 {
                            any_failures = true;
                        }
                        if !live && st.items_failed == 0 && st.items_skipped == 0 {
                            self.write_cache_marker(&keys[i], &stage, items, job_id);
                        }
                    }
                }
                Err(Error::Cancelled) if live => {
                    // A live job ends by cancellation: the stream was
                    // stopped. What the ticks committed stands.
                    stopped = true;
                    st.status = StageStatus::Complete;
                    st.items_done = emitted;
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
        checkpoint_done.cancel();
        if let Some(task) = checkpointer {
            let _ = task.await;
        }

        let index_state = if live {
            // The caller owns the state of a live video.
            video.index_state
        } else {
            let index_state = if !ok {
                IndexState::Failed
            } else if policy.fine.is_empty() {
                IndexState::Coarse
            } else {
                IndexState::Fine
            };
            self.storage.set_index_state(video.id, index_state).await?;
            index_state
        };
        state.finished = true;
        state.updated_at = Utc::now();
        self.storage.checkpoint(job_id, &state).await?;
        let stopped_at = if live && stopped {
            stage_clocks.iter().filter_map(|c| c.get()).max()
        } else {
            None
        };

        let elapsed = started.elapsed();
        let budget_report = BudgetReport {
            cost_usd: budget.spent_usd(),
            max_cost_usd: budget.max_cost_usd,
            max_wallclock_secs: budget.max_wallclock_secs,
            exhausted: budget.exhausted_reason().map(str::to_string),
            max_cost_usd_per_hour: budget.rolling.and_then(|r| r.max_cost_usd_per_hour),
        };
        let failed_total: u64 = state.stages.values().map(|s| s.items_failed).sum();
        let skipped_total: u64 = state.stages.values().map(|s| s.items_skipped).sum();
        let summary = format!(
            "{} stages, {} samples, {:.1}s{}{}{}",
            state.stages.len(),
            state
                .stages
                .get("sample")
                .map(|s| s.items_done)
                .unwrap_or(0),
            elapsed.as_secs_f64(),
            if budget_report.cost_usd > 0.0 {
                format!(", ${:.4}", budget_report.cost_usd)
            } else {
                String::new()
            },
            if failed_total > 0 {
                format!(", {failed_total} inputs failed")
            } else {
                String::new()
            },
            if skipped_total > 0 {
                format!(", {skipped_total} skipped (budget)")
            } else {
                String::new()
            }
        );
        if any_failures {
            warn!(video = %video.id, "{summary}");
        }
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
            budget: budget_report,
            stopped_at,
        })
    }

    fn cache_dir(&self) -> Option<std::path::PathBuf> {
        self.storage.cache_dir().map(|d| d.join("operators"))
    }

    fn cache_marker_exists(&self, key: &str) -> bool {
        self.cache_dir()
            .map(|d| d.join(format!("{key}.json")).is_file())
            .unwrap_or(false)
    }

    fn write_cache_marker(&self, key: &str, stage: &str, items: u64, job: JobId) {
        let Some(dir) = self.cache_dir() else {
            return;
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            debug!("cannot create operator cache dir: {e}");
            return;
        }
        let marker = serde_json::json!({
            "stage": stage,
            "items": items,
            "job": job.to_string(),
            "completed_at": Utc::now().to_rfc3339(),
        });
        let path = dir.join(format!("{key}.json"));
        if let Err(e) = std::fs::write(&path, marker.to_string()) {
            debug!("cannot write operator cache marker {}: {e}", path.display());
        }
    }
}

/// How often a live job writes its stages' `last_t` into the checkpoint.
const LIVE_CHECKPOINT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// A job after acquisition, ready to run.
struct Prepared {
    job_id: JobId,
    started: Instant,
    acquire_secs: f64,
    policy_name: String,
    policy: IndexPolicy,
    dag: Dag,
    media: MediaItem,
    opts: JobOptions,
    cancel: CancellationToken,
}

/// Cache key per `docs/05-indexing-pipeline.md`: content hash, operator id
/// and version, and the operator's parameters (provider, model, prompt
/// hash, thresholds), hashed to a file name.
pub fn cache_key(content_hash: &str, op: &dyn Operator, params: &serde_json::Value) -> String {
    let mut h = blake3::Hasher::new();
    h.update(content_hash.as_bytes());
    h.update(b"|");
    h.update(op.id().as_bytes());
    h.update(b"|");
    h.update(op.version().to_string().as_bytes());
    h.update(b"|");
    h.update(params.to_string().as_bytes());
    format!("{}-{}", op.id(), &h.finalize().to_hex()[..24])
}

/// What driving one operator came to: the error if it failed (or was
/// cancelled), and the counts up to that point either way.
struct StageOutcome {
    result: Result<()>,
    emitted: u64,
    stored: u64,
}

/// Drive one operator over its input channel. Returns items emitted and
/// rows stored. In replay mode the operator re-emits stored outputs once it
/// has seen the media item and ignores everything else. In a live job every
/// processed item moves the stage's clock (a tick to its head, so every
/// operator that received the tick records `last_t = head`), and ticks
/// move the budget's rolling window.
async fn run_stage(
    op: Box<dyn Operator>,
    ctx: OpContext,
    mut rx: mpsc::Receiver<Item>,
    events: &EventBus,
    replay: bool,
    live: bool,
) -> StageOutcome {
    let started = Instant::now();
    events.emit(Event::StageStarted {
        job: ctx.job,
        stage: ctx.stage.clone(),
    });
    let mut emitted = 0u64;
    let mut stored = 0u64;
    let result: Result<()> = async {
        if replay {
            // Wait for the media item so the operator knows the video.
            let mut media_seen = false;
            loop {
                let item = tokio::select! {
                    biased;
                    _ = ctx.cancel.cancelled() => return Err(Error::Cancelled),
                    item = rx.recv() => item,
                };
                let Some(item) = item else { break };
                if let Item::Media(_) = &item {
                    if !media_seen {
                        media_seen = true;
                        op.run(&ctx, OpInput { item }).await?;
                        match op.replay(&ctx).await? {
                            Some(n) => emitted += n,
                            None => {
                                return Err(Error::operator(
                                    &ctx.stage,
                                    "stage planned as replay but the operator cannot replay",
                                ))
                            }
                        }
                    }
                }
                // Other items (from producers that ran anyway) are ignored.
            }
            return Ok(());
        }
        loop {
            let item = tokio::select! {
                biased;
                _ = ctx.cancel.cancelled() => return Err(Error::Cancelled),
                item = rx.recv() => item,
            };
            let Some(item) = item else { break };
            let time = if live { item.time() } else { None };
            if let (true, Item::Tick { head }) = (live, &item) {
                ctx.budget.advance(*head);
            }
            let out = op.run(&ctx, OpInput { item }).await?;
            if let Some(t) = time {
                ctx.clock.advance(t);
            }
            emitted += out.emitted;
            stored += out.stored;
        }
        let out = op.finish(&ctx).await?;
        emitted += out.emitted;
        stored += out.stored;
        Ok(())
    }
    .await;
    // Close our end so upstream producers see the consumer gone.
    rx.close();
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let result = match result {
        Ok(()) => {
            debug!(stage = %ctx.stage, emitted, elapsed_ms, "stage finished");
            events.emit(Event::StageFinished {
                job: ctx.job,
                stage: ctx.stage.clone(),
                items: emitted,
                elapsed_ms,
            });
            Ok(())
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
    };
    StageOutcome {
        result,
        emitted,
        stored,
    }
}
