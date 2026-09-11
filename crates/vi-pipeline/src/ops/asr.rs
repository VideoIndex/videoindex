//! `asr`: transcribe speech segments through the provider bound to the
//! `asr` role and store `TranscriptSpan`s with word timings. Requests run
//! concurrently up to `models.asr.concurrency`; each request's cost lands
//! in its own `Provenance` row.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use vi_core::model::{Span, Track, TranscriptSpan};
use vi_core::{Error, Result, SpanId, Timestamp};
use vi_providers::{provenance_for, AsrOptions, AudioData};

use crate::operator::*;

/// ASR operator.
#[derive(Debug)]
pub struct Asr {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    track: Option<Track>,
    skip: bool,
    tasks: JoinSet<Result<(u64, u64)>>,
    emitted: u64,
    stored: u64,
    cost_usd: f64,
    media_secs: f64,
    requests: u64,
}

impl Default for Asr {
    fn default() -> Self {
        Self::new()
    }
}

impl Asr {
    /// New operator.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State::default()),
        }
    }

    /// Fold finished requests into the counters, waiting for room when the
    /// concurrency limit is reached.
    async fn reap(st: &mut State, max_in_flight: usize) -> Result<()> {
        while st.tasks.len() >= max_in_flight.max(1) {
            if let Some(r) = st.tasks.join_next().await {
                let (e, s) = r.map_err(|e| Error::Other(format!("asr task panicked: {e}")))??;
                st.emitted += e;
                st.stored += s;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Operator for Asr {
    fn id(&self) -> &'static str {
        "asr"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[ItemKind::Media, ItemKind::SpeechRange]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::TranscriptSpan]
    }

    fn required_roles(&self) -> &[&'static str] {
        &[vi_core::config::roles::ASR]
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        // Assume two thirds of a lecture is speech and 30 s per request.
        let speech = if input.has_audio {
            input.duration_secs * 0.66
        } else {
            0.0
        };
        CostEstimate {
            cpu_secs: speech * 0.002,
            usd: 0.0,
            provider_calls: (speech / 30.0).ceil() as u64,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        match input.item {
            Item::Media(media) => {
                let mut st = self.state.lock().await;
                st.track = media.audio_track().cloned();
                if st.track.is_none() {
                    tracing::info!("no audio track; ASR has nothing to do");
                    return Ok(OpOutput::default());
                }
                let human = media
                    .acquired
                    .subtitle_files
                    .iter()
                    .any(|f| media.acquired.subtitle_is_human(f));
                if ctx.config.models.asr.skip_if_human_subtitles && human {
                    tracing::info!("human-authored subtitles present; skipping ASR");
                    st.skip = true;
                    return Ok(OpOutput::default());
                }
                // Fail at the first item, not after an hour of VAD, when no
                // provider is bound.
                ctx.providers.asr()?;
                // Re-runs replace the earlier transcript of this operator.
                let track_id = st.track.as_ref().map(|t| t.id);
                if let Some(id) = track_id {
                    ctx.storage.delete_spans_by_operator(id, self.id()).await?;
                }
                Ok(OpOutput::default())
            }
            Item::SpeechRange(seg) => {
                let mut st = self.state.lock().await;
                if st.skip {
                    return Ok(OpOutput::default());
                }
                let Some(track) = st.track.clone() else {
                    return Err(ctx.err("speech arrived before the media item"));
                };
                let max = ctx.config.models.asr.concurrency;
                Self::reap(&mut st, max).await?;
                st.requests += 1;
                st.media_secs += seg.t1.sub(seg.t0).as_secs_f64();
                let job = TranscribeJob {
                    storage: ctx.storage.clone(),
                    providers: ctx.providers.clone(),
                    emitter: ctx.emitter.clone(),
                    track,
                    seg,
                    language: ctx.config.models.asr.language.clone(),
                    span_secs: ctx.config.models.asr.span_secs,
                    operator: self.id(),
                    version: self.version(),
                    stage: ctx.stage.clone(),
                };
                st.tasks.spawn(job.run());
                Ok(OpOutput::default())
            }
            _ => Err(ctx.err("expected media or a speech range")),
        }
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let mut st = self.state.lock().await;
        Self::reap(&mut st, 1).await?;
        if let Some(r) = st.tasks.join_next().await {
            let (e, s) = r.map_err(|e| Error::Other(format!("asr task panicked: {e}")))??;
            st.emitted += e;
            st.stored += s;
        }
        if st.requests > 0 {
            tracing::info!(
                requests = st.requests,
                spans = st.stored,
                speech_secs = format!("{:.1}", st.media_secs),
                cost_usd = st.cost_usd,
                "transcription finished"
            );
        }
        ctx.progress(ctx.expected_items.unwrap_or(st.emitted));
        Ok(OpOutput {
            emitted: st.emitted,
            stored: st.stored,
        })
    }
}

struct TranscribeJob {
    storage: Arc<dyn vi_index::Storage>,
    providers: Arc<vi_providers::ProviderRegistry>,
    emitter: Emitter,
    track: Track,
    seg: Arc<SpeechItem>,
    language: Option<String>,
    span_secs: f64,
    operator: &'static str,
    version: u32,
    stage: String,
}

impl TranscribeJob {
    async fn run(self) -> Result<(u64, u64)> {
        let asr = self.providers.asr()?;
        let audio = AudioData {
            samples: self.seg.samples.clone(),
            sample_rate: self.seg.sample_rate,
        };
        let opts = AsrOptions {
            language: self.language.clone(),
            prompt: None,
            word_timestamps: true,
            temperature: 0.0,
        };
        let resp = asr
            .transcribe(&audio, &opts)
            .await
            .map_err(|e| Error::operator(&self.stage, e.to_string()))?;
        let offset = self.seg.t0.as_secs_f64();
        let prov = provenance_for(
            self.operator,
            self.version,
            &resp.stats,
            None,
            serde_json::json!({
                "t0": offset,
                "t1": self.seg.t1.as_secs_f64(),
                "segment_index": self.seg.index,
                "language": resp.language,
                "segments": resp.segments.len(),
                "word_timestamps": true,
            }),
        );
        self.storage.put_provenance(&prov).await?;
        let spans = group_segments(
            &resp.segments,
            offset,
            self.span_secs,
            &self.track,
            resp.language.as_deref(),
            prov.id,
        );
        if spans.is_empty() {
            return Ok((0, 0));
        }
        let batch: Vec<Span> = spans.iter().cloned().map(Span::Transcript).collect();
        self.storage.put_spans(&batch).await?;
        let stored = spans.len() as u64;
        let mut emitted = 0;
        for sp in spans {
            self.emitter
                .emit(Item::TranscriptSpan(Arc::new(sp)))
                .await?;
            emitted += 1;
        }
        Ok((emitted, stored))
    }
}

/// Split segments longer than `span_secs` that carry word timings, cutting
/// after sentence-final punctuation when one falls in the second half of a
/// piece, else at the largest pause. Whisper's batched decoder returns one
/// segment per 30 s piece, which is too long for BM25 and for citations.
fn split_long_segments(
    segments: &[vi_providers::AsrSegment],
    span_secs: f64,
) -> Vec<vi_providers::AsrSegment> {
    let mut out = Vec::with_capacity(segments.len());
    for seg in segments {
        if seg.end - seg.start <= span_secs || seg.words.len() < 4 {
            out.push(seg.clone());
            continue;
        }
        let words = &seg.words;
        let mut piece_start = 0usize;
        while piece_start < words.len() {
            let t0 = words[piece_start].start;
            // Candidate end indices: words that keep the piece within span_secs.
            let mut last_fit = piece_start;
            while last_fit + 1 < words.len() && words[last_fit + 1].end - t0 <= span_secs {
                last_fit += 1;
            }
            let end_idx = if last_fit + 1 >= words.len() {
                words.len() - 1
            } else {
                let lo = piece_start + (last_fit - piece_start) / 2;
                // Prefer sentence-final punctuation in the second half.
                (lo..=last_fit)
                    .rev()
                    .find(|i| words[*i].word.trim_end().ends_with(['.', '?', '!']))
                    .unwrap_or_else(|| {
                        // Else the largest pause between consecutive words.
                        (lo..last_fit)
                            .max_by(|a, b| {
                                let ga = words[*a + 1].start - words[*a].end;
                                let gb = words[*b + 1].start - words[*b].end;
                                ga.partial_cmp(&gb).unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .unwrap_or(last_fit)
                    })
            };
            let piece = &words[piece_start..=end_idx];
            out.push(vi_providers::AsrSegment {
                start: piece[0].start,
                end: piece[piece.len() - 1].end,
                text: piece
                    .iter()
                    .map(|w| w.word.as_str())
                    .collect::<String>()
                    .trim()
                    .to_string(),
                words: piece.to_vec(),
                confidence: seg.confidence,
                no_speech_prob: seg.no_speech_prob,
            });
            piece_start = end_idx + 1;
        }
    }
    out
}

/// Group short ASR segments into spans of about `span_secs`, starting a new
/// span at gaps over 1.5 s. Word timings are kept as absolute seconds.
fn group_segments(
    segments: &[vi_providers::AsrSegment],
    offset: f64,
    span_secs: f64,
    track: &Track,
    language: Option<&str>,
    prov: vi_core::ProvenanceId,
) -> Vec<TranscriptSpan> {
    const GAP: f64 = 1.5;
    let den = 1000u32;
    let segments = split_long_segments(segments, span_secs);
    let mut out = Vec::new();
    let mut cur: Vec<&vi_providers::AsrSegment> = Vec::new();
    let flush = |cur: &mut Vec<&vi_providers::AsrSegment>, out: &mut Vec<TranscriptSpan>| {
        if cur.is_empty() {
            return;
        }
        let t0 = cur[0].start + offset;
        let t1 = cur[cur.len() - 1].end.max(cur[0].start) + offset;
        let text = cur
            .iter()
            .map(|s| s.text.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if text.is_empty() {
            cur.clear();
            return;
        }
        let confs: Vec<f32> = cur.iter().filter_map(|s| s.confidence).collect();
        let confidence = if confs.is_empty() {
            None
        } else {
            Some(confs.iter().sum::<f32>() / confs.len() as f32)
        };
        let words: Vec<serde_json::Value> = cur
            .iter()
            .flat_map(|s| s.words.iter())
            .map(|w| {
                let mut o = serde_json::json!({
                    "w": w.word.trim(),
                    "s": ((w.start + offset) * 1000.0).round() / 1000.0,
                    "e": ((w.end + offset) * 1000.0).round() / 1000.0,
                });
                if let Some(p) = w.probability {
                    o["p"] = serde_json::json!((f64::from(p) * 1000.0).round() / 1000.0);
                }
                o
            })
            .collect();
        out.push(TranscriptSpan {
            id: SpanId::new(),
            track_id: track.id,
            t0: Timestamp::from_secs_f64(t0, den),
            t1: Timestamp::from_secs_f64(t1.max(t0 + 0.001), den),
            text,
            speaker: None,
            language: language.map(str::to_string),
            confidence,
            words: if words.is_empty() {
                None
            } else {
                Some(serde_json::Value::Array(words))
            },
            provenance_id: prov,
        });
        cur.clear();
    };
    for s in &segments {
        if s.text.trim().is_empty() {
            continue;
        }
        let split = match cur.last() {
            Some(prev) => s.start - prev.end > GAP || s.end - cur[0].start > span_secs,
            None => false,
        };
        if split {
            flush(&mut cur, &mut out);
        }
        cur.push(s);
    }
    flush(&mut cur, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vi_core::model::TrackKind;
    use vi_core::{ProvenanceId, TrackId, VideoId};
    use vi_providers::{AsrSegment, AsrWord};

    fn seg(start: f64, end: f64, text: &str) -> AsrSegment {
        AsrSegment {
            start,
            end,
            text: text.into(),
            words: vec![AsrWord {
                word: format!(" {text}"),
                start,
                end,
                probability: Some(0.9),
            }],
            confidence: Some(0.8),
            no_speech_prob: None,
        }
    }

    #[test]
    fn long_segments_split_at_sentence_ends_or_pauses() {
        let mut words = Vec::new();
        for i in 0..30 {
            let t = i as f64;
            words.push(AsrWord {
                word: if i == 12 {
                    " twelve.".into()
                } else {
                    format!(" w{i}")
                },
                start: t,
                end: t + 0.8,
                probability: None,
            });
        }
        let seg = AsrSegment {
            start: 0.0,
            end: 29.8,
            text: "long".into(),
            words,
            confidence: None,
            no_speech_prob: None,
        };
        let pieces = split_long_segments(&[seg], 15.0);
        assert_eq!(pieces.len(), 3, "{pieces:?}");
        // First cut after the sentence end at word 12.
        assert!((pieces[0].end - 12.8).abs() < 1e-9, "{pieces:?}");
        assert!(pieces[0].text.ends_with("twelve."));
        assert!(pieces.iter().all(|p| p.end - p.start <= 15.0));
        let n: usize = pieces.iter().map(|p| p.words.len()).sum();
        assert_eq!(n, 30);
    }

    #[test]
    fn groups_segments_into_spans() {
        let track = Track {
            id: TrackId::new(),
            video_id: VideoId::new(),
            kind: TrackKind::Audio,
            stream_index: 1,
            codec: "aac".into(),
            timebase_num: 1,
            timebase_den: 48000,
            width: None,
            height: None,
            fps: None,
            sample_rate: Some(48000),
            channels: Some(2),
            language: None,
        };
        let segs = vec![
            seg(0.0, 4.0, "one"),
            seg(4.2, 9.0, "two"),
            seg(9.1, 16.0, "three"), // pushes over 15 s: new span
            seg(20.0, 22.0, "four"), // gap > 1.5 s: new span
            seg(22.1, 23.0, ""),     // empty: dropped
        ];
        let spans = group_segments(&segs, 100.0, 15.0, &track, Some("en"), ProvenanceId::new());
        assert_eq!(spans.len(), 3, "{spans:?}");
        assert_eq!(spans[0].text, "one two");
        assert!((spans[0].t0.as_secs_f64() - 100.0).abs() < 1e-6);
        assert!((spans[0].t1.as_secs_f64() - 109.0).abs() < 1e-6);
        assert_eq!(spans[1].text, "three");
        assert_eq!(spans[2].text, "four");
        let words = spans[0].words.as_ref().unwrap().as_array().unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[1]["s"], 104.2);
        assert_eq!(spans[0].language.as_deref(), Some("en"));
        assert!((spans[0].confidence.unwrap() - 0.8).abs() < 1e-6);
    }
}
