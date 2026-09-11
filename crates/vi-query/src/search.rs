//! Hybrid search, currently text-only, with temporal fusion.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use vi_core::model::{IndexState, SegmentLevel};
use vi_core::{Result, SegmentId, Timestamp, VideoId};
use vi_index::{BlobKey, Hit, Kind, Storage, TextQuery};

/// Reciprocal rank fusion constant.
const RRF_K: f64 = 60.0;
/// Window used to group hits when a video has no chapter segments, seconds.
const DEFAULT_WINDOW_SECS: f64 = 60.0;
/// Weight of secondary evidence in a group's score.
const SECONDARY_WEIGHT: f64 = 0.3;

/// What to search for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchRequest {
    /// Query text.
    pub query: String,
    /// Restrict to these videos; empty means all.
    pub videos: Vec<VideoId>,
    /// Restrict to these evidence kinds; empty means transcript, OCR, description.
    pub kinds: Vec<Kind>,
    /// Max results.
    pub k: usize,
}

impl SearchRequest {
    /// Top-`k` for `query` across the index.
    pub fn new(query: impl Into<String>, k: usize) -> Self {
        Self {
            query: query.into(),
            videos: Vec::new(),
            kinds: Vec::new(),
            k,
        }
    }
}

/// One piece of evidence behind a hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// transcript, ocr, description.
    pub kind: Kind,
    /// Start.
    pub t0: Timestamp,
    /// End.
    pub t1: Timestamp,
    /// Matched text.
    pub text: String,
    /// Fused score of this piece.
    pub score: f64,
}

/// A ranked result: a time range of one video with its evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    /// Video.
    pub video_id: VideoId,
    /// Video title, when known.
    pub title: Option<String>,
    /// Chapter segment containing the hit, when the video has chapters.
    pub segment_id: Option<SegmentId>,
    /// Chapter title, when known.
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
}

fn state_rank(s: IndexState) -> u8 {
    match s {
        IndexState::Failed => 0,
        IndexState::Acquired => 1,
        IndexState::Coarse => 2,
        IndexState::Fine => 3,
    }
}

/// Run a search.
pub async fn search(storage: &dyn Storage, req: &SearchRequest) -> Result<SearchResponse> {
    let kinds: Vec<Kind> = if req.kinds.is_empty() {
        vec![Kind::Transcript, Kind::Ocr, Kind::Description]
    } else {
        req.kinds.clone()
    };
    let k = req.k.max(1);
    // Over-fetch per kind so fusion has material.
    let per_kind = (k * 5).clamp(20, 200);

    // One ranked list per kind, then RRF.
    let mut fused: BTreeMap<String, (Hit, f64)> = BTreeMap::new();
    let mut candidates = 0usize;
    for kind in &kinds {
        let hits = storage
            .text_search(&TextQuery {
                query: req.query.clone(),
                videos: req.videos.clone(),
                kinds: vec![*kind],
                k: per_kind,
            })
            .await?;
        candidates += hits.len();
        for (rank, hit) in hits.into_iter().enumerate() {
            let rrf = 1.0 / (RRF_K + rank as f64 + 1.0);
            let e = fused.entry(hit.id.clone()).or_insert((hit, 0.0));
            e.1 += rrf;
        }
    }
    if fused.is_empty() {
        return Ok(SearchResponse {
            hits: Vec::new(),
            index_state: None,
            candidates,
        });
    }

    // Group by video and chapter (or fixed window).
    let mut by_video: BTreeMap<VideoId, Vec<(Hit, f64)>> = BTreeMap::new();
    for (_, (hit, score)) in fused {
        by_video.entry(hit.video_id).or_default().push((hit, score));
    }

    let mut results: Vec<SearchHit> = Vec::new();
    let mut lowest_state: Option<IndexState> = None;
    for (video_id, hits) in by_video {
        let video = storage.get_video(video_id).await?;
        if let Some(v) = &video {
            lowest_state = Some(match lowest_state {
                Some(s) if state_rank(s) <= state_rank(v.index_state) => s,
                _ => v.index_state,
            });
        }
        let title = video.as_ref().and_then(|v| v.title.clone());
        let chapters = storage.segments(video_id, SegmentLevel::Chapter).await?;

        // Group key: chapter index, or window index when no chapters.
        let mut groups: BTreeMap<(u8, usize), Vec<(Hit, f64)>> = BTreeMap::new();
        for (hit, score) in hits {
            let key = match chapters
                .iter()
                .position(|c| hit.t0 >= c.t0 && hit.t0 < c.t1)
            {
                Some(i) => (0u8, i),
                None => (
                    1u8,
                    (hit.t0.as_secs_f64() / DEFAULT_WINDOW_SECS).floor() as usize,
                ),
            };
            groups.entry(key).or_default().push((hit, score));
        }
        for ((kind, idx), mut members) in groups {
            members.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let best = members[0].1;
            let rest: f64 = members.iter().skip(1).map(|m| m.1).sum();
            let score = best + SECONDARY_WEIGHT * rest;
            let (segment_id, segment_title, t0, t1) = if kind == 0 {
                let c = &chapters[idx];
                (Some(c.id), c.title.clone(), c.t0, c.t1)
            } else {
                let t0 = members
                    .iter()
                    .map(|m| m.0.t0)
                    .min()
                    .unwrap_or(Timestamp::ZERO);
                let t1 = members
                    .iter()
                    .map(|m| m.0.t1)
                    .max()
                    .unwrap_or(t0)
                    .max(t0.add(Timestamp::from_secs(1)));
                (None, None, t0, t1)
            };
            let anchor = members[0].0.t0;
            let evidence: Vec<Evidence> = members
                .into_iter()
                .map(|(h, s)| Evidence {
                    kind: h.kind,
                    t0: h.t0,
                    t1: h.t1,
                    text: h.text,
                    score: s,
                })
                .collect();
            let thumbnail = nearest_thumbnail(storage, video_id, anchor).await?;
            results.push(SearchHit {
                video_id,
                title: title.clone(),
                segment_id,
                segment_title,
                t0,
                t1,
                score,
                evidence,
                thumbnail,
            });
        }
    }
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(k);
    Ok(SearchResponse {
        hits: results,
        index_state: lowest_state,
        candidates,
    })
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
