//! `scenes`: group shots into scenes of about 20 s to 3 min using the
//! visual similarity of their keyframes (stored SigLIP embeddings) and
//! transcript continuity (a span crossing the boundary keeps the shots
//! together). Shots get their scene as `parent_id`.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use vi_core::model::{Provenance, Segment, SegmentLevel, TargetKind, TranscriptSpan};
use vi_core::{Result, SegmentId, Timestamp};

use crate::operator::*;

/// Shortest scene, seconds (shorter ones merge with a neighbour).
pub const MIN_SCENE_SECS: f64 = 20.0;
/// Longest scene, seconds (longer runs split at the weakest boundary).
pub const MAX_SCENE_SECS: f64 = 180.0;
/// Cosine similarity at or above which adjacent shots look alike.
pub const SIMILAR: f32 = 0.85;

/// Scene grouping operator.
#[derive(Debug, Default)]
pub struct Scenes {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    media: Option<Arc<MediaItem>>,
    shots: Vec<Segment>,
    spans: Vec<(f64, f64)>,
}

impl Scenes {
    /// New operator.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Cut shots longer than [`MAX_SCENE_SECS`] into pieces of roughly
/// `MAX_SCENE_SECS / 1.5`, moving each cut to the nearest transcript span
/// boundary within 10 s when there is one. Pieces keep the shot's keyframe
/// and provenance; they are not written back as shots.
pub fn split_long_shots(shots: Vec<Segment>, spans: &[(f64, f64)]) -> Vec<Segment> {
    let target = MAX_SCENE_SECS / 1.5;
    let mut out = Vec::with_capacity(shots.len());
    for shot in shots {
        let (a, b) = (shot.t0.as_secs_f64(), shot.t1.as_secs_f64());
        let len = b - a;
        if len <= MAX_SCENE_SECS {
            out.push(shot);
            continue;
        }
        let n = (len / target).round().max(2.0) as usize;
        let step = len / n as f64;
        let mut cuts: Vec<f64> = (1..n).map(|i| a + step * i as f64).collect();
        for c in &mut cuts {
            // Snap to a span start within 10 s.
            if let Some(best) = spans
                .iter()
                .map(|(s0, _)| *s0)
                .filter(|s0| (*s0 - *c).abs() <= 10.0 && *s0 > a + 1.0 && *s0 < b - 1.0)
                .min_by(|x, y| (x - *c).abs().total_cmp(&(y - *c).abs()))
            {
                *c = best;
            }
        }
        let mut edges = vec![a];
        edges.extend(cuts);
        edges.push(b);
        for w in edges.windows(2) {
            let mut piece = shot.clone();
            piece.t0 = Timestamp::from_secs_f64(w[0], 1000);
            piece.t1 = Timestamp::from_secs_f64(w[1], 1000);
            if w[0] == a {
                piece.t0 = shot.t0;
            }
            if w[1] == b {
                piece.t1 = shot.t1;
            }
            out.push(piece);
        }
    }
    out
}

/// Boundary strength between consecutive shots: 1 means a clean break.
/// Lower when the keyframes look alike or a transcript span crosses.
fn boundary_strength(sim: Option<f32>, span_crosses: bool) -> f32 {
    let mut s = match sim {
        Some(v) => (1.0 - v).clamp(0.0, 1.0),
        None => 0.5,
    };
    if span_crosses {
        s *= 0.5;
    }
    s
}

/// Group shots into scenes. Pure so it is testable: `sims[i]` is the
/// similarity between shot `i` and `i + 1`, `crossing[i]` whether a span
/// crosses that boundary. Returns index ranges `[start, end)` of shots.
pub fn group(shots: &[(f64, f64)], sims: &[Option<f32>], crossing: &[bool]) -> Vec<(usize, usize)> {
    if shots.is_empty() {
        return Vec::new();
    }
    let strength: Vec<f32> = (0..shots.len().saturating_sub(1))
        .map(|i| {
            boundary_strength(
                sims.get(i).copied().flatten(),
                crossing.get(i).copied().unwrap_or(false),
            )
        })
        .collect();
    // 1. Start with one scene per shot, then merge across weak boundaries
    //    (similar or spoken across) while the result stays under the max.
    let mut scenes: Vec<(usize, usize)> = (0..shots.len()).map(|i| (i, i + 1)).collect();
    let dur = |a: usize, b: usize| shots[b - 1].1 - shots[a].0;
    let mut merged = true;
    while merged {
        merged = false;
        let mut i = 0;
        while i + 1 < scenes.len() {
            let b = scenes[i].1 - 1; // last shot of scene i; boundary b
            let weak = strength[b] < 0.5;
            let short = dur(scenes[i].0, scenes[i].1) < MIN_SCENE_SECS
                || dur(scenes[i + 1].0, scenes[i + 1].1) < MIN_SCENE_SECS;
            if (weak || short) && dur(scenes[i].0, scenes[i + 1].1) <= MAX_SCENE_SECS {
                scenes[i].1 = scenes[i + 1].1;
                scenes.remove(i + 1);
                merged = true;
            } else {
                i += 1;
            }
        }
    }
    // 2. A remaining short scene joins its weaker-boundary neighbour even
    //    past the max, except when it is the only scene.
    let mut i = 0;
    while scenes.len() > 1 && i < scenes.len() {
        if dur(scenes[i].0, scenes[i].1) < MIN_SCENE_SECS {
            let left = if i > 0 {
                Some(strength[scenes[i].0 - 1])
            } else {
                None
            };
            let right = if i + 1 < scenes.len() {
                Some(strength[scenes[i].1 - 1])
            } else {
                None
            };
            let join_left = match (left, right) {
                (Some(l), Some(r)) => l <= r,
                (Some(_), None) => true,
                _ => false,
            };
            if join_left {
                scenes[i - 1].1 = scenes[i].1;
                scenes.remove(i);
                continue;
            } else {
                scenes[i].1 = scenes[i + 1].1;
                scenes.remove(i + 1);
                continue;
            }
        }
        i += 1;
    }
    scenes
}

#[async_trait]
impl Operator for Scenes {
    fn id(&self) -> &'static str {
        "scenes"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[ItemKind::Media, ItemKind::Shot, ItemKind::TranscriptSpan]
    }

    fn optional_inputs(&self) -> &[InputKind] {
        &[ItemKind::TranscriptSpan]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::Scene]
    }

    fn cache_params(&self, ctx: &OpContext) -> serde_json::Value {
        let model = ctx
            .providers
            .image_embedder()
            .map(|e| e.model().to_string())
            .unwrap_or_default();
        serde_json::json!({
            "min_secs": MIN_SCENE_SECS, "max_secs": MAX_SCENE_SECS, "similar": SIMILAR,
            "image_model": model,
        })
    }

    fn replay_supported(&self) -> bool {
        true
    }

    async fn replay(&self, ctx: &OpContext) -> Result<Option<u64>> {
        let _ = self.state.lock().await.media.take();
        let scenes = ctx.storage.segments(ctx.video, SegmentLevel::Scene).await?;
        let mut n = 0;
        for s in scenes {
            ctx.emit(Item::Scene(Arc::new(s))).await?;
            n += 1;
        }
        Ok(Some(n))
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        CostEstimate {
            cpu_secs: input.duration_secs * 0.0005,
            usd: 0.0,
            provider_calls: 0,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let mut st = self.state.lock().await;
        match input.item {
            Item::Media(m) => st.media = Some(m),
            Item::Shot(s) => st.shots.push((*s).clone()),
            Item::TranscriptSpan(sp) => {
                let sp: &TranscriptSpan = &sp;
                st.spans.push((sp.t0.as_secs_f64(), sp.t1.as_secs_f64()));
            }
            _ => return Err(ctx.err("expected media, a shot or a transcript span")),
        }
        Ok(OpOutput::default())
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let mut st = self.state.lock().await;
        let Some(media) = st.media.take() else {
            return Ok(OpOutput::default());
        };
        let video = media.video.id;
        ctx.storage
            .delete_segments(video, SegmentLevel::Scene)
            .await?;
        let mut shots = std::mem::take(&mut st.shots);
        if shots.is_empty() {
            // Nothing to group: one scene per video keeps the level total.
            shots = ctx.storage.segments(video, SegmentLevel::Shot).await?;
        }
        if shots.is_empty() {
            return Ok(OpOutput::default());
        }
        shots.sort_by_key(|s| s.t0);
        // A static camera can hold one shot for half an hour; scenes must
        // still be a few minutes at most, so long shots are cut into equal
        // pieces of about MAX_SCENE_SECS / 1.5 first, preferring a cut at a
        // transcript span boundary when one is near.
        let spans_for_split = st.spans.clone();
        let original_shots = shots.clone();
        shots = split_long_shots(shots, &spans_for_split);
        // Keyframe embeddings, when an image embedder is bound and vectors exist.
        let sims: Vec<Option<f32>> = match ctx.providers.image_embedder() {
            Ok(emb) => {
                let targets: Vec<(TargetKind, String)> = shots
                    .iter()
                    .map(|s| {
                        (
                            TargetKind::Frame,
                            s.keyframe_sample_id
                                .map(|k| k.to_string())
                                .unwrap_or_default(),
                        )
                    })
                    .collect();
                let vecs = ctx.storage.get_embeddings(emb.model(), &targets).await?;
                (0..shots.len().saturating_sub(1))
                    .map(|i| match (&vecs[i], &vecs[i + 1]) {
                        (Some(a), Some(b)) => {
                            Some(a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>())
                        }
                        _ => None,
                    })
                    .collect()
            }
            Err(_) => vec![None; shots.len().saturating_sub(1)],
        };
        let spans = std::mem::take(&mut st.spans);
        let crossing: Vec<bool> = (0..shots.len().saturating_sub(1))
            .map(|i| {
                let b = shots[i].t1.as_secs_f64();
                spans.iter().any(|(a, z)| *a < b - 0.5 && *z > b + 0.5)
            })
            .collect();
        let ranges: Vec<(f64, f64)> = shots
            .iter()
            .map(|s| (s.t0.as_secs_f64(), s.t1.as_secs_f64()))
            .collect();
        let groups = group(&ranges, &sims, &crossing);
        let prov = Provenance::local(
            self.id(),
            self.version(),
            serde_json::json!({"shots": shots.len(), "scenes": groups.len(), "min_secs": MIN_SCENE_SECS, "max_secs": MAX_SCENE_SECS}),
        );
        ctx.storage.put_provenance(&prov).await?;
        let mut scenes = Vec::with_capacity(groups.len());
        for (a, b) in &groups {
            let first = &shots[*a];
            let last = &shots[*b - 1];
            scenes.push(Segment {
                id: SegmentId::new(),
                video_id: video,
                level: SegmentLevel::Scene,
                parent_id: None,
                t0: first.t0,
                t1: last.t1,
                keyframe_sample_id: first.keyframe_sample_id,
                title: None,
                summary: None,
                provenance_id: prov.id,
            });
        }
        // The first scene starts at 0 and the last ends at the duration.
        if let Some(f) = scenes.first_mut() {
            f.t0 = Timestamp::ZERO;
        }
        if let Some(l) = scenes.last_mut() {
            l.t1 = l.t1.max(media.video.duration);
        }
        ctx.storage.put_segments(&scenes).await?;
        // Point the original shots at the scene containing their start.
        let mut originals = original_shots;
        for s in &mut originals {
            let mid = s.t0.as_secs_f64() + 0.001;
            if let Some(scene) = scenes
                .iter()
                .find(|sc| mid >= sc.t0.as_secs_f64() && mid < sc.t1.as_secs_f64())
            {
                s.parent_id = Some(scene.id);
            }
        }
        ctx.storage.put_segments(&originals).await?;
        let n = scenes.len() as u64;
        for s in scenes {
            ctx.emit(Item::Scene(Arc::new(s))).await?;
        }
        tracing::info!(video = %video, shots = originals.len(), pieces = shots.len(), scenes = n, "scenes written");
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
    fn groups_short_shots_and_splits_at_strong_boundaries() {
        // Twelve 10 s shots; strong visual breaks everywhere, no speech
        // crossing: shots merge only up to the minimum length.
        let shots: Vec<(f64, f64)> = (0..12)
            .map(|i| (i as f64 * 10.0, (i + 1) as f64 * 10.0))
            .collect();
        let sims = vec![Some(0.1); 11];
        let crossing = vec![false; 11];
        let g = group(&shots, &sims, &crossing);
        assert!(
            g.iter()
                .all(|(a, b)| shots[*b - 1].1 - shots[*a].0 >= MIN_SCENE_SECS),
            "{g:?}"
        );
        assert_eq!(g.first().unwrap().0, 0);
        assert_eq!(g.last().unwrap().1, 12);
        for w in g.windows(2) {
            assert_eq!(w[0].1, w[1].0, "no gaps");
        }
        // Similar shots merge into one scene up to the maximum length.
        let sims = vec![Some(0.95); 11];
        let g = group(&shots, &sims, &crossing);
        assert_eq!(g.len(), 1, "{g:?}");
        // Twenty 10 s similar shots: 200 s exceeds the max, so two scenes.
        let shots: Vec<(f64, f64)> = (0..20)
            .map(|i| (i as f64 * 10.0, (i + 1) as f64 * 10.0))
            .collect();
        let g = group(&shots, &[Some(0.95); 19], &[false; 19]);
        assert!(g.len() >= 2, "{g:?}");
        assert!(g
            .iter()
            .all(|(a, b)| shots[*b - 1].1 - shots[*a].0 <= MAX_SCENE_SECS));
        assert!(group(&[], &[], &[]).is_empty());
    }

    #[test]
    fn long_shots_are_cut_into_pieces_at_span_starts() {
        let shot = Segment {
            id: SegmentId::new(),
            video_id: vi_core::VideoId::new(),
            level: SegmentLevel::Shot,
            parent_id: None,
            t0: Timestamp::ZERO,
            t1: Timestamp::from_secs(600),
            keyframe_sample_id: None,
            title: None,
            summary: None,
            provenance_id: vi_core::ProvenanceId::new(),
        };
        let spans = vec![(118.0, 130.0), (245.0, 260.0)];
        let pieces = split_long_shots(vec![shot], &spans);
        assert_eq!(pieces.len(), 5, "{pieces:?}");
        assert_eq!(pieces[0].t0, Timestamp::ZERO);
        assert_eq!(pieces[4].t1, Timestamp::from_secs(600));
        assert!(
            (pieces[0].t1.as_secs_f64() - 118.0).abs() < 1e-6,
            "snapped to a span start"
        );
        assert!((pieces[1].t1.as_secs_f64() - 245.0).abs() < 1e-6);
        for w in pieces.windows(2) {
            assert_eq!(w[0].t1, w[1].t0);
        }
    }
}
