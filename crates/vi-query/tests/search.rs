//! Search over a hand-built index: no media needed.

#![allow(clippy::unwrap_used)]

use chrono::Utc;
use vi_core::model::*;
use vi_core::*;
use vi_index::{BlobKey, EmbeddedIndex, Kind, Storage};
use vi_query::{search, SearchRequest};

async fn seed(idx: &EmbeddedIndex) -> (VideoId, TrackId) {
    let vid = VideoId::new();
    idx.put_video(&Video {
        id: vid,
        source_uri: "https://example/v".into(),
        content_hash: "h".into(),
        title: Some("Retrieval workshop".into()),
        description: None,
        channel: None,
        published_at: None,
        duration: Timestamp::from_secs(3600),
        start_wallclock: None,
        probe: serde_json::json!({}),
        index_state: IndexState::Coarse,
        created_at: Utc::now(),
    })
    .await
    .unwrap();
    let track = Track {
        id: TrackId::new(),
        video_id: vid,
        kind: TrackKind::Subtitle,
        stream_index: 1000,
        codec: "srt".into(),
        timebase_num: 1,
        timebase_den: 1000,
        width: None,
        height: None,
        fps: None,
        sample_rate: None,
        channels: None,
        language: Some("en".into()),
    };
    let vtrack = Track {
        id: TrackId::new(),
        kind: TrackKind::Video,
        stream_index: 0,
        codec: "h264".into(),
        ..track.clone()
    };
    idx.put_tracks(&[track.clone(), vtrack.clone()])
        .await
        .unwrap();
    let prov = Provenance::local("test", 1, serde_json::json!({}));
    idx.put_provenance(&prov).await.unwrap();
    idx.put_segments(&[
        Segment {
            id: SegmentId::new(),
            video_id: vid,
            level: SegmentLevel::Chapter,
            parent_id: None,
            t0: Timestamp::from_secs(0),
            t1: Timestamp::from_secs(1800),
            keyframe_sample_id: None,
            title: Some("Part one".into()),
            summary: None,
            provenance_id: prov.id,
        },
        Segment {
            id: SegmentId::new(),
            video_id: vid,
            level: SegmentLevel::Chapter,
            parent_id: None,
            t0: Timestamp::from_secs(1800),
            t1: Timestamp::from_secs(3600),
            keyframe_sample_id: None,
            title: Some("Part two".into()),
            summary: None,
            provenance_id: prov.id,
        },
    ])
    .await
    .unwrap();
    let span = |t0: i64, text: &str| {
        Span::Transcript(TranscriptSpan {
            id: SpanId::new(),
            track_id: track.id,
            t0: Timestamp::from_secs(t0),
            t1: Timestamp::from_secs(t0 + 15),
            text: text.into(),
            speaker: None,
            language: Some("en".into()),
            confidence: Some(1.0),
            words: None,
            provenance_id: prov.id,
        })
    };
    idx.put_spans(&[
        span(
            100,
            "today we cover hybrid retrieval with BM25 and dense vectors",
        ),
        span(
            1840,
            "hybrid retrieval returns candidate segments for the agent",
        ),
        span(1900, "lunch will be served in the atrium"),
    ])
    .await
    .unwrap();
    // A frame with a thumbnail near the second-half hit, plus OCR on it.
    let frame = FrameSample {
        id: FrameSampleId::new(),
        track_id: vtrack.id,
        t: Timestamp::from_secs(1841),
        pts: 0,
        is_keyframe: true,
        phash: Some(1),
        thumbnail_blob: Some(BlobKey::for_bytes(b"thumb").0),
        width: 640,
        height: 360,
    };
    idx.put_frame_samples(std::slice::from_ref(&frame))
        .await
        .unwrap();
    idx.put_spans(&[Span::Ocr(OcrSpan {
        id: SpanId::new(),
        frame_sample_id: frame.id,
        t: frame.t,
        text: "Hybrid retrieval: BM25 + dense".into(),
        bbox: None,
        confidence: Some(0.9),
        provenance_id: prov.id,
    })])
    .await
    .unwrap();
    (vid, track.id)
}

#[tokio::test]
async fn fuses_kinds_and_groups_by_chapter() {
    let dir = tempfile::tempdir().unwrap();
    let idx = EmbeddedIndex::create(&dir.path().join("q.vidx")).unwrap();
    let (vid, _) = seed(&idx).await;

    let r = search(&idx, None, &SearchRequest::new("hybrid retrieval", 10))
        .await
        .unwrap();
    assert_eq!(r.index_state, Some(IndexState::Coarse));
    assert_eq!(r.hits.len(), 2, "{r:#?}");
    // Part two has transcript + OCR agreeing: it wins over the single
    // transcript hit in part one even though part one's text is also a match.
    let top = &r.hits[0];
    assert_eq!(top.video_id, vid);
    assert_eq!(top.segment_title.as_deref(), Some("Part two"));
    assert_eq!(top.t0, Timestamp::from_secs(1800));
    assert_eq!(top.evidence.len(), 2);
    assert!(top.evidence.iter().any(|e| e.kind == Kind::Ocr));
    assert!(top.thumbnail.as_deref().unwrap().starts_with("blob:"));
    assert_eq!(r.hits[1].segment_title.as_deref(), Some("Part one"));
    assert!(r.hits[1].thumbnail.is_none());
    assert!(r.hits[0].score > r.hits[1].score);

    let only_first = search(
        &idx,
        None,
        &SearchRequest {
            kinds: vec![Kind::Transcript],
            ..SearchRequest::new("lunch atrium", 10)
        },
    )
    .await
    .unwrap();
    assert_eq!(only_first.hits.len(), 1);
    assert_eq!(
        only_first.hits[0].evidence[0].t0,
        Timestamp::from_secs(1900)
    );

    let none = search(
        &idx,
        None,
        &SearchRequest::new("quantum chromodynamics", 10),
    )
    .await
    .unwrap();
    assert!(none.hits.is_empty());
    assert_eq!(none.candidates, 0);

    let other_video = search(
        &idx,
        None,
        &SearchRequest {
            videos: vec![VideoId::new()],
            ..SearchRequest::new("retrieval", 10)
        },
    )
    .await
    .unwrap();
    assert!(other_video.hits.is_empty());
}

#[tokio::test]
async fn falls_back_to_windows_without_chapters() {
    let dir = tempfile::tempdir().unwrap();
    let idx = EmbeddedIndex::create(&dir.path().join("q.vidx")).unwrap();
    let (vid, _) = seed(&idx).await;
    idx.delete_segments(vid, SegmentLevel::Chapter)
        .await
        .unwrap();
    let r = search(&idx, None, &SearchRequest::new("retriev", 10))
        .await
        .unwrap();
    assert!(r.hits.len() >= 2);
    assert!(r.hits.iter().all(|h| h.segment_id.is_none()));
    // The hit at 1840 s sits in the 60 s window starting at 1800.
    assert!(
        r.hits
            .iter()
            .any(|h| h.t0 == Timestamp::from_secs(1800) && h.t1 == Timestamp::from_secs(1860)),
        "{:?}",
        r.hits.iter().map(|h| (h.t0, h.t1)).collect::<Vec<_>>()
    );
    assert_eq!(r.grouping, "window");
}
