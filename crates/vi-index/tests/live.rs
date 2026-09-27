//! Storage semantics the live modules lean on (live C4): time-filtered
//! searches, live progress, and the since-feeds. Hand-built index, no
//! media.

#![allow(clippy::unwrap_used)]

use chrono::Utc;
use vi_core::model::*;
use vi_core::*;
use vi_index::{EmbeddedIndex, Hit, Kind, MentionQuery, Storage, TextQuery, VectorQuery};

fn video(hash: &str, state: IndexState) -> Video {
    Video {
        id: VideoId::new(),
        source_uri: format!("https://example/{hash}"),
        content_hash: hash.into(),
        title: Some(format!("Talk {hash}")),
        description: None,
        channel: None,
        published_at: None,
        duration: Timestamp::from_secs(100),
        start_wallclock: None,
        probe: serde_json::json!({}),
        index_state: state,
        created_at: Utc::now(),
        watermark: None,
        live_ended_at: None,
    }
}

fn track(video_id: VideoId, kind: TrackKind) -> Track {
    Track {
        id: TrackId::new(),
        video_id,
        kind,
        stream_index: match kind {
            TrackKind::Video => 0,
            TrackKind::Audio => 1,
            TrackKind::Subtitle => 2,
        },
        codec: "x".into(),
        timebase_num: 1,
        timebase_den: 1000,
        width: None,
        height: None,
        fps: None,
        sample_rate: None,
        channels: None,
        language: None,
    }
}

fn span(track: TrackId, prov: ProvenanceId, t0: i64, t1: i64, text: &str) -> TranscriptSpan {
    TranscriptSpan {
        id: SpanId::new(),
        track_id: track,
        t0: Timestamp::from_secs(t0),
        t1: Timestamp::from_secs(t1),
        text: text.into(),
        speaker: None,
        language: Some("en".into()),
        confidence: Some(1.0),
        words: None,
        provenance_id: prov,
    }
}

fn frame(track: TrackId, t: i64) -> FrameSample {
    FrameSample {
        id: FrameSampleId::new(),
        track_id: track,
        t: Timestamp::from_secs(t),
        pts: t * 1000,
        is_keyframe: true,
        phash: None,
        thumbnail_blob: None,
        width: 640,
        height: 360,
    }
}

fn ocr(frame: &FrameSample, prov: ProvenanceId, text: &str) -> OcrSpan {
    OcrSpan {
        id: SpanId::new(),
        frame_sample_id: frame.id,
        t: frame.t,
        text: text.into(),
        bbox: None,
        confidence: Some(0.9),
        provenance_id: prov,
    }
}

fn segment(
    video: VideoId,
    prov: ProvenanceId,
    level: SegmentLevel,
    t0: i64,
    t1: i64,
    title: &str,
) -> Segment {
    Segment {
        id: SegmentId::new(),
        video_id: video,
        level,
        parent_id: None,
        t0: Timestamp::from_secs(t0),
        t1: Timestamp::from_secs(t1),
        keyframe_sample_id: None,
        title: Some(title.into()),
        summary: None,
        provenance_id: prov,
    }
}

/// One video whose every row mentions retrieval: transcript spans starting
/// at 0, 30, 60 and 90 s (different lengths, so BM25 ranks them apart), OCR
/// lines on frames at 20 and 70 s, descriptions on segments [0, 50) and
/// [50, 100), and a text-vector embedding per transcript span whose
/// similarity to the query `[1, 0]` falls with time.
async fn seed_searchable(idx: &EmbeddedIndex) -> (VideoId, Vec<TranscriptSpan>) {
    let v = video("search", IndexState::Coarse);
    idx.put_video(&v).await.unwrap();
    let audio = track(v.id, TrackKind::Audio);
    let vt = track(v.id, TrackKind::Video);
    idx.put_tracks(&[audio.clone(), vt.clone()]).await.unwrap();
    let prov = Provenance::local("test", 1, serde_json::json!({}));
    idx.put_provenance(&prov).await.unwrap();
    let spans = vec![
        span(
            audio.id,
            prov.id,
            0,
            10,
            "welcome to the workshop on retrieval",
        ),
        span(
            audio.id,
            prov.id,
            30,
            40,
            "retrieval returns candidate segments for the agent to read",
        ),
        span(audio.id, prov.id, 60, 70, "hybrid retrieval"),
        span(
            audio.id,
            prov.id,
            90,
            100,
            "more retrieval talk after the break with questions from the audience",
        ),
    ];
    idx.put_spans(
        &spans
            .iter()
            .cloned()
            .map(Span::Transcript)
            .collect::<Vec<_>>(),
    )
    .await
    .unwrap();
    let f20 = frame(vt.id, 20);
    let f70 = frame(vt.id, 70);
    idx.put_frame_samples(&[f20.clone(), f70.clone()])
        .await
        .unwrap();
    idx.put_spans(&[
        Span::Ocr(ocr(&f20, prov.id, "Retrieval: BM25 + dense")),
        Span::Ocr(ocr(
            &f70,
            prov.id,
            "Retrieval evaluation results on the benchmark suite",
        )),
    ])
    .await
    .unwrap();
    let s0 = segment(v.id, prov.id, SegmentLevel::Scene, 0, 50, "Intro");
    let s1 = segment(v.id, prov.id, SegmentLevel::Scene, 50, 100, "Results");
    idx.put_segments(&[s0.clone(), s1.clone()]).await.unwrap();
    idx.put_descriptions(&[
        Description {
            id: DescriptionId::new(),
            target_kind: TargetKind::Segment,
            target_id: s0.id.to_string(),
            kind: DescriptionKind::Caption,
            text: "Speaker introduces retrieval on a slide".into(),
            structured: None,
            provenance_id: prov.id,
        },
        Description {
            id: DescriptionId::new(),
            target_kind: TargetKind::Segment,
            target_id: s1.id.to_string(),
            kind: DescriptionKind::Caption,
            text: "Speaker shows retrieval evaluation charts and tables side by side".into(),
            structured: None,
            provenance_id: prov.id,
        },
    ])
    .await
    .unwrap();
    let vectors: [[f32; 2]; 4] = [[1.0, 0.0], [0.9, 0.436], [0.6, 0.8], [0.3, 0.954]];
    idx.put_embeddings(
        &spans
            .iter()
            .zip(vectors)
            .map(|(s, vec)| Embedding {
                id: EmbeddingId::new(),
                target_kind: TargetKind::TranscriptSpan,
                target_id: s.id.to_string(),
                model: "m".into(),
                dim: 2,
                vector: vec.to_vec(),
                provenance_id: prov.id,
            })
            .collect::<Vec<_>>(),
    )
    .await
    .unwrap();
    (v.id, spans)
}

fn before(until: i64) -> Option<TimeRange> {
    TimeRange::new(Timestamp::ZERO, Timestamp::from_secs(until))
}

fn ids_and_scores(hits: &[Hit]) -> Vec<(String, f64)> {
    hits.iter().map(|h| (h.id.clone(), h.score)).collect()
}

#[tokio::test]
async fn time_filtered_searches_return_only_rows_before_until_with_the_same_ranking() {
    let dir = tempfile::tempdir().unwrap();
    let idx = EmbeddedIndex::create(&dir.path().join("l.vidx")).unwrap();
    let (vid, spans) = seed_searchable(&idx).await;
    let until = Timestamp::from_secs(60);

    // Text: every kind, unbounded then bounded.
    let all = idx
        .text_search(&TextQuery::new("retrieval", 20))
        .await
        .unwrap();
    assert_eq!(all.len(), 8, "{all:#?}");
    let bounded = idx
        .text_search(&TextQuery {
            time_range: before(60),
            ..TextQuery::new("retrieval", 20)
        })
        .await
        .unwrap();
    assert!(
        bounded.iter().all(|h| h.t0 < until),
        "no row at or after until: {bounded:#?}"
    );
    // The span starting exactly at 60 s is out; the description of the
    // segment starting at 50 s is in: two spans, one OCR line, two
    // descriptions.
    assert_eq!(bounded.len(), 5, "{bounded:#?}");
    assert!(bounded.iter().any(|h| h.kind == Kind::Ocr));
    assert!(bounded.iter().any(|h| h.kind == Kind::Description));
    let expected: Vec<Hit> = all.iter().filter(|h| h.t0 < until).cloned().collect();
    assert_eq!(
        ids_and_scores(&bounded),
        ids_and_scores(&expected),
        "same order and scores as the unbounded search restricted to those rows"
    );
    // One kind at a time keeps the same property.
    for kind in [Kind::Transcript, Kind::Ocr, Kind::Description] {
        let all_k = idx
            .text_search(&TextQuery {
                kinds: vec![kind],
                ..TextQuery::new("retrieval", 20)
            })
            .await
            .unwrap();
        let bounded_k = idx
            .text_search(&TextQuery {
                kinds: vec![kind],
                time_range: before(60),
                ..TextQuery::new("retrieval", 20)
            })
            .await
            .unwrap();
        let expected: Vec<Hit> = all_k.iter().filter(|h| h.t0 < until).cloned().collect();
        assert_eq!(
            ids_and_scores(&bounded_k),
            ids_and_scores(&expected),
            "{kind:?}"
        );
    }
    // An empty range matches nothing and does not fail.
    let none = idx
        .text_search(&TextQuery {
            time_range: Some(TimeRange {
                t0: Timestamp::ZERO,
                t1: Timestamp::ZERO,
            }),
            ..TextQuery::new("retrieval", 20)
        })
        .await
        .unwrap();
    assert!(none.is_empty());

    // Mentions: counts and samples both respect the range.
    let q = MentionQuery {
        terms: vec!["retrieval".into()],
        videos: vec![],
        kinds: vec![],
        prefix: false,
        samples_per_video: 10,
        time_range: None,
    };
    let all_m = idx.find_mentions(&q).await.unwrap();
    assert_eq!(all_m.len(), 1);
    assert_eq!(all_m[0].total(), 8, "{all_m:#?}");
    let bounded_m = idx
        .find_mentions(&MentionQuery {
            time_range: before(60),
            ..q.clone()
        })
        .await
        .unwrap();
    assert_eq!(bounded_m.len(), 1);
    let m = &bounded_m[0];
    assert_eq!(m.video_id, vid);
    let count = |v: &vi_index::VideoMentions, kind: Kind| -> u64 {
        v.counts
            .iter()
            .filter(|c| c.kind == kind)
            .map(|c| c.count)
            .sum()
    };
    assert_eq!(count(m, Kind::Transcript), 2, "{m:#?}");
    assert_eq!(count(m, Kind::Ocr), 1, "{m:#?}");
    assert_eq!(count(m, Kind::Description), 2, "{m:#?}");
    assert_eq!(m.total(), 5);
    assert!(m.samples.iter().all(|s| s.t0 < until), "{m:#?}");
    let expected: Vec<_> = all_m[0]
        .samples
        .iter()
        .filter(|s| s.t0 < until)
        .cloned()
        .collect();
    assert_eq!(m.samples, expected);

    // Vectors: the nearest rows in [0, 60) in the unbounded order.
    let vq = VectorQuery {
        model: "m".into(),
        vector: vec![1.0, 0.0],
        videos: vec![],
        kinds: vec![],
        k: 10,
        time_range: None,
    };
    let all_v = idx.vector_search(&vq).await.unwrap();
    assert_eq!(
        all_v.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
        spans.iter().map(|s| s.id.to_string()).collect::<Vec<_>>(),
        "similarity falls with time"
    );
    let bounded_v = idx
        .vector_search(&VectorQuery {
            time_range: before(60),
            ..vq.clone()
        })
        .await
        .unwrap();
    assert!(bounded_v.iter().all(|h| h.t0 < until), "{bounded_v:#?}");
    let expected: Vec<Hit> = all_v.iter().filter(|h| h.t0 < until).cloned().collect();
    assert_eq!(bounded_v, expected);
    assert_eq!(bounded_v.len(), 2);
    // `k` still caps the bounded result.
    let one = idx
        .vector_search(&VectorQuery {
            time_range: before(60),
            k: 1,
            ..vq.clone()
        })
        .await
        .unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].id, spans[0].id.to_string());
    // The over-fetch is three times `k`: a range that admits only the
    // fourth-nearest row is missed at `k = 1` (three rows fetched, none in
    // range; the documented shortfall until the sidecar carries a time
    // column) and found at `k = 2` (six fetched).
    let late = |k: usize| VectorQuery {
        time_range: TimeRange::new(Timestamp::from_secs(90), Timestamp::from_secs(100)),
        k,
        ..vq.clone()
    };
    assert!(idx.vector_search(&late(1)).await.unwrap().is_empty());
    let found = idx.vector_search(&late(2)).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, spans[3].id.to_string());
}

#[tokio::test]
async fn set_live_progress_roundtrips_and_live_videos_lists_the_live_ones() {
    let dir = tempfile::tempdir().unwrap();
    let idx = EmbeddedIndex::create(&dir.path().join("l.vidx")).unwrap();
    let batch = video("batch", IndexState::Coarse);
    let mut live = video("live", IndexState::Live);
    live.duration = Timestamp::ZERO;
    let mut live2 = video("live2", IndexState::Live);
    live2.duration = Timestamp::ZERO;
    for v in [&batch, &live, &live2] {
        idx.put_video(v).await.unwrap();
    }
    assert_eq!(
        idx.live_videos()
            .await
            .unwrap()
            .iter()
            .map(|v| v.id)
            .collect::<Vec<_>>(),
        vec![live.id, live2.id],
        "oldest first, batch videos absent"
    );

    // One tick: head 90 s, watermark 84 s, both in a 90 kHz timebase.
    let head = Timestamp::new(90 * 90_000, 90_000);
    let watermark = Timestamp::new(84 * 90_000, 90_000);
    idx.set_live_progress(live.id, head, watermark)
        .await
        .unwrap();
    let back = idx.get_video(live.id).await.unwrap().unwrap();
    assert_eq!(
        back.duration,
        Timestamp::from_secs(90),
        "head is the duration while live"
    );
    assert_eq!(back.duration.den, 90_000, "rational kept as written");
    assert_eq!(back.watermark, Some(Timestamp::from_secs(84)));
    assert_eq!(back.watermark.unwrap().den, 90_000);
    assert_eq!(back.index_state, IndexState::Live);
    assert_eq!(back.live_ended_at, None);
    assert_eq!(
        Video {
            duration: head,
            watermark: Some(watermark),
            ..live.clone()
        },
        back,
        "nothing else moved"
    );
    let stats = idx.stats().await.unwrap();
    let st = stats.videos.iter().find(|s| s.video.id == live.id).unwrap();
    assert_eq!(st.head, Some(Timestamp::from_secs(90)));
    assert_eq!(st.watermark, Some(Timestamp::from_secs(84)));
    // The next tick moves both forward.
    idx.set_live_progress(live.id, Timestamp::from_secs(92), Timestamp::from_secs(88))
        .await
        .unwrap();
    let back = idx.get_video(live.id).await.unwrap().unwrap();
    assert_eq!(
        (back.duration, back.watermark),
        (Timestamp::from_secs(92), Some(Timestamp::from_secs(88)))
    );
    // Other videos are untouched.
    assert_eq!(idx.get_video(batch.id).await.unwrap().unwrap(), batch);
    assert_eq!(idx.get_video(live2.id).await.unwrap().unwrap(), live2);

    // The stream ends: the video leaves the live list, its last head stays
    // as the duration.
    idx.set_index_state(live.id, IndexState::Coarse)
        .await
        .unwrap();
    assert_eq!(
        idx.live_videos()
            .await
            .unwrap()
            .iter()
            .map(|v| v.id)
            .collect::<Vec<_>>(),
        vec![live2.id]
    );
    assert_eq!(
        idx.get_video(live.id).await.unwrap().unwrap().duration,
        Timestamp::from_secs(92)
    );
    // Progress for a video that does not exist is a caller error.
    assert!(idx
        .set_live_progress(VideoId::new(), head, watermark)
        .await
        .is_err());
}

#[tokio::test]
async fn spans_since_returns_exactly_the_rows_written_after_the_marker() {
    let dir = tempfile::tempdir().unwrap();
    let idx = EmbeddedIndex::create(&dir.path().join("l.vidx")).unwrap();
    let v = video("feed", IndexState::Live);
    idx.put_video(&v).await.unwrap();
    let audio = track(v.id, TrackKind::Audio);
    let vt = track(v.id, TrackKind::Video);
    idx.put_tracks(&[audio.clone(), vt.clone()]).await.unwrap();
    let prov = Provenance::local("test", 1, serde_json::json!({}));
    idx.put_provenance(&prov).await.unwrap();

    // Batch 1: everything up to 30 s.
    let f5 = frame(vt.id, 5);
    let f25 = frame(vt.id, 25);
    idx.put_frame_samples(&[f5.clone(), f25.clone()])
        .await
        .unwrap();
    idx.put_spans(&[
        Span::Transcript(span(audio.id, prov.id, 0, 10, "one")),
        Span::Transcript(span(audio.id, prov.id, 10, 20, "two")),
        Span::Transcript(span(audio.id, prov.id, 20, 30, "three")),
        Span::Ocr(ocr(&f5, prov.id, "slide 1")),
        Span::Ocr(ocr(&f25, prov.id, "slide 2")),
    ])
    .await
    .unwrap();
    let marker = Timestamp::from_secs(30);
    assert_eq!(idx.spans_since(v.id, &[], marker).await.unwrap(), vec![]);
    assert_eq!(
        idx.spans_since(v.id, &[], Timestamp::ZERO)
            .await
            .unwrap()
            .len(),
        5,
        "from zero, everything"
    );

    // Batch 2: what the next tick committed, including an utterance that
    // straddles the marker (started before it, finished after).
    let f35 = frame(vt.id, 35);
    idx.put_frame_samples(std::slice::from_ref(&f35))
        .await
        .unwrap();
    let straddle = span(audio.id, prov.id, 28, 38, "straddles the marker");
    let four = span(audio.id, prov.id, 30, 40, "four");
    let five = span(audio.id, prov.id, 40, 50, "five");
    let slide3 = ocr(&f35, prov.id, "slide 3");
    idx.put_spans(&[
        Span::Transcript(four.clone()),
        Span::Ocr(slide3.clone()),
        Span::Transcript(straddle.clone()),
        Span::Transcript(five.clone()),
    ])
    .await
    .unwrap();

    let since = idx.spans_since(v.id, &[], marker).await.unwrap();
    assert_eq!(
        since,
        vec![
            Span::Transcript(straddle.clone()),
            Span::Transcript(four.clone()),
            Span::Ocr(slide3.clone()),
            Span::Transcript(five.clone()),
        ],
        "batch 2 only, in time order"
    );
    assert_eq!(
        idx.spans_since(v.id, &[Kind::Transcript], marker)
            .await
            .unwrap(),
        vec![
            Span::Transcript(straddle),
            Span::Transcript(four),
            Span::Transcript(five)
        ]
    );
    assert_eq!(
        idx.spans_since(v.id, &[Kind::Ocr], marker).await.unwrap(),
        vec![Span::Ocr(slide3)]
    );
    // Other kinds are ignored; a marker past everything returns nothing.
    assert!(idx
        .spans_since(v.id, &[Kind::Segment], marker)
        .await
        .unwrap()
        .is_empty());
    assert!(idx
        .spans_since(v.id, &[], Timestamp::from_secs(50))
        .await
        .unwrap()
        .is_empty());
    // Another video's rows never leak in.
    assert!(idx
        .spans_since(VideoId::new(), &[], Timestamp::ZERO)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn segments_since_returns_one_level_after_the_marker() {
    let dir = tempfile::tempdir().unwrap();
    let idx = EmbeddedIndex::create(&dir.path().join("l.vidx")).unwrap();
    let v = video("shots", IndexState::Live);
    idx.put_video(&v).await.unwrap();
    let prov = Provenance::local("shot_boundary", 1, serde_json::json!({}));
    idx.put_provenance(&prov).await.unwrap();

    // Batch 1: two closed shots and a chapter over them.
    idx.put_segments(&[
        segment(v.id, prov.id, SegmentLevel::Shot, 0, 10, "s1"),
        segment(v.id, prov.id, SegmentLevel::Shot, 10, 20, "s2"),
        segment(v.id, prov.id, SegmentLevel::Chapter, 0, 20, "c1"),
    ])
    .await
    .unwrap();
    let marker = Timestamp::from_secs(20);
    assert!(idx
        .segments_since(v.id, SegmentLevel::Shot, marker)
        .await
        .unwrap()
        .is_empty());

    // Batch 2: a closed shot, the open shot at the head, and a chapter.
    let s3 = segment(v.id, prov.id, SegmentLevel::Shot, 20, 30, "s3");
    let mut open = segment(v.id, prov.id, SegmentLevel::Shot, 30, 45, "open");
    let c2 = segment(v.id, prov.id, SegmentLevel::Chapter, 20, 45, "c2");
    idx.put_segments(&[open.clone(), c2.clone(), s3.clone()])
        .await
        .unwrap();
    assert_eq!(
        idx.segments_since(v.id, SegmentLevel::Shot, marker)
            .await
            .unwrap(),
        vec![s3.clone(), open.clone()],
        "batch 2 shots only, by time"
    );
    assert_eq!(
        idx.segments_since(v.id, SegmentLevel::Chapter, marker)
            .await
            .unwrap(),
        vec![c2]
    );
    assert!(idx
        .segments_since(v.id, SegmentLevel::Scene, marker)
        .await
        .unwrap()
        .is_empty());

    // The next tick extends the open shot in place: a marker at its old
    // end still sees it, and only it.
    open.t1 = Timestamp::from_secs(60);
    idx.put_segments(std::slice::from_ref(&open)).await.unwrap();
    assert_eq!(
        idx.segments_since(v.id, SegmentLevel::Shot, Timestamp::from_secs(45))
            .await
            .unwrap(),
        vec![open.clone()]
    );
    assert_eq!(
        idx.segments(v.id, SegmentLevel::Shot).await.unwrap().len(),
        4,
        "upsert, not a new row"
    );
}
