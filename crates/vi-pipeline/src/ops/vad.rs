//! `vad`: decode the audio track to 16 kHz mono, run Silero VAD, and emit
//! speech segments with their PCM. Only speech reaches ASR; an hour of
//! lecture with 40 minutes of speech costs 40 minutes of ASR.

use std::sync::Arc;

use async_trait::async_trait;
use vi_core::{cpu, Result, Timestamp};
use vi_media::AudioDecodeRequest;
use vi_perceive::onnx::resolve_device;
use vi_perceive::vad::{self, SAMPLE_RATE};

use crate::operator::*;

/// Independent audio parts scored in lockstep (one ONNX Runtime call scores
/// this many windows).
const VAD_BATCH: usize = 32;

/// Voice activity detector operator.
#[derive(Debug, Default)]
pub struct Vad;

impl Vad {
    /// New operator.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Operator for Vad {
    fn id(&self) -> &'static str {
        "vad"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[ItemKind::Media]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::SpeechRange]
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        // Audio decode plus a tiny model: about 1% of real time.
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

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let Item::Media(media) = input.item else {
            return Err(ctx.err("expected the media item"));
        };
        let Some(track) = media.audio_track().cloned() else {
            tracing::info!(video = %media.video.id, "no audio track; skipping VAD");
            ctx.progress(0);
            return Ok(OpOutput::default());
        };
        let models = ctx.config.models.clone();
        let device = resolve_device(&models.device).map_err(|e| ctx.err(e))?;
        let mut detector =
            cpu::run(move || vad::Vad::load(&models.dir, device, models.vad.clone()))
                .await?
                .map_err(|e| ctx.err(e))?;

        // Decode the whole track to memory (16 kHz mono i16: 115 MB per
        // hour), then score it in one batched pass off the runtime.
        let req = AudioDecodeRequest {
            stream_index: Some(track.stream_index),
            sample_rate: SAMPLE_RATE,
            chunk_secs: 30.0,
            overlap_secs: 0.0,
            ..AudioDecodeRequest::new(&media.acquired.path)
        };
        let mut stream = vi_media::decode_audio(&ctx.worker, req).await?;
        let total_secs = media.probe.duration.as_secs_f64();
        let mut pcm: Vec<i16> =
            Vec::with_capacity((total_secs * f64::from(SAMPLE_RATE)) as usize + 1024);
        let mut start: Option<Timestamp> = None;
        let mut chunks = 0u64;
        let expected = ctx.expected_items.unwrap_or(0) as f64;
        loop {
            ctx.check_cancelled()?;
            let Some(chunk) = stream.next().await? else {
                break;
            };
            if start.is_none() {
                start = Some(chunk.t0);
            }
            chunks += 1;
            pcm.extend_from_slice(&chunk.samples);
            if chunks % 10 == 0 && total_secs > 0.0 {
                let done = pcm.len() as f64 / f64::from(SAMPLE_RATE);
                // Decoding is the first half of this stage's progress.
                ctx.progress(((done / total_secs).min(1.0) * 0.5 * expected) as u64);
            }
        }
        let start = start.unwrap_or(Timestamp::ZERO);
        let pcm = Arc::<[i16]>::from(pcm);
        let scoring = pcm.clone();
        let batch = VAD_BATCH;
        let (segments, detector_back) = cpu::run(move || {
            let r = detector.detect(&scoring, batch);
            (r, detector)
        })
        .await?;
        drop(detector_back);
        let segments = segments.map_err(|e| ctx.err(e))?;
        let speech = vad::speech_secs(&segments);
        tracing::info!(
            video = %media.video.id,
            segments = segments.len(),
            speech_secs = format!("{speech:.1}"),
            audio_secs = format!("{:.1}", pcm.len() as f64 / f64::from(SAMPLE_RATE)),
            "speech detected"
        );

        let mut emitted = 0u64;
        for (i, seg) in segments.iter().enumerate() {
            ctx.check_cancelled()?;
            let a = ((seg.t0 * f64::from(SAMPLE_RATE)) as usize).min(pcm.len());
            let b = ((seg.t1 * f64::from(SAMPLE_RATE)) as usize).clamp(a, pcm.len());
            if b <= a {
                continue;
            }
            let item = SpeechItem {
                t0: start.add(Timestamp::from_secs_f64(seg.t0, SAMPLE_RATE)),
                t1: start.add(Timestamp::from_secs_f64(seg.t1, SAMPLE_RATE)),
                samples: Arc::from(&pcm[a..b]),
                sample_rate: SAMPLE_RATE,
                index: i as u64,
            };
            ctx.emit(Item::SpeechRange(Arc::new(item))).await?;
            emitted += 1;
        }
        ctx.progress(ctx.expected_items.unwrap_or(emitted));
        Ok(OpOutput { emitted, stored: 0 })
    }
}
