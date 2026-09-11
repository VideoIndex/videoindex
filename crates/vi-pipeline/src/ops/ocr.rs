//! `ocr`: on-screen text through the `ocr` role (RapidOCR locally by
//! default). Frames are read when they look different from the last frame
//! read (pHash distance, or the pixel-change measure that catches slide
//! text changes pHash misses, or a time gap); lines identical to those of
//! the previous read are not stored again, so a slide shown for a minute
//! yields one `OcrSpan` per line at the moment it appeared.

use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use vi_core::config::roles;
use vi_core::model::{BBox, OcrSpan, Span};
use vi_core::{cpu, Result, SpanId, Timestamp};
use vi_perceive::shot::FrameSignature;
use vi_perceive::{hamming, PHASH_DEDUP_DISTANCE};
use vi_providers::{provenance_for, ImageData};

use crate::operator::*;

/// OCR operator.
#[derive(Debug, Default)]
pub struct Ocr {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    last_hash: Option<u64>,
    last_sig: Option<FrameSignature>,
    last_t: Option<Timestamp>,
    last_lines: BTreeSet<String>,
    frames: u64,
    read: u64,
    stored: u64,
    emitted: u64,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("frames", &self.frames)
            .field("read", &self.read)
            .field("stored", &self.stored)
            .finish()
    }
}

impl Ocr {
    /// New operator.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Operator for Ocr {
    fn id(&self) -> &'static str {
        "ocr"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[ItemKind::Hashed]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::OcrSpan]
    }

    fn required_roles(&self) -> &[&'static str] {
        &[roles::OCR]
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        // About one frame in five is read; detection plus recognition of a
        // slide costs about 150 ms on the CPU.
        CostEstimate {
            cpu_secs: input.expected_samples as f64 * 0.2 * 0.15,
            usd: 0.0,
            provider_calls: (input.expected_samples as f64 * 0.2).ceil() as u64,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let Item::Hashed {
            sample,
            phash,
            t,
            frame,
        } = input.item
        else {
            return Err(ctx.err("expected a hashed frame"));
        };
        let gate = ctx.config.models.ocr.clone();
        let f = frame.clone();
        let sig = cpu::run(move || FrameSignature::from_frame(&f))
            .await?
            .ok_or_else(|| ctx.err("frame is not RGB24"))?;
        let mut st = self.state.lock().await;
        st.frames += 1;
        let changed = match (&st.last_sig, st.last_hash, st.last_t) {
            (Some(prev), Some(h), Some(lt)) => {
                hamming(h, phash) > PHASH_DEDUP_DISTANCE
                    || prev.pixel_change(&sig, 24.0) >= gate.min_pixel_change
                    || t.as_secs_f64() - lt.as_secs_f64() >= gate.max_gap_secs
            }
            _ => true,
        };
        if !changed {
            drop(frame);
            return Ok(OpOutput::default());
        }
        // Copy pixels out and release the decoder slot before the provider
        // call, which can take a while.
        let (w, h) = (frame.width, frame.height);
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            rgb.extend_from_slice(frame.row(y));
        }
        drop(frame);
        st.last_hash = Some(phash);
        st.last_sig = Some(sig);
        st.last_t = Some(t);
        st.read += 1;
        let ocr = ctx.providers.ocr()?;
        let resp = ocr
            .read(&ImageData::Rgb8 {
                width: w,
                height: h,
                data: Arc::from(rgb),
            })
            .await
            .map_err(|e| ctx.err(e.to_string()))?;
        let lines: Vec<_> = resp
            .lines
            .into_iter()
            .filter(|l| l.text.chars().count() >= gate.min_chars)
            .collect();
        let current: BTreeSet<String> = lines.iter().map(|l| l.text.clone()).collect();
        let new_lines: Vec<_> = lines
            .iter()
            .filter(|l| !st.last_lines.contains(&l.text))
            .collect();
        st.last_lines = current;
        if new_lines.is_empty() {
            return Ok(OpOutput::default());
        }
        let prov = provenance_for(
            self.id(),
            self.version(),
            &resp.stats,
            None,
            serde_json::json!({ "lines": new_lines.len(), "frame": sample.to_string() }),
        );
        ctx.storage.put_provenance(&prov).await?;
        let spans: Vec<OcrSpan> = new_lines
            .iter()
            .map(|l| OcrSpan {
                id: SpanId::new(),
                frame_sample_id: sample,
                t,
                text: l.text.clone(),
                bbox: l.bbox.map(|b| BBox {
                    x: b.x,
                    y: b.y,
                    w: b.w,
                    h: b.h,
                }),
                confidence: l.confidence,
                provenance_id: prov.id,
            })
            .collect();
        let batch: Vec<Span> = spans.iter().cloned().map(Span::Ocr).collect();
        ctx.storage.put_spans(&batch).await?;
        st.stored += spans.len() as u64;
        let n = spans.len() as u64;
        for sp in spans {
            ctx.emit(Item::OcrSpan(Arc::new(sp))).await?;
        }
        st.emitted += n;
        if st.frames % 64 == 0 {
            ctx.progress(st.frames);
        }
        Ok(OpOutput {
            emitted: n,
            stored: n,
        })
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let st = self.state.lock().await;
        tracing::info!(
            frames = st.frames,
            read = st.read,
            lines = st.stored,
            "on-screen text written"
        );
        ctx.progress(ctx.expected_items.unwrap_or(st.frames));
        Ok(OpOutput::default())
    }
}
