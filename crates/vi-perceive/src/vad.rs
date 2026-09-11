//! Voice activity detection with Silero VAD (ONNX). Feed 16 kHz mono PCM
//! in any chunking; get back speech segments merged across short pauses,
//! padded, and split so none exceeds a maximum length. Only these segments
//! go to ASR.

use std::path::Path;

use ort::inputs;
use ort::value::Tensor;
use vi_core::config::VadConfig;

use crate::onnx::{extract_f32, Device, OnnxSession};
use crate::PerceiveError;

/// Sample rate the model expects.
pub const SAMPLE_RATE: u32 = 16_000;
/// Samples per model window (32 ms at 16 kHz).
pub const WINDOW: usize = 512;
/// Context samples the v5 model carries from the previous window.
const CONTEXT: usize = 64;
/// Model file name under `<models.dir>/silero-vad/`.
pub const MODEL_FILE: &str = "silero-vad/silero_vad.onnx";
/// Where to get the model.
pub const MODEL_HINT: &str =
    "download onnx/model.onnx from huggingface.co/onnx-community/silero-vad to <models.dir>/silero-vad/silero_vad.onnx";

/// A run of speech, in seconds from the start of the audio fed in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeechSegment {
    /// Start.
    pub t0: f64,
    /// End.
    pub t1: f64,
}

impl SpeechSegment {
    /// Length in seconds.
    pub fn duration(&self) -> f64 {
        self.t1 - self.t0
    }
}

/// Streaming detector: the model, its recurrent state, and the segmenter.
pub struct Vad {
    session: OnnxSession,
    state: Vec<f32>,
    context: Vec<f32>,
    pending: Vec<f32>,
    /// Absolute sample index of `pending[0]`.
    pending_start: u64,
    probs: Vec<f32>,
    cfg: VadConfig,
}

impl std::fmt::Debug for Vad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vad")
            .field("windows", &self.probs.len())
            .field("cfg", &self.cfg)
            .finish()
    }
}

impl Vad {
    /// Load the model from `<models_dir>/silero-vad/silero_vad.onnx`.
    pub fn load(models_dir: &Path, device: Device, cfg: VadConfig) -> Result<Self, PerceiveError> {
        // The model is tiny and sequential; the CPU is the right place and
        // one thread avoids contention with the decoder.
        let session = OnnxSession::load(&models_dir.join(MODEL_FILE), device, 1, MODEL_HINT)?;
        Ok(Self {
            session,
            state: vec![0.0; 2 * 128],
            context: vec![0.0; CONTEXT],
            pending: Vec::with_capacity(WINDOW * 4),
            pending_start: 0,
            probs: Vec::new(),
            cfg,
        })
    }

    /// Feed samples (16 kHz mono). Windows are evaluated as they complete.
    pub fn push(&mut self, samples: &[i16]) -> Result<(), PerceiveError> {
        self.pending
            .extend(samples.iter().map(|s| f32::from(*s) / 32768.0));
        while self.pending.len() >= WINDOW {
            let window: Vec<f32> = self.pending.drain(..WINDOW).collect();
            self.pending_start += WINDOW as u64;
            let p = self.infer(&window)?;
            self.probs.push(p);
        }
        Ok(())
    }

    /// Speech probability of one window.
    fn infer(&mut self, window: &[f32]) -> Result<f32, PerceiveError> {
        let mut input = Vec::with_capacity(CONTEXT + WINDOW);
        input.extend_from_slice(&self.context);
        input.extend_from_slice(window);
        self.context.copy_from_slice(&window[WINDOW - CONTEXT..]);
        let input_t = Tensor::from_array(([1usize, CONTEXT + WINDOW], input))
            .map_err(|e| PerceiveError::Onnx(e.to_string()))?;
        let state_t = Tensor::from_array(([2usize, 1, 128], self.state.clone()))
            .map_err(|e| PerceiveError::Onnx(e.to_string()))?;
        let sr_t = Tensor::from_array(((), vec![i64::from(SAMPLE_RATE)]))
            .map_err(|e| PerceiveError::Onnx(e.to_string()))?;
        let (prob, state) = self.session.run(
            inputs!["input" => input_t, "state" => state_t, "sr" => sr_t],
            |out| {
                let (_, p) = extract_f32(out, "output")?;
                let (_, s) = extract_f32(out, "stateN")?;
                Ok((p.first().copied().unwrap_or(0.0), s))
            },
        )?;
        if state.len() == self.state.len() {
            self.state = state;
        }
        Ok(prob)
    }

    /// Score a whole recording at once, running `batch` independent parts
    /// of it in lockstep through the model. The recurrent state restarts at
    /// each part boundary, which costs at most one window of context per
    /// part; the win is `batch` times fewer ONNX Runtime calls (each call
    /// costs about as much as the arithmetic for a single window). Returns
    /// per-window probabilities in time order.
    pub fn score_batched(
        &mut self,
        samples: &[i16],
        batch: usize,
    ) -> Result<Vec<f32>, PerceiveError> {
        let batch = batch.max(1);
        let n_windows = samples.len().div_ceil(WINDOW);
        if n_windows == 0 {
            return Ok(Vec::new());
        }
        let per_part = n_windows.div_ceil(batch);
        let parts = n_windows.div_ceil(per_part);
        let mut audio: Vec<f32> = samples.iter().map(|s| f32::from(*s) / 32768.0).collect();
        audio.resize(parts * per_part * WINDOW, 0.0);
        let mut probs = vec![0.0f32; parts * per_part];
        let mut state = vec![0.0f32; 2 * parts * 128];
        let mut context = vec![0.0f32; parts * CONTEXT];
        for j in 0..per_part {
            let mut input = Vec::with_capacity(parts * (CONTEXT + WINDOW));
            for b in 0..parts {
                let off = (b * per_part + j) * WINDOW;
                let window = &audio[off..off + WINDOW];
                input.extend_from_slice(&context[b * CONTEXT..(b + 1) * CONTEXT]);
                input.extend_from_slice(window);
                context[b * CONTEXT..(b + 1) * CONTEXT]
                    .copy_from_slice(&window[WINDOW - CONTEXT..]);
            }
            let input_t = Tensor::from_array(([parts, CONTEXT + WINDOW], input))
                .map_err(|e| PerceiveError::Onnx(e.to_string()))?;
            let state_t = Tensor::from_array(([2usize, parts, 128], std::mem::take(&mut state)))
                .map_err(|e| PerceiveError::Onnx(e.to_string()))?;
            let sr_t = Tensor::from_array(((), vec![i64::from(SAMPLE_RATE)]))
                .map_err(|e| PerceiveError::Onnx(e.to_string()))?;
            let (p, st) = self.session.run(
                inputs!["input" => input_t, "state" => state_t, "sr" => sr_t],
                |out| {
                    let (_, p) = extract_f32(out, "output")?;
                    let (_, st) = extract_f32(out, "stateN")?;
                    Ok((p, st))
                },
            )?;
            if st.len() != 2 * parts * 128 || p.len() != parts {
                return Err(PerceiveError::Onnx(format!(
                    "unexpected VAD output sizes: probs {} state {}",
                    p.len(),
                    st.len()
                )));
            }
            state = st;
            for b in 0..parts {
                probs[b * per_part + j] = p[b];
            }
        }
        probs.truncate(n_windows);
        Ok(probs)
    }

    /// Segments for a whole recording: [`Self::score_batched`] then
    /// [`segment`].
    pub fn detect(
        &mut self,
        samples: &[i16],
        batch: usize,
    ) -> Result<Vec<SpeechSegment>, PerceiveError> {
        let probs = self.score_batched(samples, batch)?;
        Ok(segment(&probs, &self.cfg))
    }

    /// Number of windows scored so far.
    pub fn windows(&self) -> usize {
        self.probs.len()
    }

    /// Per-window speech probabilities so far.
    pub fn probabilities(&self) -> &[f32] {
        &self.probs
    }

    /// Finish: score the partial last window (zero-padded) and return the
    /// segments over everything fed so far.
    pub fn finish(mut self) -> Result<Vec<SpeechSegment>, PerceiveError> {
        if !self.pending.is_empty() {
            let mut window = std::mem::take(&mut self.pending);
            window.resize(WINDOW, 0.0);
            let p = self.infer(&window)?;
            self.probs.push(p);
        }
        Ok(segment(&self.probs, &self.cfg))
    }
}

/// Turn per-window probabilities into segments. Pure, so it is testable
/// without the model. Window `i` covers `[i * 32 ms, (i + 1) * 32 ms)`.
pub fn segment(probs: &[f32], cfg: &VadConfig) -> Vec<SpeechSegment> {
    let win_secs = WINDOW as f64 / f64::from(SAMPLE_RATE);
    let total = probs.len() as f64 * win_secs;
    // 1. Hysteresis thresholding into raw runs.
    let mut raw: Vec<(usize, usize)> = Vec::new(); // [start, end) windows
    let mut in_speech = false;
    let mut start = 0usize;
    for (i, p) in probs.iter().enumerate() {
        if !in_speech && *p >= cfg.threshold {
            in_speech = true;
            start = i;
        } else if in_speech && *p < cfg.neg_threshold {
            in_speech = false;
            raw.push((start, i));
        }
    }
    if in_speech {
        raw.push((start, probs.len()));
    }
    // 2. Merge runs separated by less than min_silence.
    let min_silence_w = (f64::from(cfg.min_silence_ms) / 1000.0 / win_secs).round() as usize;
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in raw {
        match merged.last_mut() {
            Some(last) if s.saturating_sub(last.1) < min_silence_w => last.1 = e,
            _ => merged.push((s, e)),
        }
    }
    // 3. Drop runs shorter than min_speech.
    let min_speech_w = (f64::from(cfg.min_speech_ms) / 1000.0 / win_secs).round() as usize;
    merged.retain(|(s, e)| e - s >= min_speech_w.max(1));
    // 4. Split runs longer than max_segment_secs at the lowest-probability
    //    window of their middle part (so pieces stay meaningful).
    let max_w = ((cfg.max_segment_secs / win_secs).floor() as usize).max(2 * min_speech_w.max(1));
    let mut pieces: Vec<(usize, usize)> = Vec::new();
    let mut stack: Vec<(usize, usize)> = merged.into_iter().rev().collect();
    while let Some((s, e)) = stack.pop() {
        if e - s <= max_w {
            pieces.push((s, e));
            continue;
        }
        // Search for the quietest window between 1/3 and the max length.
        let lo = s + (e - s).min(max_w) / 3;
        let hi = (s + max_w).min(e - 1);
        let cut = (lo..hi)
            .min_by(|a, b| {
                probs[*a]
                    .partial_cmp(&probs[*b])
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(hi);
        pieces.push((s, cut));
        stack.push((cut, e));
    }
    // 5. Pad and convert to seconds, clamping to the audio.
    let pad = f64::from(cfg.pad_ms) / 1000.0;
    let mut out: Vec<SpeechSegment> = pieces
        .into_iter()
        .map(|(s, e)| SpeechSegment {
            t0: (s as f64 * win_secs - pad).max(0.0),
            t1: (e as f64 * win_secs + pad).min(total),
        })
        .collect();
    // Padding may make neighbours overlap; keep them disjoint.
    for i in 1..out.len() {
        if out[i].t0 < out[i - 1].t1 {
            let mid = (out[i].t0 + out[i - 1].t1) / 2.0;
            out[i - 1].t1 = mid;
            out[i].t0 = mid;
        }
    }
    out.retain(|s| s.t1 > s.t0);
    out
}

/// Total seconds of speech in a set of segments.
pub fn speech_secs(segments: &[SpeechSegment]) -> f64 {
    segments.iter().map(SpeechSegment::duration).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> VadConfig {
        VadConfig {
            threshold: 0.5,
            neg_threshold: 0.35,
            min_speech_ms: 250,
            min_silence_ms: 700,
            pad_ms: 200,
            max_segment_secs: 29.0,
        }
    }

    // 31.25 windows per second (these tests pin their own VadConfig above).
    fn windows(secs: f64) -> usize {
        (secs * 31.25).round() as usize
    }

    #[test]
    fn segments_speech_with_hysteresis_merge_and_drop() {
        let mut p = vec![0.0f32; windows(2.0)]; // 2 s silence
        p.extend(vec![0.9; windows(5.0)]); // 5 s speech
        p.extend(vec![0.4; windows(0.3)]); // dip below on-threshold but above off: still speech
        p.extend(vec![0.9; windows(2.0)]);
        p.extend(vec![0.0; windows(0.4)]); // short pause: merged
        p.extend(vec![0.9; windows(3.0)]);
        p.extend(vec![0.0; windows(3.0)]); // long pause: split
        p.extend(vec![0.9; windows(0.1)]); // blip: dropped
        p.extend(vec![0.0; windows(1.0)]);
        p.extend(vec![0.9; windows(4.0)]);
        let segs = segment(&p, &cfg());
        assert_eq!(segs.len(), 2, "{segs:?}");
        assert!((segs[0].t0 - 1.8).abs() < 0.05, "{segs:?}");
        assert!((segs[0].t1 - 12.9).abs() < 0.05, "{segs:?}");
        // 12.9 + 3 s pause + 0.1 s blip + 1 s silence = 16.8, minus padding.
        assert!((segs[1].t0 - 16.6).abs() < 0.1, "{segs:?}");
        assert!((speech_secs(&segs) - (11.1 + 4.2)).abs() < 0.15);
    }

    #[test]
    fn long_speech_is_split_at_quiet_windows() {
        let mut p = vec![0.95f32; windows(70.0)];
        // Two quieter moments that should attract the cuts.
        p[windows(20.0)..windows(20.2)].fill(0.5);
        p[windows(45.0)..windows(45.2)].fill(0.5);
        let segs = segment(&p, &cfg());
        assert!(segs.len() >= 3, "{segs:?}");
        assert!(
            segs.iter().all(|s| s.duration() <= 29.0 + 0.4 + 1e-6),
            "{segs:?}"
        );
        assert!((segs[0].t1 - 20.0).abs() < 0.5, "{segs:?}");
        // Disjoint and ordered.
        for w in segs.windows(2) {
            assert!(w[0].t1 <= w[1].t0 + 1e-9);
        }
    }

    #[test]
    fn silence_gives_nothing() {
        assert!(segment(&vec![0.01; 1000], &cfg()).is_empty());
        assert!(segment(&[], &cfg()).is_empty());
    }

    #[test]
    fn model_scores_tone_as_non_speech_when_available() {
        let dir = std::path::PathBuf::from("/data/videoindex/models");
        if !dir.join(MODEL_FILE).is_file() {
            eprintln!("skipping: Silero model not present");
            return;
        }
        let mut vad = Vad::load(&dir, Device::Cpu, cfg()).unwrap();
        // 3 s of a 440 Hz tone plus 1 s of silence.
        let tone: Vec<i16> = (0..3 * SAMPLE_RATE as usize)
            .map(|i| {
                (8000.0 * (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 16000.0).sin()) as i16
            })
            .collect();
        vad.push(&tone).unwrap();
        vad.push(&vec![0i16; SAMPLE_RATE as usize]).unwrap();
        assert_eq!(vad.windows(), 4 * SAMPLE_RATE as usize / WINDOW);
        let max = vad.probabilities().iter().cloned().fold(0.0f32, f32::max);
        assert!(max < 0.5, "tone scored as speech: {max}");
        assert!(vad.finish().unwrap().is_empty());

        // Batched scoring agrees with streaming on silence/tone and is
        // shaped right.
        let mut vad = Vad::load(&dir, Device::Cpu, cfg()).unwrap();
        let mut all = tone.clone();
        all.extend(vec![0i16; SAMPLE_RATE as usize]);
        let probs = vad.score_batched(&all, 8).unwrap();
        assert_eq!(probs.len(), all.len().div_ceil(WINDOW));
        assert!(probs.iter().all(|p| *p < 0.5));
        assert!(vad.detect(&all, 8).unwrap().is_empty());
    }
}
