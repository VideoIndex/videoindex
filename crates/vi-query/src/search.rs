//! Hybrid search with temporal fusion (`docs/06-query-and-agents.md`).
//!
//! Ranked lists, each fused with reciprocal rank fusion:
//! 1. BM25 over transcript, OCR and description rows (one list per kind);
//! 2. text-vector search over the same rows with the `text_embed` model;
//! 3. image-vector search over frame embeddings with the `image_embed`
//!    model's text tower, so "slide with an architecture diagram" matches
//!    frames directly.
//!
//! Fused evidence is grouped into temporal units: scene segments when they
//! exist, else shots cut into pieces of about 60 s, else chapters, else
//! fixed 60 s windows. A group scores `best + 0.3 × sum(others)`, so a unit
//! where transcript, on-screen text and pixels agree outranks one lucky
//! keyword.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use vi_core::model::{IndexState, Segment, SegmentLevel, TargetKind};
use vi_core::{Result, SegmentId, Timestamp, VideoId};
use vi_index::{BlobKey, Hit, Kind, Storage, TextQuery, VectorQuery};
use vi_providers::ProviderRegistry;

/// Reciprocal rank fusion constant.
const RRF_K: f64 = 60.0;
/// Target length of a temporal unit when grouping by shots or windows, seconds.
const UNIT_SECS: f64 = 60.0;
/// Weight of secondary evidence in a group's score.
const SECONDARY_WEIGHT: f64 = 0.3;

/// What to search for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchRequest {
    /// Query text.
    pub query: String,
    /// Restrict to these videos; empty means all.
    pub videos: Vec<VideoId>,
    /// Restrict to these evidence kinds; empty means transcript, OCR,
    /// description and (when an image embedder is configured) frame.
    pub kinds: Vec<Kind>,
    /// Max results.
    pub k: usize,
    /// Skip the vector lists (BM25 only).
    pub text_only: bool,
}

impl SearchRequest {
    /// Top-`k` for `query` across the index.
    pub fn new(query: impl Into<String>, k: usize) -> Self {
        Self {
            query: query.into(),
            videos: Vec::new(),
            kinds: Vec::new(),
            k,
            text_only: false,
        }
    }
}

/// One piece of evidence behind a hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// transcript, ocr, description, frame.
    pub kind: Kind,
    /// Start.
    pub t0: Timestamp,
    /// End.
    pub t1: Timestamp,
    /// Matched text (empty for frames).
    pub text: String,
    /// Fused score of this piece.
    pub score: f64,
    /// Which lists ranked it: `bm25`, `text_vec`, `image_vec`.
    pub sources: Vec<String>,
}

/// A ranked result: a time range of one video with its evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    /// Video.
    pub video_id: VideoId,
    /// Video title, when known.
    pub title: Option<String>,
    /// Segment the unit came from (scene, shot or chapter), when any.
    pub segment_id: Option<SegmentId>,
    /// Chapter title containing the unit, when the video has chapters.
    pub segment_title: Option<String>,
    /// Start of the result range.
    pub t0: Timestamp,
    /// End of the result range.
    pub t1: Timestamp,
    /// Fused score, higher is better.
    pub score: f64,
    /// Evidence, best first.
    pub evidence: Vec<Evidence>,
    /// `blob:` URI of the nearest thumbnail, when sampled.
    pub thumbnail: Option<String>,
}

/// Search response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResponse {
    /// Ranked hits.
    pub hits: Vec<SearchHit>,
    /// Lowest index state among the videos searched (`acquired` < `coarse` < `fine`).
    pub index_state: Option<IndexState>,
    /// Raw evidence rows considered before fusion.
    pub candidates: usize,
    /// Lists that contributed: `bm25:transcript`, `text_vec`, `image_vec`, ...
    pub lists: Vec<String>,
    /// How hits were grouped: `scene`, `shot`, `chapter`, `window`.
    pub grouping: String,
}

fn state_rank(s: IndexState) -> u8 {
    match s {
        IndexState::Failed => 0,
        IndexState::Acquired => 1,
        IndexState::Coarse => 2,
        IndexState::Fine => 3,
    }
}

struct Fused {
    hit: Hit,
    score: f64,
    sources: Vec<String>,
}

/// Run a search. `providers` supplies the query embedders; with `None` (or
/// no `text_embed` / `image_embed` roles) the search is BM25 only.
pub async fn search(
    storage: &dyn Storage,
    providers: Option<&ProviderRegistry>,
    req: &SearchRequest,
) -> Result<SearchResponse> {
    let want_frames = req.kinds.is_empty() || req.kinds.contains(&Kind::Frame);
    let text_kinds: Vec<Kind> = if req.kinds.is_empty() {
        vec![Kind::Transcript, Kind::Ocr, Kind::Description]
    } else {
        req.kinds
            .iter()
            .copied()
            .filter(|k| matches!(k, Kind::Transcript | Kind::Ocr | Kind::Description))
            .collect()
    };
    let k = req.k.max(1);
    // Over-fetch per list so fusion has material.
    let per_list = (k * 5).clamp(20, 200);

    let mut fused: BTreeMap<String, Fused> = BTreeMap::new();
    let mut candidates = 0usize;
    let mut lists = Vec::new();
    let mut add_list = |name: &str, hits: Vec<Hit>, fused: &mut BTreeMap<String, Fused>| {
        if hits.is_empty() {
            return;
        }
        lists.push(name.to_string());
        candidates += hits.len();
        for (rank, hit) in hits.into_iter().enumerate() {
            let rrf = 1.0 / (RRF_K + rank as f64 + 1.0);
            let e = fused.entry(hit.id.clone()).or_insert(Fused {
                hit,
                score: 0.0,
                sources: Vec::new(),
            });
            e.score += rrf;
            e.sources.push(name.to_string());
        }
    };

    // 1. BM25, one list per kind.
    for kind in &text_kinds {
        let hits = storage
            .text_search(&TextQuery {
                query: req.query.clone(),
                videos: req.videos.clone(),
                kinds: vec![*kind],
                k: per_list,
            })
            .await?;
        add_list(&format!("bm25:{}", kind_name(*kind)), hits, &mut fused);
    }

    // 2 and 3. Vector lists, when embedders are configured.
    if !req.text_only {
        if let Some(reg) = providers {
            if !text_kinds.is_empty() {
                match reg.text_embedder() {
                    Ok(emb) => match emb.embed_query(&req.query).await {
                        Ok(resp) if !resp.vectors.is_empty() => {
                            let hits = storage
                                .vector_search(&VectorQuery {
                                    model: emb.model().to_string(),
                                    vector: resp.vectors[0].clone(),
                                    videos: req.videos.clone(),
                                    kinds: text_kinds
                                        .iter()
                                        .filter_map(|k| match k {
                                            Kind::Transcript => Some(TargetKind::TranscriptSpan),
                                            Kind::Ocr => Some(TargetKind::OcrSpan),
                                            Kind::Description => Some(TargetKind::Description),
                                            _ => None,
                                        })
                                        .collect(),
                                    k: per_list,
                                })
                                .await?;
                            add_list("text_vec", hits, &mut fused);
                        }
                        Ok(_) => {}
                        Err(e) => tracing::warn!("text query embedding failed: {e}"),
                    },
                    Err(e) => tracing::debug!("no text embedder for search: {e}"),
                }
            }
            if want_frames {
                match reg.image_embedder() {
                    Ok(emb) => match emb.embed_text(std::slice::from_ref(&req.query)).await {
                        Ok(resp) if !resp.vectors.is_empty() => {
                            let hits = storage
                                .vector_search(&VectorQuery {
                                    model: emb.model().to_string(),
                                    vector: resp.vectors[0].clone(),
                                    videos: req.videos.clone(),
                                    kinds: vec![TargetKind::Frame],
                                    k: per_list,
                                })
                                .await?;
                            add_list("image_vec", hits, &mut fused);
                        }
                        Ok(_) => {}
                        Err(e) => tracing::warn!("image query embedding failed: {e}"),
                    },
                    Err(e) => tracing::debug!("no image embedder for search: {e}"),
                }
            }
        }
    }

    if fused.is_empty() {
        return Ok(SearchResponse {
            hits: Vec::new(),
            index_state: None,
            candidates,
            lists,
            grouping: String::new(),
        });
    }

    // Group by video, then by temporal unit.
    let mut by_video: BTreeMap<VideoId, Vec<Fused>> = BTreeMap::new();
    for (_, f) in fused {
        by_video.entry(f.hit.video_id).or_default().push(f);
    }

    let mut results: Vec<SearchHit> = Vec::new();
    let mut lowest_state: Option<IndexState> = None;
    let mut grouping = String::new();
    for (video_id, hits) in by_video {
        let Some(video) = storage.get_video(video_id).await? else {
            // Evidence whose video row is gone (or was never resolved) is
            // not citable; drop it rather than guess a range.
            tracing::warn!(%video_id, "dropping {} hits of an unknown video", hits.len());
            continue;
        };
        lowest_state = Some(match lowest_state {
            Some(s) if state_rank(s) <= state_rank(video.index_state) => s,
            _ => video.index_state,
        });
        let title = video.title.clone();
        let duration = video.duration;
        let chapters = storage.segments(video_id, SegmentLevel::Chapter).await?;
        let (units, how) = temporal_units(storage, video_id, duration, &chapters).await?;
        if grouping.is_empty() {
            grouping = how.to_string();
        }

        let mut groups: BTreeMap<usize, Vec<Fused>> = BTreeMap::new();
        for f in hits {
            let idx = units
                .iter()
                .position(|u| f.hit.t0 >= u.t0 && f.hit.t0 < u.t1)
                .unwrap_or(units.len().saturating_sub(1));
            groups.entry(idx).or_default().push(f);
        }
        for (idx, mut members) in groups {
            members.sort_by(|a, b| b.score.total_cmp(&a.score));
            let best = members[0].score;
            let rest: f64 = members.iter().skip(1).map(|m| m.score).sum();
            let score = best + SECONDARY_WEIGHT * rest;
            let unit = units.get(idx);
            let (t0, t1, segment_id) = match unit {
                Some(u) => (u.t0, u.t1, u.segment),
                None => {
                    let t0 = members
                        .iter()
                        .map(|m| m.hit.t0)
                        .min()
                        .unwrap_or(Timestamp::ZERO);
                    (t0, t0.add(Timestamp::from_secs(1)), None)
                }
            };
            let chapter_title = chapters
                .iter()
                .find(|c| t0 >= c.t0 && t0 < c.t1)
                .and_then(|c| c.title.clone());
            let anchor = members[0].hit.t0;
            let evidence: Vec<Evidence> = members
                .into_iter()
                .map(|f| Evidence {
                    kind: f.hit.kind,
                    t0: f.hit.t0,
                    t1: f.hit.t1,
                    text: f.hit.text,
                    score: f.score,
                    sources: f.sources,
                })
                .collect();
            let thumbnail = nearest_thumbnail(storage, video_id, anchor).await?;
            results.push(SearchHit {
                video_id,
                title: title.clone(),
                segment_id,
                segment_title: chapter_title,
                t0,
                t1,
                score,
                evidence,
                thumbnail,
            });
        }
    }
    results.sort_by(|a, b| b.score.total_cmp(&a.score));
    results.truncate(k);
    Ok(SearchResponse {
        hits: results,
        index_state: lowest_state,
        candidates,
        lists,
        grouping,
    })
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Transcript => "transcript",
        Kind::Ocr => "ocr",
        Kind::Description => "description",
        Kind::Segment => "segment",
        Kind::Frame => "frame",
    }
}

/// A temporal grouping unit.
#[derive(Debug, Clone)]
struct Unit {
    t0: Timestamp,
    t1: Timestamp,
    segment: Option<SegmentId>,
}

/// Units for a video: scenes, else shots cut to about [`UNIT_SECS`], else
/// chapters, else fixed windows.
async fn temporal_units(
    storage: &dyn Storage,
    video: VideoId,
    duration: Timestamp,
    chapters: &[Segment],
) -> Result<(Vec<Unit>, &'static str)> {
    let scenes = storage.segments(video, SegmentLevel::Scene).await?;
    if !scenes.is_empty() {
        return Ok((
            scenes
                .iter()
                .map(|s| Unit {
                    t0: s.t0,
                    t1: s.t1,
                    segment: Some(s.id),
                })
                .collect(),
            "scene",
        ));
    }
    let shots = storage.segments(video, SegmentLevel::Shot).await?;
    if !shots.is_empty() {
        let mut units = Vec::new();
        for s in &shots {
            let len = s.t1.as_secs_f64() - s.t0.as_secs_f64();
            if len <= 1.5 * UNIT_SECS {
                units.push(Unit {
                    t0: s.t0,
                    t1: s.t1,
                    segment: Some(s.id),
                });
                continue;
            }
            // Long shot: equal pieces of about UNIT_SECS.
            let n = (len / UNIT_SECS).round().max(1.0) as usize;
            let step = len / n as f64;
            for i in 0..n {
                let a = s.t0.as_secs_f64() + step * i as f64;
                let b = if i + 1 == n {
                    s.t1.as_secs_f64()
                } else {
                    a + step
                };
                units.push(Unit {
                    t0: Timestamp::from_secs_f64(a, 1000),
                    t1: Timestamp::from_secs_f64(b, 1000),
                    segment: Some(s.id),
                });
            }
        }
        return Ok((units, "shot"));
    }
    if !chapters.is_empty() {
        return Ok((
            chapters
                .iter()
                .map(|c| Unit {
                    t0: c.t0,
                    t1: c.t1,
                    segment: Some(c.id),
                })
                .collect(),
            "chapter",
        ));
    }
    // Cap the fallback at a week of video so a bad duration cannot ask for
    // an absurd allocation.
    let total = duration.as_secs_f64().clamp(UNIT_SECS, 7.0 * 86_400.0);
    let n = (total / UNIT_SECS).ceil() as usize;
    Ok((
        (0..n)
            .map(|i| Unit {
                t0: Timestamp::from_secs_f64(i as f64 * UNIT_SECS, 1000),
                t1: Timestamp::from_secs_f64(((i + 1) as f64 * UNIT_SECS).min(total), 1000),
                segment: None,
            })
            .collect(),
        "window",
    ))
}

/// Thumbnail of the frame sample nearest to `t` on the video's sampled track.
async fn nearest_thumbnail(
    storage: &dyn Storage,
    video: VideoId,
    t: Timestamp,
) -> Result<Option<String>> {
    let lo = Timestamp::from_secs_f64((t.as_secs_f64() - 2.0).max(0.0), 1000);
    let hi = Timestamp::from_secs_f64(t.as_secs_f64() + 2.0, 1000);
    let w = storage.time_window(video, lo, hi, &[Kind::Frame]).await?;
    let best = w
        .frames
        .iter()
        .filter(|f| f.thumbnail_blob.is_some())
        .min_by(|a, b| {
            let da = (a.t.as_secs_f64() - t.as_secs_f64()).abs();
            let db = (b.t.as_secs_f64() - t.as_secs_f64()).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
    Ok(best
        .and_then(|f| f.thumbnail_blob.as_deref())
        .and_then(|k| BlobKey::parse(k).ok())
        .map(|k| k.uri()))
}
