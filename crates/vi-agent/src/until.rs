//! The `until` clamp (live C5): everything an ask reads, and every citation
//! it makes, stays below a time bound. The rule is
//! `until.unwrap_or(watermark).unwrap_or(duration)`: the caller's bound when
//! it gave one (a live session passes its watermark, batch evaluation the
//! question's "ask at" time), else a live video's watermark, else the
//! video's duration, which is today's behaviour for batch videos.
//!
//! The clamp is applied where every tool reads: the [`Storage`] the tools
//! see during an ask is a [`BoundedStorage`] that cuts durations, ranges,
//! search filters and segment lists at the bound, so `search`, `timeline`,
//! the text tools, `view`, `zoom` and any tool added later are bounded
//! without a change to each. Citations are clamped as the answer streams,
//! and the live videos are re-read at the start of every turn so the bound
//! (and the head in the prompt) follow a stream during a long answer.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use bytes::Bytes;
use serde_json::json;
use vi_core::model::*;
use vi_core::{FrameSampleId, JobId, ProvenanceId, Result, TimeRange, Timestamp, TrackId, VideoId};
use vi_index::{
    BlobKey, Hit, IndexStats, Kind, Manifest, MentionQuery, Result as IndexResult, Storage,
    TextQuery, VectorQuery, VideoMentions, Window,
};
use vi_providers::{ContentPart, Message, Role};

use crate::agent::AskRequest;
use crate::citations::Cite;
use crate::tool_ext;

/// Where a live video stood at the last refresh, seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveMark {
    /// Latest decoded time (the video's `duration` while live).
    pub head: f64,
    /// Time up to which every coarse stage has committed, when known.
    pub watermark: Option<f64>,
}

/// The bound of one ask: the caller's `until`, and the live videos' marks
/// re-read at the start of every turn so the clamps follow a stream during
/// a long answer.
#[derive(Debug)]
pub struct Bound {
    until: Option<f64>,
    live: Mutex<BTreeMap<VideoId, LiveMark>>,
}

impl Bound {
    /// A bound at `until` seconds (`None`: only live watermarks bound).
    pub fn new(until: Option<f64>) -> Self {
        Self {
            until: until.filter(|u| u.is_finite()).map(|u| u.max(0.0)),
            live: Mutex::new(BTreeMap::new()),
        }
    }

    /// The caller's bound, seconds.
    pub fn until(&self) -> Option<f64> {
        self.until
    }

    fn marks(&self) -> MutexGuard<'_, BTreeMap<VideoId, LiveMark>> {
        self.live.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Where a live video stood at the last refresh.
    pub fn mark(&self, id: VideoId) -> Option<LiveMark> {
        self.marks().get(&id).copied()
    }

    /// Record the live videos as they are now; a video that is no longer
    /// live drops out.
    pub fn refresh(&self, videos: &[Video]) {
        let mut m = self.marks();
        m.clear();
        for v in videos.iter().filter(|v| v.index_state.is_live()) {
            m.insert(
                v.id,
                LiveMark {
                    head: v.duration.as_secs_f64(),
                    watermark: v.watermark.map(|w| w.as_secs_f64()),
                },
            );
        }
    }

    /// The bound for a video known only by id: `until`, else the live
    /// video's watermark; `None` when nothing bounds it.
    pub fn for_id(&self, id: VideoId) -> Option<f64> {
        self.until
            .or_else(|| self.mark(id).and_then(|m| m.watermark))
    }

    /// What a video's reads and ranges clamp to, seconds:
    /// `until.unwrap_or(watermark).unwrap_or(duration)`, never above the
    /// duration. A batch video without `until` keeps its duration.
    pub fn clamp_secs(&self, video: &Video) -> f64 {
        let duration = video.duration.as_secs_f64();
        let watermark = video
            .watermark
            .filter(|_| video.index_state.is_live())
            .map(|w| w.as_secs_f64())
            .or_else(|| self.mark(video.id).and_then(|m| m.watermark));
        self.until
            .or(watermark)
            .map_or(duration, |b| b.min(duration))
    }

    /// The video as the ask sees it: its duration cut to the bound, so every
    /// range a tool clamps to the duration stops there.
    pub fn clamp_video(&self, mut video: Video) -> Video {
        let b = self.clamp_secs(&video);
        if b < video.duration.as_secs_f64() {
            video.duration = Timestamp::from_secs_f64(b, 1000);
        }
        video
    }

    /// Whether anything is bounded at all.
    pub fn is_active(&self) -> bool {
        self.until.is_some() || !self.marks().is_empty()
    }
}

/// A duration in seconds as the ask running on this task allows it: cut
/// at the caller's `until`, unchanged outside an ask. The tools' range
/// arguments clamp to this (the video's own bound is already applied by
/// [`BoundedStorage`]; this is the belt to that braces).
pub fn clamp_secs_for_ask(duration: f64) -> f64 {
    match tool_ext::current().and_then(|s| s.bound.until()) {
        Some(u) => duration.min(u),
        None => duration,
    }
}

/// A citation as the ask running on this task allows it: dropped when it
/// starts at or after the video's bound, cut when it runs past it, unchanged
/// outside an ask or for an unbounded video.
pub fn clamp_citation(c: Cite) -> Option<Cite> {
    let Some(scope) = tool_ext::current() else {
        return Some(c);
    };
    match scope.bound.for_id(c.video_id) {
        None => Some(c),
        Some(b) if c.t0 >= b => None,
        Some(b) => Some(Cite {
            t1: c.t1.min(b),
            ..c
        }),
    }
}

/// Rewrite the `- <id> | <duration> | <title>` lines of the system prompt's
/// video list for the videos whose reads are bounded below their duration,
/// so the model is told the same extent the tools report.
fn rewrite_video_lines(system: &mut String, videos: &[Video], bound: &Bound) {
    let mut lines: Vec<String> = system.split('\n').map(str::to_string).collect();
    let mut changed = false;
    for v in videos {
        let b = bound.clamp_secs(v);
        let prefix = format!("- {} | ", v.id);
        for line in lines.iter_mut().filter(|l| l.starts_with(&prefix)) {
            let new = format!(
                "- {} | {} | {}",
                v.id,
                vi_perceive::grid::hms(b),
                v.title.clone().unwrap_or_default()
            );
            if *line != new {
                *line = new;
                changed = true;
            }
        }
    }
    if changed {
        *system = lines.join("\n");
    }
}

/// Re-read the live videos at the start of a turn (one query) so the
/// clamps, and the head shown in the system prompt's video list, follow the
/// stream during a long answer. Nothing to do outside an ask or when no
/// video is live.
pub async fn refresh_live(storage: &dyn Storage, messages: &mut [Message]) -> Result<()> {
    let Some(scope) = tool_ext::current() else {
        return Ok(());
    };
    let live = storage.live_videos().await?;
    scope.bound.refresh(&live);
    if live.is_empty() {
        return Ok(());
    }
    if let Some(m) = messages.first_mut().filter(|m| m.role == Role::System) {
        for part in &mut m.parts {
            if let ContentPart::Text(t) = part {
                rewrite_video_lines(t, &live, &scope.bound);
            }
        }
    }
    Ok(())
}

/// The hash an addendum is recorded under: the prompt-hash form (blake3,
/// first 16 hex characters), so it sits next to the prompt hashes in the
/// provenance table.
pub fn addendum_hash(text: &str) -> String {
    vi_providers::cost::prompt_hash(text)
}

/// The provenance row an addendum is recorded in, content-addressed from
/// its text, so the same addendum always names the same row and a reader
/// with the text can find it.
pub fn addendum_provenance_id(text: &str) -> ProvenanceId {
    let hex = BlobKey::for_bytes(text.as_bytes()).0;
    let n = u128::from_str_radix(&hex[..32], 16).unwrap_or(0);
    let mut u = ProvenanceId::nil().0;
    u.0 = n;
    ProvenanceId::from(u)
}

/// Finish the system prompt for one ask: the video list shows the bounded
/// extents, the bound is stated, the addendum is appended, and the addendum
/// is recorded in provenance by hash (operator `ask`, `prompt_hash` the
/// addendum's hash, `params.kind = "system_addendum"`). Outside an ask only
/// the addendum text is appended.
pub async fn apply_addendum(
    system: &mut String,
    req: &AskRequest,
    storage: Arc<dyn Storage>,
) -> Result<()> {
    if let Some(scope) = tool_ext::current() {
        let videos = storage.list_videos().await?;
        scope.bound.refresh(&videos);
        if scope.bound.is_active() {
            rewrite_video_lines(system, &videos, &scope.bound);
        }
    }
    if let Some(u) = req.until.filter(|u| u.is_finite()) {
        system.push_str(&format!(
            "\n\nAnswer as of {}: the index is read only below that time, nothing at or after it can be seen or cited, and durations are reported up to it.\n",
            vi_perceive::grid::hms(u.max(0.0))
        ));
    }
    let Some(text) = req
        .system_addendum
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    else {
        return Ok(());
    };
    system.push_str("\n\n");
    system.push_str(text);
    system.push('\n');
    let hash = addendum_hash(text);
    let mut prov = Provenance::local(
        "ask",
        1,
        json!({
            "kind": "system_addendum",
            "hash": hash,
            "until": req.until,
            "chars": text.chars().count(),
        }),
    );
    prov.id = addendum_provenance_id(text);
    prov.prompt_hash = Some(hash);
    storage.put_provenance(&prov).await?;
    Ok(())
}

/// Storage as an ask sees it: reads cut at the bound, writes passed
/// through. Built by [`bounded`] for the tools of one ask.
pub struct BoundedStorage {
    inner: Arc<dyn Storage>,
    bound: Arc<Bound>,
}

impl std::fmt::Debug for BoundedStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundedStorage")
            .field("bound", &self.bound)
            .finish()
    }
}

/// The storage the tools of the ask running on this task read from: the
/// bounded view when inside an ask, the storage itself otherwise (the MCP
/// server's direct tool calls).
pub fn bounded(inner: Arc<dyn Storage>) -> Arc<dyn Storage> {
    match tool_ext::current() {
        Some(scope) => Arc::new(BoundedStorage {
            inner,
            bound: scope.bound.clone(),
        }),
        None => inner,
    }
}

/// `existing` cut at `b` seconds from above; the whole range below `b` when
/// there was none.
fn range_below(existing: Option<TimeRange>, b: f64) -> Option<TimeRange> {
    let hi = Timestamp::from_secs_f64(b, 1000);
    Some(match existing {
        None => TimeRange {
            t0: Timestamp::ZERO,
            t1: hi,
        },
        Some(r) => TimeRange {
            t0: r.t0,
            t1: r.t1.min(hi).max(r.t0),
        },
    })
}

impl BoundedStorage {
    /// The bound a search over `videos` gets: `until` for any scope; without
    /// it a live video's watermark when the search names exactly that
    /// video (a search across several live videos without `until` cannot
    /// be cut per video in one query and is left as it is).
    fn query_bound(&self, videos: &[VideoId]) -> Option<f64> {
        self.bound.until().or_else(|| match videos {
            [one] => self.bound.for_id(*one),
            _ => None,
        })
    }

    fn cut_segments(&self, video: VideoId, mut segs: Vec<Segment>) -> Vec<Segment> {
        if let Some(b) = self.bound.for_id(video) {
            let hi = Timestamp::from_secs_f64(b, 1000);
            segs.retain(|s| s.t0 < hi);
            for s in &mut segs {
                if s.t1 > hi {
                    s.t1 = hi;
                }
            }
        }
        segs
    }
}

#[async_trait]
impl Storage for BoundedStorage {
    async fn put_video(&self, v: &Video) -> IndexResult<()> {
        self.inner.put_video(v).await
    }
    async fn get_video(&self, id: VideoId) -> IndexResult<Option<Video>> {
        Ok(self
            .inner
            .get_video(id)
            .await?
            .map(|v| self.bound.clamp_video(v)))
    }
    async fn find_video_by_hash(&self, content_hash: &str) -> IndexResult<Option<Video>> {
        Ok(self
            .inner
            .find_video_by_hash(content_hash)
            .await?
            .map(|v| self.bound.clamp_video(v)))
    }
    async fn list_videos(&self) -> IndexResult<Vec<Video>> {
        Ok(self
            .inner
            .list_videos()
            .await?
            .into_iter()
            .map(|v| self.bound.clamp_video(v))
            .collect())
    }
    async fn set_index_state(&self, id: VideoId, state: IndexState) -> IndexResult<()> {
        self.inner.set_index_state(id, state).await
    }
    async fn set_live_progress(
        &self,
        video: VideoId,
        head: Timestamp,
        watermark: Timestamp,
    ) -> IndexResult<()> {
        self.inner.set_live_progress(video, head, watermark).await
    }
    async fn live_videos(&self) -> IndexResult<Vec<Video>> {
        Ok(self
            .inner
            .live_videos()
            .await?
            .into_iter()
            .map(|v| self.bound.clamp_video(v))
            .collect())
    }
    async fn put_tracks(&self, t: &[Track]) -> IndexResult<()> {
        self.inner.put_tracks(t).await
    }
    async fn tracks(&self, video: VideoId) -> IndexResult<Vec<Track>> {
        self.inner.tracks(video).await
    }
    async fn delete_tracks(&self, video: VideoId, kind: TrackKind) -> IndexResult<u64> {
        self.inner.delete_tracks(video, kind).await
    }
    async fn put_segments(&self, s: &[Segment]) -> IndexResult<()> {
        self.inner.put_segments(s).await
    }
    async fn delete_segments(&self, video: VideoId, level: SegmentLevel) -> IndexResult<u64> {
        self.inner.delete_segments(video, level).await
    }
    async fn segments(&self, video: VideoId, level: SegmentLevel) -> IndexResult<Vec<Segment>> {
        let segs = self.inner.segments(video, level).await?;
        Ok(self.cut_segments(video, segs))
    }
    async fn put_frame_samples(&self, s: &[FrameSample]) -> IndexResult<()> {
        self.inner.put_frame_samples(s).await
    }
    async fn update_frame_phash(&self, updates: &[(FrameSampleId, u64)]) -> IndexResult<()> {
        self.inner.update_frame_phash(updates).await
    }
    async fn update_frame_thumbnail(
        &self,
        updates: &[(FrameSampleId, BlobKey)],
    ) -> IndexResult<()> {
        self.inner.update_frame_thumbnail(updates).await
    }
    async fn delete_frame_samples(&self, track: TrackId) -> IndexResult<u64> {
        self.inner.delete_frame_samples(track).await
    }
    async fn frame_samples(
        &self,
        track: TrackId,
        range: Option<TimeRange>,
    ) -> IndexResult<Vec<FrameSample>> {
        // A track does not name its video; only the caller's bound applies.
        let range = match self.bound.until() {
            Some(u) => range_below(range, u),
            None => range,
        };
        self.inner.frame_samples(track, range).await
    }
    async fn put_spans(&self, s: &[Span]) -> IndexResult<()> {
        self.inner.put_spans(s).await
    }
    async fn delete_spans_by_operator(&self, track: TrackId, operator: &str) -> IndexResult<u64> {
        self.inner.delete_spans_by_operator(track, operator).await
    }
    async fn spans_by_operator(&self, video: VideoId, operator: &str) -> IndexResult<Vec<Span>> {
        self.inner.spans_by_operator(video, operator).await
    }
    async fn put_descriptions(&self, d: &[Description]) -> IndexResult<()> {
        self.inner.put_descriptions(d).await
    }
    async fn put_embeddings(&self, e: &[Embedding]) -> IndexResult<()> {
        self.inner.put_embeddings(e).await
    }
    async fn put_provenance(&self, p: &Provenance) -> IndexResult<ProvenanceId> {
        self.inner.put_provenance(p).await
    }
    async fn get_provenance(&self, id: ProvenanceId) -> IndexResult<Option<Provenance>> {
        self.inner.get_provenance(id).await
    }
    async fn get_embeddings(
        &self,
        model: &str,
        targets: &[(TargetKind, String)],
    ) -> IndexResult<Vec<Option<Vec<f32>>>> {
        self.inner.get_embeddings(model, targets).await
    }
    async fn descriptions(&self, video: VideoId) -> IndexResult<Vec<Description>> {
        self.inner.descriptions(video).await
    }
    async fn put_entities(
        &self,
        entities: &[Entity],
        mentions: &[EntityMention],
    ) -> IndexResult<()> {
        self.inner.put_entities(entities, mentions).await
    }
    async fn entities(&self, video: VideoId) -> IndexResult<Vec<Entity>> {
        self.inner.entities(video).await
    }
    async fn put_events(&self, events: &[Event]) -> IndexResult<()> {
        self.inner.put_events(events).await
    }
    async fn events(&self, video: VideoId) -> IndexResult<Vec<Event>> {
        let mut out = self.inner.events(video).await?;
        if let Some(b) = self.bound.for_id(video) {
            let hi = Timestamp::from_secs_f64(b, 1000);
            out.retain(|e| e.t0 < hi);
        }
        Ok(out)
    }
    async fn delete_extractions(&self, video: VideoId) -> IndexResult<u64> {
        self.inner.delete_extractions(video).await
    }
    async fn text_search(&self, q: &TextQuery) -> IndexResult<Vec<Hit>> {
        let mut q = q.clone();
        if let Some(b) = self.query_bound(&q.videos) {
            q.time_range = range_below(q.time_range, b);
        }
        self.inner.text_search(&q).await
    }
    async fn find_mentions(&self, q: &MentionQuery) -> IndexResult<Vec<VideoMentions>> {
        let mut q = q.clone();
        if let Some(b) = self.query_bound(&q.videos) {
            q.time_range = range_below(q.time_range, b);
        }
        self.inner.find_mentions(&q).await
    }
    async fn vector_search(&self, q: &VectorQuery) -> IndexResult<Vec<Hit>> {
        let mut q = q.clone();
        if let Some(b) = self.query_bound(&q.videos) {
            q.time_range = range_below(q.time_range, b);
        }
        self.inner.vector_search(&q).await
    }
    async fn time_window(
        &self,
        video: VideoId,
        t0: Timestamp,
        t1: Timestamp,
        kinds: &[Kind],
    ) -> IndexResult<Window> {
        let t1 = match self.bound.for_id(video) {
            Some(b) => t1.min(Timestamp::from_secs_f64(b, 1000)),
            None => t1,
        };
        if t1 <= t0 {
            return Ok(Window::default());
        }
        self.inner.time_window(video, t0, t1, kinds).await
    }
    async fn spans_since(
        &self,
        video: VideoId,
        kinds: &[Kind],
        since: Timestamp,
    ) -> IndexResult<Vec<Span>> {
        let mut out = self.inner.spans_since(video, kinds, since).await?;
        if let Some(b) = self.bound.for_id(video) {
            let hi = Timestamp::from_secs_f64(b, 1000);
            out.retain(|s| match s {
                Span::Transcript(t) => t.t0 < hi,
                Span::Ocr(o) => o.t < hi,
            });
        }
        Ok(out)
    }
    async fn segments_since(
        &self,
        video: VideoId,
        level: SegmentLevel,
        since: Timestamp,
    ) -> IndexResult<Vec<Segment>> {
        let segs = self.inner.segments_since(video, level, since).await?;
        Ok(self.cut_segments(video, segs))
    }
    async fn put_blob(&self, key: &BlobKey, bytes: Bytes) -> IndexResult<()> {
        self.inner.put_blob(key, bytes).await
    }
    async fn get_blob(&self, key: &BlobKey) -> IndexResult<Option<Bytes>> {
        self.inner.get_blob(key).await
    }
    async fn put_session(
        &self,
        id: &str,
        state: &serde_json::Value,
        ttl_secs: u64,
    ) -> IndexResult<()> {
        self.inner.put_session(id, state, ttl_secs).await
    }
    async fn get_session(&self, id: &str) -> IndexResult<Option<serde_json::Value>> {
        self.inner.get_session(id).await
    }
    async fn checkpoint(&self, job: JobId, state: &JobState) -> IndexResult<()> {
        self.inner.checkpoint(job, state).await
    }
    async fn load_checkpoint(&self, job: JobId) -> IndexResult<Option<JobState>> {
        self.inner.load_checkpoint(job).await
    }
    async fn list_jobs(&self) -> IndexResult<Vec<JobState>> {
        self.inner.list_jobs().await
    }
    fn cache_dir(&self) -> Option<std::path::PathBuf> {
        self.inner.cache_dir()
    }
    async fn manifest(&self) -> IndexResult<Manifest> {
        self.inner.manifest().await
    }
    async fn stats(&self) -> IndexResult<IndexStats> {
        self.inner.stats().await
    }
    async fn compact(&self) -> IndexResult<()> {
        self.inner.compact().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn video(secs: i64, state: IndexState, watermark: Option<i64>) -> Video {
        Video {
            id: VideoId::new(),
            source_uri: "x".into(),
            content_hash: "h".into(),
            title: Some("A talk".into()),
            description: None,
            channel: None,
            published_at: None,
            duration: Timestamp::from_secs(secs),
            start_wallclock: None,
            probe: serde_json::json!({}),
            index_state: state,
            created_at: Utc::now(),
            watermark: watermark.map(Timestamp::from_secs),
            live_ended_at: None,
        }
    }

    #[test]
    fn clamp_rule_until_then_watermark_then_duration() {
        let batch = video(120, IndexState::Coarse, None);
        let live = video(90, IndexState::Live, Some(84));
        let none = Bound::new(None);
        assert_eq!(none.clamp_secs(&batch), 120.0, "batch: duration");
        assert_eq!(none.clamp_secs(&live), 84.0, "live: watermark");
        assert!(!none.is_active());
        let sixty = Bound::new(Some(60.0));
        assert_eq!(sixty.clamp_secs(&batch), 60.0);
        assert_eq!(sixty.clamp_secs(&live), 60.0, "until beats the watermark");
        assert_eq!(
            Bound::new(Some(500.0)).clamp_secs(&batch),
            120.0,
            "never past the duration"
        );
        assert_eq!(Bound::new(Some(f64::NAN)).until(), None);
        assert_eq!(Bound::new(Some(-3.0)).until(), Some(0.0));
        let cut = sixty.clamp_video(batch.clone());
        assert_eq!(cut.duration, Timestamp::from_secs(60));
        assert_eq!(none.clamp_video(batch.clone()), batch);
        // A live video whose row has no watermark yet is bounded by the
        // refreshed mark, and by nothing before the first refresh.
        let fresh = video(30, IndexState::Live, None);
        assert_eq!(none.clamp_secs(&fresh), 30.0);
        assert_eq!(none.for_id(fresh.id), None);
        let mut marked = fresh.clone();
        marked.watermark = Some(Timestamp::from_secs(26));
        none.refresh(std::slice::from_ref(&marked));
        assert_eq!(none.for_id(fresh.id), Some(26.0));
        assert_eq!(none.clamp_secs(&fresh), 26.0);
        assert!(none.is_active());
        assert_eq!(none.mark(fresh.id).map(|m| m.head), Some(30.0));
        // Ended: it drops out of the marks.
        none.refresh(&[video(30, IndexState::Coarse, Some(26))]);
        assert!(!none.is_active());
        // Outside an ask nothing clamps a duration.
        assert_eq!(clamp_secs_for_ask(120.0), 120.0);
    }

    #[test]
    fn ranges_cut_from_above() {
        let r = range_below(None, 60.0).unwrap();
        assert_eq!((r.t0, r.t1), (Timestamp::ZERO, Timestamp::from_secs(60)));
        let r = range_below(
            TimeRange::new(Timestamp::from_secs(10), Timestamp::from_secs(100)),
            60.0,
        )
        .unwrap();
        assert_eq!(
            (r.t0, r.t1),
            (Timestamp::from_secs(10), Timestamp::from_secs(60))
        );
        // Already below: unchanged. Entirely above: empty at its start.
        let r = range_below(
            TimeRange::new(Timestamp::from_secs(10), Timestamp::from_secs(20)),
            60.0,
        )
        .unwrap();
        assert_eq!(r.t1, Timestamp::from_secs(20));
        let r = range_below(
            TimeRange::new(Timestamp::from_secs(70), Timestamp::from_secs(80)),
            60.0,
        )
        .unwrap();
        assert_eq!(
            (r.t0, r.t1),
            (Timestamp::from_secs(70), Timestamp::from_secs(70))
        );
    }

    #[test]
    fn video_lines_follow_the_bound() {
        let v = video(120, IndexState::Coarse, None);
        let other = video(50, IndexState::Coarse, None);
        let mut system = format!(
            "Rules.\n\nVideos in this index (id | duration | title):\n- {} | 00:02:00 | A talk\n- {} | 00:00:50 | A talk\n",
            v.id, other.id
        );
        let before = system.clone();
        rewrite_video_lines(&mut system, &[v.clone(), other.clone()], &Bound::new(None));
        assert_eq!(system, before, "nothing bounded, nothing rewritten");
        rewrite_video_lines(
            &mut system,
            &[v.clone(), other.clone()],
            &Bound::new(Some(60.0)),
        );
        assert!(
            system.contains(&format!("- {} | 00:01:00 | A talk", v.id)),
            "{system}"
        );
        assert!(
            system.contains(&format!("- {} | 00:00:50 | A talk", other.id)),
            "{system}"
        );
        assert!(system.ends_with('\n') && system.starts_with("Rules.\n\n"));
    }

    #[test]
    fn citations_pass_outside_an_ask() {
        let c = Cite {
            video_id: VideoId::new(),
            t0: 70.0,
            t1: 80.0,
        };
        assert_eq!(clamp_citation(c.clone()), Some(c));
    }

    #[test]
    fn addendum_ids_are_content_addressed() {
        let a = addendum_provenance_id("This video is a live stream.");
        let b = addendum_provenance_id("This video is a live stream.");
        let c = addendum_provenance_id("Another preamble.");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, ProvenanceId::nil());
        assert_eq!(ProvenanceId::parse(&a.to_string()).unwrap(), a);
        assert_eq!(addendum_hash("x").len(), 16);
    }
}
