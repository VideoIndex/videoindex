//! `subtitle_import`: sidecar subtitles (`.srt`, `.vtt`) become a `subtitle`
//! Track with TranscriptSpans, so transferred yt-dlp downloads are searchable
//! before any ASR runs. Human-authored subtitles get confidence 1.0,
//! automatic captions 0.6, as reported by the `.info.json`.

use std::sync::Arc;

use async_trait::async_trait;
use vi_core::model::{Provenance, Span, Track, TrackKind, TranscriptSpan};
use vi_core::{Result, SpanId, TrackId};
use vi_media::sidecar;

use crate::operator::*;

/// Stream indexes at or above this mark sidecar tracks, which have no
/// container stream.
pub const SIDECAR_STREAM_BASE: u32 = 1000;

/// Target span length for grouping short cues, seconds.
const GROUP_SECS: f64 = 15.0;
/// Gap that starts a new span, seconds.
const GAP_SECS: f64 = 2.0;

/// Subtitle importer.
#[derive(Debug, Default)]
pub struct SubtitleImport;

impl SubtitleImport {
    /// New importer.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Operator for SubtitleImport {
    fn id(&self) -> &'static str {
        "subtitle_import"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[ItemKind::Media]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::TranscriptSpan]
    }

    fn cost_estimate(&self, _input: &InputSummary) -> CostEstimate {
        CostEstimate {
            cpu_secs: 0.1,
            usd: 0.0,
            provider_calls: 0,
        }
    }

    fn cache_params(&self, _ctx: &OpContext) -> serde_json::Value {
        serde_json::json!({ "group_secs": GROUP_SECS, "gap_secs": GAP_SECS })
    }

    fn replay_supported(&self) -> bool {
        true
    }

    async fn replay(&self, ctx: &OpContext) -> Result<Option<u64>> {
        let spans = ctx.storage.spans_by_operator(ctx.video, self.id()).await?;
        let mut n = 0;
        for sp in spans {
            if let Span::Transcript(t) = sp {
                ctx.emit(Item::TranscriptSpan(Arc::new(t))).await?;
                n += 1;
            }
        }
        Ok(Some(n))
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let Item::Media(media) = input.item else {
            return Err(ctx.err("expected the media item"));
        };
        // Idempotent: sidecar tracks are rebuilt from the files each run.
        ctx.storage
            .delete_tracks(media.video.id, TrackKind::Subtitle)
            .await?;
        let files = &media.acquired.subtitle_files;
        if files.is_empty() {
            ctx.progress(0);
            return Ok(OpOutput::default());
        }
        let mut emitted = 0u64;
        let mut stored = 0u64;
        // yt-dlp often writes the same auto-captions twice (`en` and
        // `en-orig`); one searchable copy is enough.
        let mut seen: Vec<blake3::Hash> = Vec::new();
        for (i, file) in files.iter().enumerate() {
            ctx.check_cancelled()?;
            let cues = match sidecar::parse_file(file) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("skipping unreadable subtitle {}: {e}", file.path.display());
                    continue;
                }
            };
            let mut hasher = blake3::Hasher::new();
            for c in &cues {
                hasher.update(c.text.as_bytes());
                hasher.update(b"\n");
            }
            let digest = hasher.finalize();
            if seen.contains(&digest) {
                tracing::info!(
                    file = %file.path.display(),
                    "skipping subtitle file identical to one already imported"
                );
                continue;
            }
            seen.push(digest);
            let grouped = sidecar::group_cues(&cues, GROUP_SECS, GAP_SECS);
            if grouped.is_empty() {
                continue;
            }
            let human = media.acquired.subtitle_is_human(file);
            let track = Track {
                id: TrackId::new(),
                video_id: media.video.id,
                kind: TrackKind::Subtitle,
                stream_index: SIDECAR_STREAM_BASE + i as u32,
                codec: match file.format {
                    sidecar::SubtitleFormat::Srt => "srt".into(),
                    sidecar::SubtitleFormat::Vtt => "webvtt".into(),
                },
                timebase_num: 1,
                timebase_den: 1000,
                width: None,
                height: None,
                fps: None,
                sample_rate: None,
                channels: None,
                language: file.language.clone(),
            };
            ctx.storage.put_tracks(std::slice::from_ref(&track)).await?;
            let prov = Provenance::local(
                self.id(),
                self.version(),
                serde_json::json!({
                    "file": file.path.file_name().and_then(|n| n.to_str()),
                    "format": file.format,
                    "language": file.language,
                    "human_authored": human,
                    "cues": cues.len(),
                    "group_secs": GROUP_SECS,
                }),
            );
            ctx.storage.put_provenance(&prov).await?;
            let spans: Vec<TranscriptSpan> = grouped
                .into_iter()
                .map(|c| TranscriptSpan {
                    id: SpanId::new(),
                    track_id: track.id,
                    t0: c.t0,
                    t1: c.t1,
                    text: c.text,
                    speaker: None,
                    language: file.language.clone(),
                    confidence: Some(if human { 1.0 } else { 0.6 }),
                    words: None,
                    provenance_id: prov.id,
                })
                .collect();
            for chunk in spans.chunks(256) {
                let batch: Vec<Span> = chunk.iter().cloned().map(Span::Transcript).collect();
                ctx.storage.put_spans(&batch).await?;
                stored += batch.len() as u64;
            }
            for sp in spans {
                ctx.emit(Item::TranscriptSpan(Arc::new(sp))).await?;
                emitted += 1;
            }
            tracing::info!(
                file = %file.path.display(),
                spans = emitted,
                human,
                "imported subtitles"
            );
        }
        ctx.progress(emitted);
        Ok(OpOutput { emitted, stored })
    }
}
