//! Where a video's media is (live C5): the file a batch video was indexed
//! from, or the live store a stream is being recorded into. `view`, `zoom`
//! and `describe` locate their pixels through here, so a live video decodes
//! from its recording once the worker reads segment feeds (live C2 part 2).

use std::path::PathBuf;

use vi_core::model::Video;
use vi_core::{Error, Result};
use vi_media::{MediaInput, SegmentFeed};

/// The decode source named by a video's stored probe: `probe["live"]["store"]`
/// for a stream (its segmented recording, read as a finished feed so a
/// window decode takes what is there and does not wait), else
/// `probe["path"]` for a file. `None` when the probe names neither.
pub fn media_input(video: &Video) -> Option<MediaInput> {
    fn text(v: Option<&serde_json::Value>) -> Option<&str> {
        v.and_then(|s| s.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
    if let Some(store) = text(video.probe.get("live").and_then(|l| l.get("store"))) {
        return Some(MediaInput::Segments(SegmentFeed::new(store)));
    }
    text(video.probe.get("path")).map(MediaInput::file)
}

/// The file whose presence says the media is on this machine: the media file
/// itself, or the `index.json` of a live store. The tools test this before
/// decoding, so a stream whose recording is present passes the same check as
/// a file.
pub fn media_path(video: &Video) -> Option<PathBuf> {
    match media_input(video)? {
        MediaInput::File { path } => Some(path),
        MediaInput::Segments(feed) => Some(feed.index_path()),
    }
}

/// The file a window decode opens. Decode requests name a file today; a
/// segmented recording needs the worker's feed decode from live C2 part 2,
/// so until that lands a stream's media is located but not yet decodable
/// here, and the error says so.
pub fn decode_path(video: &Video) -> Result<PathBuf> {
    match media_input(video) {
        Some(MediaInput::File { path }) if path.is_file() => Ok(path),
        Some(MediaInput::Segments(feed)) if feed.index_path().is_file() => {
            Err(Error::Unsupported(format!(
                "window decode over the live store {} needs the segment-feed decode (live C2 part 2)",
                feed.dir.display()
            )))
        }
        _ => Err(Error::NotFound(format!(
            "media file for video {} is not on this machine",
            video.id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use vi_core::model::IndexState;
    use vi_core::{Timestamp, VideoId};

    fn video(probe: serde_json::Value) -> Video {
        Video {
            id: VideoId::new(),
            source_uri: "x".into(),
            content_hash: "h".into(),
            title: None,
            description: None,
            channel: None,
            published_at: None,
            duration: Timestamp::from_secs(10),
            start_wallclock: None,
            probe,
            index_state: IndexState::Coarse,
            created_at: Utc::now(),
            watermark: None,
            live_ended_at: None,
        }
    }

    #[test]
    fn file_and_store_probes_resolve() {
        let f = video(serde_json::json!({"path": "/tmp/a.mp4"}));
        assert_eq!(media_input(&f), Some(MediaInput::file("/tmp/a.mp4")));
        assert_eq!(media_path(&f), Some(PathBuf::from("/tmp/a.mp4")));
        let s = video(
            serde_json::json!({"path": "", "live": {"source": "youtube:abc", "store": "/data/live/v1"}}),
        );
        assert_eq!(
            media_input(&s),
            Some(MediaInput::Segments(SegmentFeed::new("/data/live/v1")))
        );
        assert_eq!(
            media_path(&s),
            Some(PathBuf::from("/data/live/v1/index.json"))
        );
        assert_eq!(media_input(&video(serde_json::json!({}))), None);
        assert!(matches!(
            decode_path(&video(serde_json::json!({}))),
            Err(Error::NotFound(_))
        ));
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.json"), "{}").unwrap();
        let live = video(serde_json::json!({"live": {"store": dir.path()}}));
        assert!(media_path(&live).unwrap().is_file());
        assert!(matches!(decode_path(&live), Err(Error::Unsupported(_))));
    }
}
