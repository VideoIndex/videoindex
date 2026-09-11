//! `chapters`: keep imported chapters when the source had them; otherwise
//! segment the transcript by topic (TextTiling-style over scene text
//! embeddings) combined with slide-title changes from OCR, aiming at
//! chapters of a few minutes, and title them from the first prominent
//! on-screen text line.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use vi_core::config::roles;
use vi_core::model::{OcrSpan, Provenance, Segment, SegmentLevel, TranscriptSpan};
use vi_core::{Result, SegmentId, Timestamp};

use crate::operator::*;

/// Shortest chapter, seconds.
pub const MIN_CHAPTER_SECS: f64 = 180.0;
/// Target chapter length, seconds (boundaries are chosen so chapters land
/// around this).
pub const TARGET_CHAPTER_SECS: f64 = 600.0;

/// Chapter operator.
#[derive(Debug, Default)]
pub struct Chapters {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    media: Option<Arc<MediaItem>>,
    scenes: Vec<Segment>,
    spans: Vec<Arc<TranscriptSpan>>,
    ocr: Vec<Arc<OcrSpan>>,
}

impl Chapters {
    /// New operator.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Depth score at each scene boundary: how much the similarity between the
/// text on either side drops relative to the neighbourhood. `embs[i]` is
/// the (normalised) embedding of scene `i`'s text.
pub fn depth_scores(embs: &[Option<Vec<f32>>]) -> Vec<f32> {
    let n = embs.len();
    if n < 2 {
        return Vec::new();
    }
    // Similarity across each boundary using windows of up to 2 scenes.
    let sim = |a: usize, b: usize| -> f32 {
        let mut s = 0.0;
        let mut c = 0;
        for i in a.saturating_sub(1)..=a {
            for j in b..(b + 2).min(n) {
                if let (Some(x), Some(y)) = (&embs[i], &embs[j]) {
                    s += x.iter().zip(y).map(|(p, q)| p * q).sum::<f32>();
                    c += 1;
                }
            }
        }
        if c == 0 {
            0.5
        } else {
            s / c as f32
        }
    };
    let gaps: Vec<f32> = (0..n - 1).map(|i| sim(i, i + 1)).collect();
    // Depth: rise to the left peak plus rise to the right peak.
    (0..gaps.len())
        .map(|i| {
            let mut l = gaps[i];
            let mut j = i;
            while j > 0 && gaps[j - 1] >= l {
                l = gaps[j - 1];
                j -= 1;
            }
            let mut r = gaps[i];
            let mut j = i;
            while j + 1 < gaps.len() && gaps[j + 1] >= r {
                r = gaps[j + 1];
                j += 1;
            }
            (l - gaps[i]) + (r - gaps[i])
        })
        .collect()
}

/// Choose chapter boundaries (indices of scenes that start a chapter)
/// from depth scores and OCR title changes, honouring the length limits.
pub fn choose_boundaries(
    scene_ranges: &[(f64, f64)],
    depth: &[f32],
    title_change: &[bool],
) -> Vec<usize> {
    let n = scene_ranges.len();
    if n == 0 {
        return Vec::new();
    }
    let total = scene_ranges[n - 1].1 - scene_ranges[0].0;
    let want = ((total / TARGET_CHAPTER_SECS).round() as usize).max(1);
    if want <= 1 || n < 2 {
        return vec![0];
    }
    // Score every boundary; OCR title changes add a fixed bonus.
    let mut scored: Vec<(f32, usize)> = (0..n - 1)
        .map(|i| {
            let d = depth.get(i).copied().unwrap_or(0.0);
            let bonus = if title_change.get(i).copied().unwrap_or(false) {
                0.25
            } else {
                0.0
            };
            (d + bonus, i + 1)
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut starts = vec![0usize];
    for (_, idx) in scored {
        if starts.len() >= want {
            break;
        }
        let t = scene_ranges[idx].0;
        // Keep every chapter at least MIN_CHAPTER_SECS long.
        let mut all = starts.clone();
        all.push(idx);
        all.sort_unstable();
        let mut ok = true;
        for w in all.windows(2) {
            if scene_ranges[w[1]].0 - scene_ranges[w[0]].0 < MIN_CHAPTER_SECS {
                ok = false;
            }
        }
        let last = all[all.len() - 1];
        if scene_ranges[n - 1].1 - scene_ranges[last].0 < MIN_CHAPTER_SECS {
            ok = false;
        }
        if ok && t > 0.0 {
            starts = all;
        }
    }
    starts
}

/// A slide title: a few words of mostly letters, not a logo or a URL.
fn looks_like_title(text: &str) -> bool {
    let n = text.chars().count();
    let words = text.split_whitespace().count();
    (12..=90).contains(&n)
        && words >= 3
        && text.chars().filter(|c| c.is_alphabetic()).count() >= n / 2
        && !text.contains("://")
        && !text.contains('@')
}

/// Where a title sits: the upper part of the frame, large enough to read.
fn title_position_ok(o: &OcrSpan) -> bool {
    match o.bbox {
        Some(b) => b.y < 0.6 && b.h >= 0.03,
        None => true,
    }
}

#[async_trait]
impl Operator for Chapters {
    fn id(&self) -> &'static str {
        "chapters"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[
            ItemKind::Media,
            ItemKind::Scene,
            ItemKind::TranscriptSpan,
            ItemKind::OcrSpan,
        ]
    }

    fn optional_inputs(&self) -> &[InputKind] {
        &[ItemKind::TranscriptSpan, ItemKind::OcrSpan]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::Chapter]
    }

    fn required_roles(&self) -> &[&'static str] {
        &[roles::TEXT_EMBED]
    }

    fn cache_params(&self, ctx: &OpContext) -> serde_json::Value {
        let model = ctx
            .providers
            .text_embedder()
            .map(|e| e.model().to_string())
            .unwrap_or_default();
        serde_json::json!({"min_secs": MIN_CHAPTER_SECS, "target_secs": TARGET_CHAPTER_SECS, "text_model": model})
    }

    fn replay_supported(&self) -> bool {
        true
    }

    async fn replay(&self, ctx: &OpContext) -> Result<Option<u64>> {
        let _ = self.state.lock().await.media.take();
        let chapters = ctx
            .storage
            .segments(ctx.video, SegmentLevel::Chapter)
            .await?;
        let mut n = 0;
        for c in chapters {
            ctx.emit(Item::Chapter(Arc::new(c))).await?;
            n += 1;
        }
        Ok(Some(n))
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        CostEstimate {
            cpu_secs: input.duration_secs * 0.002,
            usd: 0.0,
            provider_calls: (input.duration_secs / 60.0).ceil() as u64,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let mut st = self.state.lock().await;
        match input.item {
            Item::Media(m) => st.media = Some(m),
            Item::Scene(s) => st.scenes.push((*s).clone()),
            Item::TranscriptSpan(s) => st.spans.push(s),
            Item::OcrSpan(o) => st.ocr.push(o),
            _ => return Err(ctx.err("expected media, a scene, a transcript span or an OCR span")),
        }
        Ok(OpOutput::default())
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let mut st = self.state.lock().await;
        let Some(media) = st.media.take() else {
            return Ok(OpOutput::default());
        };
        let video = media.video.id;
        // Imported chapters (sidecar or container) win.
        let existing = ctx.storage.segments(video, SegmentLevel::Chapter).await?;
        if !existing.is_empty() {
            let mut n = 0;
            for c in existing {
                ctx.emit(Item::Chapter(Arc::new(c))).await?;
                n += 1;
            }
            tracing::info!(video = %video, chapters = n, "kept imported chapters");
            return Ok(OpOutput {
                emitted: n,
                stored: 0,
            });
        }
        let mut scenes = std::mem::take(&mut st.scenes);
        if scenes.is_empty() {
            scenes = ctx.storage.segments(video, SegmentLevel::Scene).await?;
        }
        if scenes.is_empty() {
            scenes = ctx.storage.segments(video, SegmentLevel::Shot).await?;
        }
        if scenes.is_empty() {
            return Ok(OpOutput::default());
        }
        scenes.sort_by_key(|s| s.t0);
        let spans = std::mem::take(&mut st.spans);
        let ocr = std::mem::take(&mut st.ocr);
        // Text per scene, embedded with the text_embed role.
        let texts: Vec<String> = scenes
            .iter()
            .map(|s| {
                let (a, b) = (s.t0.as_secs_f64(), s.t1.as_secs_f64());
                spans
                    .iter()
                    .filter(|sp| sp.t0.as_secs_f64() < b && sp.t1.as_secs_f64() > a)
                    .map(|sp| sp.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        let embedder = ctx.providers.text_embedder()?;
        let mut embs: Vec<Option<Vec<f32>>> = vec![None; scenes.len()];
        let mut cost = 0.0;
        if ctx.allow_provider_call() {
            let idx: Vec<usize> = texts
                .iter()
                .enumerate()
                .filter(|(_, t)| t.len() > 20)
                .map(|(i, _)| i)
                .collect();
            for chunk in idx.chunks(embedder.max_batch().max(1)) {
                let batch: Vec<String> = chunk
                    .iter()
                    .map(|i| texts[*i].chars().take(2000).collect())
                    .collect();
                match embedder.embed(&batch).await {
                    Ok(resp) => {
                        ctx.record_cost(&resp.stats);
                        cost += resp.stats.cost_usd;
                        for (i, v) in chunk.iter().zip(resp.vectors) {
                            embs[*i] = Some(v);
                        }
                    }
                    Err(e) => ctx.record_failure(
                        scenes[chunk[0]].t0,
                        scenes[chunk[chunk.len() - 1]].t1,
                        e,
                    ),
                }
            }
        }
        let depth = depth_scores(&embs);
        // OCR title change at a scene boundary: the first title-like line
        // in the new scene differs from the previous scene's.
        let first_title = |s: &Segment| -> Option<String> {
            let (a, b) = (s.t0.as_secs_f64(), s.t1.as_secs_f64());
            ocr.iter()
                .filter(|o| {
                    o.t.as_secs_f64() >= a
                        && o.t.as_secs_f64() < b
                        && looks_like_title(&o.text)
                        && title_position_ok(o)
                })
                .min_by(|x, y| {
                    // Prefer the highest, widest line: largest box height then top.
                    let hx = x.bbox.map(|b| b.h).unwrap_or(0.0);
                    let hy = y.bbox.map(|b| b.h).unwrap_or(0.0);
                    hy.total_cmp(&hx).then(x.t.cmp(&y.t))
                })
                .map(|o| o.text.clone())
        };
        let titles: Vec<Option<String>> = scenes.iter().map(first_title).collect();
        let title_change: Vec<bool> = (0..scenes.len().saturating_sub(1))
            .map(|i| match (&titles[i], &titles[i + 1]) {
                (Some(a), Some(b)) => a != b,
                _ => false,
            })
            .collect();
        let ranges: Vec<(f64, f64)> = scenes
            .iter()
            .map(|s| (s.t0.as_secs_f64(), s.t1.as_secs_f64()))
            .collect();
        let starts = choose_boundaries(&ranges, &depth, &title_change);
        let prov = Provenance::local(
            self.id(),
            self.version(),
            serde_json::json!({"scenes": scenes.len(), "chapters": starts.len(), "text_model": embedder.model(), "cost_usd": cost}),
        );
        ctx.storage.put_provenance(&prov).await?;
        let mut chapters = Vec::with_capacity(starts.len());
        for (k, &s) in starts.iter().enumerate() {
            let end_scene = starts.get(k + 1).map(|e| e - 1).unwrap_or(scenes.len() - 1);
            let t0 = if k == 0 {
                Timestamp::ZERO
            } else {
                scenes[s].t0
            };
            let t1 = if k + 1 == starts.len() {
                scenes[end_scene].t1.max(media.video.duration)
            } else {
                scenes[end_scene].t1
            };
            let title = (s..=end_scene).find_map(|i| titles[i].clone());
            chapters.push(Segment {
                id: SegmentId::new(),
                video_id: video,
                level: SegmentLevel::Chapter,
                parent_id: None,
                t0,
                t1,
                keyframe_sample_id: scenes[s].keyframe_sample_id,
                title,
                summary: None,
                provenance_id: prov.id,
            });
        }
        ctx.storage.put_segments(&chapters).await?;
        let n = chapters.len() as u64;
        for c in chapters {
            ctx.emit(Item::Chapter(Arc::new(c))).await?;
        }
        tracing::info!(video = %video, chapters = n, "chapters written from topic segmentation");
        Ok(OpOutput {
            emitted: n,
            stored: n,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_peaks_where_topics_change() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let embs: Vec<Option<Vec<f32>>> = vec![
            Some(a.clone()),
            Some(a.clone()),
            Some(a),
            Some(b.clone()),
            Some(b.clone()),
            Some(b),
        ];
        let d = depth_scores(&embs);
        assert_eq!(d.len(), 5);
        let (imax, _) = d
            .iter()
            .enumerate()
            .max_by(|x, y| x.1.total_cmp(y.1))
            .unwrap();
        assert_eq!(imax, 2, "{d:?}");
    }

    #[test]
    fn boundaries_respect_lengths() {
        // Ten 120 s scenes = 20 min: aim for two chapters.
        let scenes: Vec<(f64, f64)> = (0..10)
            .map(|i| (i as f64 * 120.0, (i + 1) as f64 * 120.0))
            .collect();
        let mut depth = vec![0.0f32; 9];
        depth[4] = 1.0; // strongest at 10 min
        depth[0] = 0.9; // strong but would make a 2-minute chapter: rejected
        let starts = choose_boundaries(&scenes, &depth, &[false; 9]);
        assert_eq!(starts, vec![0, 5], "{starts:?}");
        // A short video gets one chapter.
        let short: Vec<(f64, f64)> = vec![(0.0, 60.0), (60.0, 120.0)];
        assert_eq!(choose_boundaries(&short, &[1.0], &[true]), vec![0]);
        assert!(looks_like_title(
            "Inside T-Bench: Building Reliable Evaluation"
        ));
        assert!(!looks_like_title("42"));
    }
}
