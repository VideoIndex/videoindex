//! Length-prefixed message protocol between the parent and the decode worker.
//!
//! Every message is a 4-byte little-endian length followed by that many bytes
//! of JSON. Control traffic is small, so JSON is fine; pixels and PCM never go
//! through the pipe, they go through [`crate::shm`] slots.

use std::io::{Read, Write};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use vi_core::Timestamp;

use crate::error::{MediaError, Result};
use crate::frame::PixelFormat;
use crate::probe::Probe;
use crate::segments::MediaInput;
use crate::shm::ShmSpec;

/// Protocol version; both sides must agree. Version 3 added the live decode
/// session (`DecodeLive`, `Tick`, `Gap`) and made every decode request name a
/// [`MediaInput`]. Parent and worker are always the same build, so there is
/// no compatibility shim.
pub const PROTOCOL_VERSION: u32 = 3;

/// Largest message either side will accept.
pub const MAX_MESSAGE_BYTES: u32 = 64 * 1024 * 1024;

/// A request to decode video frames at a fixed rate over a range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoDecodeRequest {
    /// What to decode: a file, or a segment feed. Over a feed the range is
    /// on the recording's timeline (the segment index's `t0`/`t1`) and only
    /// the segments covering it are opened.
    pub input: MediaInput,
    /// Stream index; `None` picks the best video stream.
    pub stream_index: Option<u32>,
    /// Frames per second to sample.
    pub fps: f64,
    /// Start time in seconds.
    pub t0_secs: f64,
    /// End time in seconds; `None` means end of stream.
    pub t1_secs: Option<f64>,
    /// Longest side of delivered frames. 0 keeps the source size.
    pub max_dim: u32,
    /// Pixel format to deliver.
    pub format: PixelFormat,
    /// Decoder threads; 0 means all CPUs.
    pub threads: usize,
}

impl VideoDecodeRequest {
    /// Sample the whole input at `fps`, scaled to `max_dim`. Takes a path
    /// (file) or anything else that converts into a [`MediaInput`].
    pub fn new(input: impl Into<MediaInput>, fps: f64, max_dim: u32) -> Self {
        Self {
            input: input.into(),
            stream_index: None,
            fps,
            t0_secs: 0.0,
            t1_secs: None,
            max_dim,
            format: PixelFormat::Rgb24,
            threads: 0,
        }
    }

    /// Restrict to `[t0, t1)` seconds.
    pub fn range(mut self, t0_secs: f64, t1_secs: Option<f64>) -> Self {
        self.t0_secs = t0_secs;
        self.t1_secs = t1_secs;
        self
    }

    /// Validate.
    pub fn check(&self) -> Result<()> {
        if self.fps <= 0.0 || !self.fps.is_finite() {
            return Err(MediaError::Invalid(format!(
                "fps must be > 0, got {}",
                self.fps
            )));
        }
        if self.t0_secs < 0.0 {
            return Err(MediaError::Invalid("t0 must be >= 0".into()));
        }
        if let Some(t1) = self.t1_secs {
            if t1 <= self.t0_secs {
                return Err(MediaError::Invalid("t1 must be > t0".into()));
            }
        }
        Ok(())
    }
}

/// A request to decode audio to 16 kHz mono PCM chunks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioDecodeRequest {
    /// What to decode: a file, or a segment feed (range on the recording's
    /// timeline, covering segments only, as for video).
    pub input: MediaInput,
    /// Stream index; `None` picks the best audio stream.
    pub stream_index: Option<u32>,
    /// Start time in seconds.
    pub t0_secs: f64,
    /// End time in seconds; `None` means end of stream.
    pub t1_secs: Option<f64>,
    /// Output sample rate.
    pub sample_rate: u32,
    /// Chunk length in seconds.
    pub chunk_secs: f64,
    /// Overlap between consecutive chunks in seconds.
    pub overlap_secs: f64,
}

impl AudioDecodeRequest {
    /// Whole input, 16 kHz, 30 s chunks with 1 s overlap.
    pub fn new(input: impl Into<MediaInput>) -> Self {
        Self {
            input: input.into(),
            stream_index: None,
            t0_secs: 0.0,
            t1_secs: None,
            sample_rate: 16_000,
            chunk_secs: 30.0,
            overlap_secs: 1.0,
        }
    }

    /// Samples per chunk.
    pub fn chunk_samples(&self) -> usize {
        (self.chunk_secs * f64::from(self.sample_rate)).round() as usize
    }

    /// Samples of overlap.
    pub fn overlap_samples(&self) -> usize {
        (self.overlap_secs * f64::from(self.sample_rate)).round() as usize
    }

    /// Validate.
    pub fn check(&self) -> Result<()> {
        if self.sample_rate == 0 {
            return Err(MediaError::Invalid("sample_rate must be > 0".into()));
        }
        if self.chunk_secs <= 0.0 || !self.chunk_secs.is_finite() {
            return Err(MediaError::Invalid("chunk_secs must be > 0".into()));
        }
        if self.overlap_secs < 0.0 || self.overlap_secs >= self.chunk_secs {
            return Err(MediaError::Invalid(
                "overlap must be in [0, chunk_secs)".into(),
            ));
        }
        if self.t0_secs < 0.0 {
            return Err(MediaError::Invalid("t0 must be >= 0".into()));
        }
        Ok(())
    }
}

/// A request to decode a growing recording once, in a single pass that
/// yields frames, audio chunks, ticks and gaps on the recording's timeline
/// (live C2). See [`crate::client::decode_live`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveDecodeRequest {
    /// The segment feed to decode; `follow` decides whether the worker waits
    /// at the index for more segments.
    pub input: MediaInput,
    /// Frames per second to sample.
    pub fps: f64,
    /// Longest side of delivered frames. 0 keeps the source size.
    pub max_dim: u32,
    /// Output sample rate of the audio chunks.
    pub sample_rate: u32,
    /// Audio chunk length in seconds (no overlap: the chunks are a stream).
    pub chunk_secs: f64,
    /// Emit a `Tick` every this many seconds of stream time.
    pub tick_secs: f64,
    /// Decoder threads; 0 means the worker's default.
    pub threads: usize,
}

impl LiveDecodeRequest {
    /// Frames at `fps` scaled to `max_dim`, 16 kHz audio in 1 s chunks, a
    /// tick every 2 s of stream time.
    pub fn new(input: impl Into<MediaInput>, fps: f64, max_dim: u32) -> Self {
        Self {
            input: input.into(),
            fps,
            max_dim,
            sample_rate: 16_000,
            chunk_secs: 1.0,
            tick_secs: 2.0,
            threads: 0,
        }
    }

    /// Samples per audio chunk.
    pub fn chunk_samples(&self) -> usize {
        (self.chunk_secs * f64::from(self.sample_rate)).round() as usize
    }

    /// Validate.
    pub fn check(&self) -> Result<()> {
        if self.input.as_segments().is_none() {
            return Err(MediaError::Invalid(
                "decode_live needs a segment feed as its input".into(),
            ));
        }
        if self.fps <= 0.0 || !self.fps.is_finite() {
            return Err(MediaError::Invalid(format!(
                "fps must be > 0, got {}",
                self.fps
            )));
        }
        if self.sample_rate == 0 {
            return Err(MediaError::Invalid("sample_rate must be > 0".into()));
        }
        if self.chunk_secs <= 0.0 || !self.chunk_secs.is_finite() {
            return Err(MediaError::Invalid("chunk_secs must be > 0".into()));
        }
        if self.tick_secs <= 0.0 || !self.tick_secs.is_finite() {
            return Err(MediaError::Invalid("tick_secs must be > 0".into()));
        }
        Ok(())
    }
}

/// Parent to worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Handshake.
    Hello {
        /// Protocol version of the parent.
        version: u32,
    },
    /// Probe a file or a segment feed.
    Probe {
        /// What to probe.
        input: MediaInput,
    },
    /// Decode video frames into shared memory.
    DecodeVideo {
        /// What to decode.
        req: VideoDecodeRequest,
        /// Where frames go.
        shm: ShmSpec,
    },
    /// Decode audio chunks into shared memory.
    DecodeAudio {
        /// What to decode.
        req: AudioDecodeRequest,
        /// Where chunks go.
        shm: ShmSpec,
    },
    /// Decode a growing recording in one pass: frames and chunks into
    /// shared memory, ticks and gaps as messages.
    DecodeLive {
        /// What to decode.
        req: LiveDecodeRequest,
        /// Where frames and chunks go.
        shm: ShmSpec,
    },
    /// The parent finished with a slot.
    SlotFree {
        /// Slot index.
        slot: u32,
    },
    /// Stop the current decode.
    Cancel,
    /// Exit.
    Shutdown,
}

/// Worker to parent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    /// Handshake reply.
    Hello {
        /// Protocol version of the worker.
        version: u32,
        /// libav version string.
        libav: String,
        /// Worker pid.
        pid: u32,
    },
    /// Probe result.
    Probe(Probe),
    /// A decode session opened; geometry of what will follow.
    Started {
        /// Timebase numerator of the decoded stream.
        time_base_num: u32,
        /// Timebase denominator.
        time_base_den: u32,
        /// Stream index used.
        stream_index: u32,
        /// Delivered frame width (video).
        width: u32,
        /// Delivered frame height (video).
        height: u32,
        /// Source width (video).
        source_width: u32,
        /// Source height (video).
        source_height: u32,
        /// Whether per-sample seeking is in use (video).
        seeking: bool,
    },
    /// One frame is in a slot.
    Frame {
        /// Slot index.
        slot: u32,
        /// Valid bytes.
        len: usize,
        /// Width.
        width: u32,
        /// Height.
        height: u32,
        /// Row stride.
        stride: usize,
        /// Pixel format.
        format: PixelFormat,
        /// PTS in the stream timebase.
        pts: i64,
        /// Time in seconds-rational.
        t: Timestamp,
        /// Keyframe flag.
        is_keyframe: bool,
    },
    /// One PCM chunk is in a slot.
    AudioChunk {
        /// Slot index.
        slot: u32,
        /// Valid bytes (`samples * 2`).
        len: usize,
        /// Start time.
        t0: Timestamp,
        /// End time.
        t1: Timestamp,
        /// Sample rate.
        sample_rate: u32,
        /// Samples in the chunk.
        samples: usize,
    },
    /// Live sessions: everything up to `head` on the recording's timeline
    /// has been delivered.
    Tick {
        /// Stream time decoded so far; never past the last listed segment's
        /// end.
        head: Timestamp,
    },
    /// Live sessions: the recording has no media for `[t0, t1)`; times after
    /// it skip the hole.
    Gap {
        /// Start of the hole.
        t0: Timestamp,
        /// End of the hole.
        t1: Timestamp,
    },
    /// The session finished.
    End {
        /// Items delivered.
        items: u64,
        /// Frames the decoder produced (video) or samples decoded (audio).
        decoded: u64,
    },
    /// Something failed; the session is over.
    Error {
        /// Message.
        message: String,
    },
    /// Diagnostic.
    Log {
        /// `debug`, `info`, `warn`.
        level: String,
        /// Text.
        message: String,
    },
}

/// Write one length-prefixed message (blocking).
pub fn write_msg<W: Write, T: Serialize>(w: &mut W, msg: &T) -> Result<()> {
    let body = serde_json::to_vec(msg)?;
    let len =
        u32::try_from(body.len()).map_err(|_| MediaError::Protocol("message too large".into()))?;
    if len > MAX_MESSAGE_BYTES {
        return Err(MediaError::Protocol("message too large".into()));
    }
    w.write_all(&len.to_le_bytes())?;
    w.write_all(&body)?;
    w.flush()?;
    Ok(())
}

/// Read one length-prefixed message (blocking). `Ok(None)` at clean EOF.
pub fn read_msg<R: Read, T: DeserializeOwned>(r: &mut R) -> Result<Option<T>> {
    let mut len_buf = [0u8; 4];
    match r.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_le_bytes(len_buf);
    if len > MAX_MESSAGE_BYTES {
        return Err(MediaError::Protocol(format!(
            "message of {len} bytes exceeds limit"
        )));
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}

/// Async variant of [`write_msg`].
pub async fn write_msg_async<W, T>(w: &mut W, msg: &T) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
    T: Serialize,
{
    use tokio::io::AsyncWriteExt;
    let body = serde_json::to_vec(msg)?;
    let len =
        u32::try_from(body.len()).map_err(|_| MediaError::Protocol("message too large".into()))?;
    if len > MAX_MESSAGE_BYTES {
        return Err(MediaError::Protocol("message too large".into()));
    }
    w.write_all(&len.to_le_bytes()).await?;
    w.write_all(&body).await?;
    w.flush().await?;
    Ok(())
}

/// Async variant of [`read_msg`].
pub async fn read_msg_async<R, T>(r: &mut R) -> Result<Option<T>>
where
    R: tokio::io::AsyncRead + Unpin,
    T: DeserializeOwned,
{
    use tokio::io::AsyncReadExt;
    let mut len_buf = [0u8; 4];
    match r.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_le_bytes(len_buf);
    if len > MAX_MESSAGE_BYTES {
        return Err(MediaError::Protocol(format!(
            "message of {len} bytes exceeds limit"
        )));
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body).await?;
    Ok(Some(serde_json::from_slice(&body)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_over_a_buffer() {
        let mut buf = Vec::new();
        write_msg(&mut buf, &Request::Hello { version: 1 }).unwrap();
        write_msg(&mut buf, &Request::SlotFree { slot: 7 }).unwrap();
        let mut cur = std::io::Cursor::new(buf);
        let a: Request = read_msg(&mut cur).unwrap().unwrap();
        let b: Request = read_msg(&mut cur).unwrap().unwrap();
        let c: Option<Request> = read_msg(&mut cur).unwrap();
        assert_eq!(a, Request::Hello { version: 1 });
        assert_eq!(b, Request::SlotFree { slot: 7 });
        assert!(c.is_none());
    }

    #[test]
    fn rejects_oversized_length() {
        let mut buf = (MAX_MESSAGE_BYTES + 1).to_le_bytes().to_vec();
        buf.extend_from_slice(&[0; 8]);
        let mut cur = std::io::Cursor::new(buf);
        let r: Result<Option<Request>> = read_msg(&mut cur);
        assert!(matches!(r, Err(MediaError::Protocol(_))));
    }

    #[test]
    fn request_validation() {
        assert!(VideoDecodeRequest::new("x.mp4", 0.0, 640).check().is_err());
        assert!(VideoDecodeRequest::new("x.mp4", 1.0, 640)
            .range(5.0, Some(4.0))
            .check()
            .is_err());
        assert!(VideoDecodeRequest::new("x.mp4", 1.0, 640).check().is_ok());
        let mut a = AudioDecodeRequest::new("x.mp4");
        assert_eq!(a.chunk_samples(), 480_000);
        a.overlap_secs = 30.0;
        assert!(a.check().is_err());
        assert_eq!(
            VideoDecodeRequest::new("x.mp4", 1.0, 640).input,
            MediaInput::from("x.mp4")
        );

        let feed = crate::segments::SegmentFeed::following("/store");
        let live = LiveDecodeRequest::new(feed.clone(), 1.0, 320);
        assert!(live.check().is_ok());
        assert_eq!(live.chunk_samples(), 16_000);
        assert!(LiveDecodeRequest::new("x.mp4", 1.0, 320).check().is_err());
        assert!(LiveDecodeRequest::new(feed.clone(), 0.0, 320)
            .check()
            .is_err());
        let mut bad = LiveDecodeRequest::new(feed.clone(), 1.0, 320);
        bad.tick_secs = 0.0;
        assert!(bad.check().is_err());
        let mut bad = LiveDecodeRequest::new(feed, 1.0, 320);
        bad.chunk_secs = f64::NAN;
        assert!(bad.check().is_err());
    }
}
