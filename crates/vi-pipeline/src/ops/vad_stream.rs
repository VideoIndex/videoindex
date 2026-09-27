//! `vad_stream`: voice activity detection over a live stream of audio
//! chunks (live C3). Where `vad` decodes a whole track and scores it in one
//! batched pass, this operator scores 16 kHz PCM as it arrives and emits a
//! `SpeechRange` per utterance with `vad`'s padding and merge rules, an
//! utterance never longer than [`MAX_UTTERANCE_SECS`], and a flush at most
//! [`FLUSH_AFTER_SILENCE_SECS`] after speech ends. `vad` is untouched; live
//! policies name `vad_stream` in its place.

use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use vi_core::config::{Config, VadConfig};
use vi_core::{cpu, Error, Result, Timestamp};
use vi_perceive::onnx::resolve_device;
use vi_perceive::vad::{self, SAMPLE_RATE, WINDOW};

use crate::operator::*;

/// Longest utterance emitted; longer speech is cut here. It is the span
/// length the index already uses for transcripts.
pub const MAX_UTTERANCE_SECS: f64 = 15.0;

/// An utterance is emitted no later than this long after its speech ended,
/// so a transcript never waits on the next sentence.
pub const FLUSH_AFTER_SILENCE_SECS: f64 = 3.0;

/// Seconds of a window, [`WINDOW`] samples at [`SAMPLE_RATE`].
const WIN_SECS: f64 = WINDOW as f64 / SAMPLE_RATE as f64;

/// Scores 16 kHz mono PCM into per-window speech probabilities as it
/// arrives. The shipped scorer is Silero through [`vi_perceive::vad::Vad`];
/// tests script one (a level detector, since the model is not a test
/// dependency).
pub trait SpeechScorer: Send {
    /// Feed samples; returns the probabilities of the windows that became
    /// complete, each [`WINDOW`] samples long, in order. Samples of a
    /// partial window are kept for the next call.
    fn push(&mut self, samples: &[i16]) -> Result<Vec<f32>>;
}

/// Builds a scorer for one job.
pub type ScorerFactory = Arc<dyn Fn(&Config) -> Result<Box<dyn SpeechScorer>> + Send + Sync>;

/// Silero VAD as a streaming scorer.
struct SileroScorer {
    vad: vad::Vad,
    seen: usize,
}

impl SpeechScorer for SileroScorer {
    fn push(&mut self, samples: &[i16]) -> Result<Vec<f32>> {
        self.vad
            .push(samples)
            .map_err(|e| Error::operator("vad_stream", e.to_string()))?;
        let probs = self.vad.probabilities();
        let new = probs[self.seen.min(probs.len())..].to_vec();
        self.seen = probs.len();
        Ok(new)
    }
}

fn silero_factory() -> ScorerFactory {
    Arc::new(|config: &Config| {
        let models = config.models.clone();
        let device = resolve_device(&models.device)
            .map_err(|e| Error::operator("vad_stream", e.to_string()))?;
        let vad = vad::Vad::load(&models.dir, device, models.vad.clone())
            .map_err(|e| Error::operator("vad_stream", e.to_string()))?;
        Ok(Box::new(SileroScorer { vad, seen: 0 }) as Box<dyn SpeechScorer>)
    })
}

/// Turns per-window probabilities into utterances as they complete, on the
/// stream's own timeline (seconds). Pure, so it is testable without audio.
#[derive(Debug)]
struct Segmenter {
    cfg: VadConfig,
    /// Start time of the next window.
    t: f64,
    in_speech: bool,
    /// The speech run being built: start, and the end of its last speech
    /// window.
    run: Option<(f64, f64)>,
    /// Utterances ready to emit, unpadded.
    ready: Vec<(f64, f64)>,
}

impl Segmenter {
    fn new(cfg: VadConfig, t: f64) -> Self {
        Self {
            cfg,
            t,
            in_speech: false,
            run: None,
            ready: Vec::new(),
        }
    }

    fn min_silence(&self) -> f64 {
        f64::from(self.cfg.min_silence_ms) / 1000.0
    }

    /// Score one window.
    fn push(&mut self, p: f32) {
        let t0 = self.t;
        let t1 = t0 + WIN_SECS;
        self.t = t1;
        match self.run {
            None => {
                if p >= self.cfg.threshold {
                    self.run = Some((t0, t1));
                    self.in_speech = true;
                }
            }
            Some((start, end)) if self.in_speech => {
                if p < self.cfg.neg_threshold {
                    self.in_speech = false;
                    self.run = Some((start, end));
                } else if t1 - start > MAX_UTTERANCE_SECS {
                    // This window would take the utterance over the cap:
                    // close it at the previous window and start the next
                    // one here, so the pieces stay contiguous.
                    self.ready.push((start, end));
                    self.run = Some((t0, t1));
                } else {
                    self.run = Some((start, t1));
                }
            }
            Some((start, end)) => {
                if p >= self.cfg.threshold {
                    self.in_speech = true;
                    if t0 - end < self.min_silence() {
                        // A short pause: the same utterance goes on.
                        self.run = Some((start, t1));
                    } else {
                        self.ready.push((start, end));
                        self.run = Some((t0, t1));
                    }
                } else if t1 - end >= FLUSH_AFTER_SILENCE_SECS {
                    self.ready.push((start, end));
                    self.run = None;
                }
            }
        }
    }

    /// Flush a run whose speech ended at least the flush wait before `now`
    /// (used on ticks, when stream time advances without audio).
    fn flush_stale(&mut self, now: f64) {
        if let Some((start, end)) = self.run {
            if !self.in_speech && now - end >= FLUSH_AFTER_SILENCE_SECS {
                self.ready.push((start, end));
                self.run = None;
            }
        }
    }

    /// Flush everything (the stream ended).
    fn flush_all(&mut self) {
        if let Some(run) = self.run.take() {
            self.ready.push(run);
        }
        self.in_speech = false;
    }

    /// Utterances ready to emit, dropping those shorter than `min_speech`.
    fn take_ready(&mut self) -> Vec<(f64, f64)> {
        let min_speech = f64::from(self.cfg.min_speech_ms) / 1000.0;
        std::mem::take(&mut self.ready)
            .into_iter()
            .filter(|(a, b)| b - a >= min_speech.max(WIN_SECS))
            .collect()
    }
}

/// Per-job state.
struct State {
    scorer: Option<Box<dyn SpeechScorer>>,
    segmenter: Segmenter,
    /// PCM kept for emission, from `pcm_start` on.
    pcm: VecDeque<i16>,
    pcm_start: f64,
    /// Whether any audio arrived yet (the clock is anchored on the first
    /// chunk).
    anchored: bool,
    /// End of the last emitted utterance, to keep them disjoint.
    last_end: f64,
    emitted: u64,
    chunks: u64,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("pcm", &self.pcm.len())
            .field("emitted", &self.emitted)
            .field("chunks", &self.chunks)
            .finish()
    }
}

impl State {
    fn pcm_end(&self) -> f64 {
        self.pcm_start + self.pcm.len() as f64 / f64::from(SAMPLE_RATE)
    }

    /// Drop PCM older than what any utterance still open could need.
    fn trim(&mut self) {
        let pad = f64::from(self.segmenter.cfg.pad_ms) / 1000.0;
        let keep = MAX_UTTERANCE_SECS + FLUSH_AFTER_SILENCE_SECS + 2.0 * pad + 1.0;
        let floor = match self.segmenter.run {
            Some((start, _)) => (start - pad).min(self.pcm_end() - keep),
            None => self.pcm_end() - keep,
        };
        if floor > self.pcm_start {
            let n = ((floor - self.pcm_start) * f64::from(SAMPLE_RATE)) as usize;
            let n = n.min(self.pcm.len());
            self.pcm.drain(..n);
            self.pcm_start += n as f64 / f64::from(SAMPLE_RATE);
        }
    }
}

/// Streaming voice activity detector.
pub struct VadStream {
    factory: ScorerFactory,
    state: Mutex<Option<State>>,
}

impl std::fmt::Debug for VadStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VadStream").finish()
    }
}

impl Default for VadStream {
    fn default() -> Self {
        Self::new()
    }
}

impl VadStream {
    /// The shipped operator: Silero VAD from `models.dir`.
    pub fn new() -> Self {
        Self::with_scorer(silero_factory())
    }

    /// An operator over any scorer (tests script one; a module may bring a
    /// different model).
    pub fn with_scorer(factory: ScorerFactory) -> Self {
        Self {
            factory,
            state: Mutex::new(None),
        }
    }

    /// Emit every utterance the segmenter has ready.
    async fn emit_ready(&self, ctx: &OpContext, st: &mut State) -> Result<u64> {
        let pad = f64::from(st.segmenter.cfg.pad_ms) / 1000.0;
        let mut n = 0;
        for (a, b) in st.segmenter.take_ready() {
            let t0 = (a - pad).max(st.pcm_start).max(st.last_end);
            let t1 = (b + pad).min(st.pcm_end()).min(t0 + MAX_UTTERANCE_SECS);
            if t1 <= t0 {
                continue;
            }
            let i0 = ((t0 - st.pcm_start) * f64::from(SAMPLE_RATE)).round() as usize;
            let i1 = ((t1 - st.pcm_start) * f64::from(SAMPLE_RATE)).round() as usize;
            let i1 = i1.min(st.pcm.len());
            if i1 <= i0 {
                continue;
            }
            let samples: Vec<i16> = st.pcm.range(i0..i1).copied().collect();
            st.last_end = t1;
            let item = SpeechItem {
                t0: Timestamp::from_secs_f64(t0, SAMPLE_RATE),
                t1: Timestamp::from_secs_f64(t1, SAMPLE_RATE),
                samples: Arc::from(samples),
                sample_rate: SAMPLE_RATE,
                index: st.emitted,
            };
            st.emitted += 1;
            n += 1;
            ctx.emit(Item::SpeechRange(Arc::new(item))).await?;
        }
        Ok(n)
    }
}

#[async_trait]
impl Operator for VadStream {
    fn id(&self) -> &'static str {
        "vad_stream"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[ItemKind::Media, ItemKind::AudioChunk, ItemKind::Tick]
    }

    fn optional_inputs(&self) -> &[InputKind] {
        &[ItemKind::Tick]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::SpeechRange]
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        CostEstimate {
            cpu_secs: if input.has_audio {
                input.duration_secs * 0.01
            } else {
                0.0
            },
            usd: 0.0,
            provider_calls: 0,
        }
    }

    fn cache_params(&self, ctx: &OpContext) -> serde_json::Value {
        serde_json::json!({
            "vad": ctx.config.models.vad,
            "max_utterance_secs": MAX_UTTERANCE_SECS,
            "flush_after_silence_secs": FLUSH_AFTER_SILENCE_SECS,
        })
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        match input.item {
            Item::Media(media) => {
                if media.audio_track().is_none() {
                    tracing::info!(video = %media.video.id, "no audio track; vad_stream idle");
                }
                let scorer = (self.factory)(&ctx.config)?;
                *self.state.lock().await = Some(State {
                    scorer: Some(scorer),
                    segmenter: Segmenter::new(ctx.config.models.vad.clone(), 0.0),
                    pcm: VecDeque::new(),
                    pcm_start: 0.0,
                    anchored: false,
                    last_end: f64::NEG_INFINITY,
                    emitted: 0,
                    chunks: 0,
                });
                Ok(OpOutput::default())
            }
            Item::AudioChunk(chunk) => {
                if chunk.sample_rate != SAMPLE_RATE {
                    return Err(ctx.err(format!(
                        "vad_stream needs {SAMPLE_RATE} Hz audio, got {} Hz",
                        chunk.sample_rate
                    )));
                }
                let mut guard = self.state.lock().await;
                let st = guard
                    .as_mut()
                    .ok_or_else(|| ctx.err("audio arrived before the media item"))?;
                st.chunks += 1;
                let t0 = chunk.t0.as_secs_f64();
                // Anchor the clock on the first chunk; re-anchor after a
                // hole (a `Gap` in the recording): what was open is flushed
                // and the buffer restarts.
                let jump = !st.anchored || (t0 - st.pcm_end()).abs() > WIN_SECS;
                let mut emitted = 0;
                if jump {
                    if st.anchored {
                        st.segmenter.flush_all();
                        emitted += self.emit_ready(ctx, st).await?;
                    }
                    st.pcm.clear();
                    st.pcm_start = t0;
                    st.segmenter = Segmenter::new(st.segmenter.cfg.clone(), t0);
                    st.anchored = true;
                }
                st.pcm.extend(chunk.samples.iter().copied());
                // Score off the runtime: the model is small but sequential.
                let mut scorer = st.scorer.take().ok_or_else(|| ctx.err("scorer missing"))?;
                let samples = chunk.samples.clone();
                let (probs, scorer_back) = cpu::run(move || {
                    let r = scorer.push(&samples);
                    (r, scorer)
                })
                .await?;
                st.scorer = Some(scorer_back);
                for p in probs? {
                    st.segmenter.push(p);
                }
                emitted += self.emit_ready(ctx, st).await?;
                st.trim();
                if st.chunks % 30 == 0 {
                    ctx.progress(st.emitted);
                }
                Ok(OpOutput { emitted, stored: 0 })
            }
            Item::Tick { head } => {
                let mut guard = self.state.lock().await;
                let Some(st) = guard.as_mut() else {
                    return Ok(OpOutput::default());
                };
                st.segmenter.flush_stale(head.as_secs_f64());
                let emitted = self.emit_ready(ctx, st).await?;
                Ok(OpOutput { emitted, stored: 0 })
            }
            _ => Err(ctx.err("expected media, an audio chunk or a tick")),
        }
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let mut guard = self.state.lock().await;
        let Some(st) = guard.as_mut() else {
            return Ok(OpOutput::default());
        };
        st.segmenter.flush_all();
        let emitted = self.emit_ready(ctx, st).await?;
        tracing::info!(
            utterances = st.emitted,
            chunks = st.chunks,
            "vad_stream finished"
        );
        ctx.progress(ctx.expected_items.unwrap_or(st.emitted));
        Ok(OpOutput { emitted, stored: 0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> VadConfig {
        VadConfig {
            threshold: 0.5,
            neg_threshold: 0.35,
            min_speech_ms: 250,
            min_silence_ms: 1000,
            pad_ms: 200,
            max_segment_secs: 120.0,
        }
    }

    fn windows(secs: f64) -> usize {
        (secs / WIN_SECS).round() as usize
    }

    fn feed(seg: &mut Segmenter, p: f32, secs: f64) {
        for _ in 0..windows(secs) {
            seg.push(p);
        }
    }

    #[test]
    fn utterances_flush_three_seconds_after_speech_and_merge_short_pauses() {
        let mut seg = Segmenter::new(cfg(), 0.0);
        feed(&mut seg, 0.0, 2.0);
        feed(&mut seg, 0.9, 4.0); // speech 2..6
        feed(&mut seg, 0.0, 0.5); // short pause: merged
        feed(&mut seg, 0.9, 2.0); // speech 6.5..8.5
        assert!(seg.take_ready().is_empty(), "still open");
        feed(&mut seg, 0.0, 2.9);
        assert!(seg.take_ready().is_empty(), "under the flush wait");
        feed(&mut seg, 0.0, 0.2);
        let out = seg.take_ready();
        assert_eq!(out.len(), 1, "{out:?}");
        // Window counts round, so boundaries land within two windows.
        assert!((out[0].0 - 2.0).abs() < 2.0 * WIN_SECS + 1e-9, "{out:?}");
        assert!((out[0].1 - 8.5).abs() < 2.0 * WIN_SECS + 1e-9, "{out:?}");
        // A pause over min_silence but under the flush wait: the next
        // speech starts a new utterance and releases the old one.
        feed(&mut seg, 0.9, 1.0);
        feed(&mut seg, 0.0, 1.5);
        feed(&mut seg, 0.9, 1.0);
        let out = seg.take_ready();
        assert_eq!(out.len(), 1, "{out:?}");
        assert!((out[0].1 - out[0].0 - 1.0).abs() < 2.0 * WIN_SECS);
        // Blips shorter than min_speech are dropped once flushed.
        let mut seg = Segmenter::new(cfg(), 0.0);
        feed(&mut seg, 0.9, 0.1);
        feed(&mut seg, 0.0, 4.0);
        assert!(seg.take_ready().is_empty());
    }

    #[test]
    fn long_speech_is_cut_at_fifteen_seconds() {
        let mut seg = Segmenter::new(cfg(), 100.0);
        feed(&mut seg, 0.95, 40.0);
        seg.flush_all();
        let out = seg.take_ready();
        assert_eq!(out.len(), 3, "{out:?}");
        for (a, b) in &out {
            assert!(b - a <= MAX_UTTERANCE_SECS + 1e-9, "{a}..{b}");
        }
        assert!((out[0].0 - 100.0).abs() < 1e-9);
        for w in out.windows(2) {
            assert!((w[0].1 - w[1].0).abs() < 1e-9, "contiguous cuts");
        }
        assert!((out[2].1 - 140.0).abs() < WIN_SECS + 1e-9);
    }

    #[test]
    fn stale_runs_flush_on_a_tick() {
        let mut seg = Segmenter::new(cfg(), 0.0);
        feed(&mut seg, 0.9, 2.0);
        feed(&mut seg, 0.0, 1.0);
        seg.flush_stale(3.5);
        assert!(
            seg.take_ready().is_empty(),
            "only 1.5 s of silence by the tick"
        );
        seg.flush_stale(5.1);
        assert_eq!(seg.take_ready().len(), 1);
    }
}
