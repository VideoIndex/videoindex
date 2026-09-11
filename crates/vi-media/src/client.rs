//! Parent-side API. Each call spawns one worker process for one request:
//! simple, crash-isolated per request, and cheap (a few milliseconds).
//! Pooling can come later without changing this API.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;
use tracing::{debug, warn};
use vi_core::config::WorkerConfig;
use vi_core::Timestamp;

use crate::error::{MediaError, Result};
use crate::frame::{FrameBuffer, FrameMeta, PixelFormat};
use crate::probe::Probe;
use crate::protocol::{
    read_msg_async, write_msg_async, AudioDecodeRequest, Request, Response, VideoDecodeRequest,
    PROTOCOL_VERSION,
};
use crate::sandbox;
use crate::shm::{SharedRegion, SlotGuard};
use crate::WORKER_ARG;

/// Environment variable naming an explicit worker executable.
pub const WORKER_PATH_ENV: &str = "VI_MEDIA_WORKER";

/// Resolve which executable to run as the worker.
pub fn worker_executable(cfg: &WorkerConfig) -> Result<PathBuf> {
    if let Some(p) = &cfg.path {
        return Ok(p.clone());
    }
    if let Some(p) = std::env::var_os(WORKER_PATH_ENV) {
        return Ok(PathBuf::from(p));
    }
    Ok(std::env::current_exe()?)
}

struct Worker {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    idle_timeout: Duration,
    /// libav version reported by the worker.
    pub libav: String,
}

impl Worker {
    async fn spawn(cfg: &WorkerConfig) -> Result<Self> {
        let exe = worker_executable(cfg)?;
        let mut cmd = std::process::Command::new(&exe);
        cmd.arg(WORKER_ARG)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        sandbox::apply(
            &mut cmd,
            sandbox::Limits {
                memory_bytes: cfg.memory_limit_mb * 1024 * 1024,
                ..sandbox::Limits::default()
            },
        );
        let mut cmd = Command::from(cmd);
        cmd.kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| {
            MediaError::WorkerExited(format!("failed to spawn {}: {e}", exe.display()))
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| MediaError::WorkerExited("no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| MediaError::WorkerExited("no stdout".into()))?;
        let mut w = Self {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
            idle_timeout: Duration::from_secs(cfg.timeout_secs.max(1)),
            libav: String::new(),
        };
        w.send(&Request::Hello {
            version: PROTOCOL_VERSION,
        })
        .await?;
        match w.recv().await? {
            Response::Hello { version, libav, .. } if version == PROTOCOL_VERSION => {
                w.libav = libav;
                Ok(w)
            }
            Response::Hello { version, .. } => Err(MediaError::Protocol(format!(
                "worker protocol {version}, expected {PROTOCOL_VERSION}"
            ))),
            Response::Error { message } => Err(MediaError::Worker(message)),
            other => Err(MediaError::Protocol(format!(
                "unexpected handshake reply {other:?}"
            ))),
        }
    }

    async fn send(&mut self, req: &Request) -> Result<()> {
        write_msg_async(&mut self.stdin, req).await
    }

    async fn recv(&mut self) -> Result<Response> {
        let fut = read_msg_async::<_, Response>(&mut self.stdout);
        match tokio::time::timeout(self.idle_timeout, fut).await {
            Ok(Ok(Some(r))) => Ok(r),
            Ok(Ok(None)) => Err(MediaError::WorkerExited(self.exit_description().await)),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                let _ = self.child.start_kill();
                Err(MediaError::Timeout(self.idle_timeout.as_secs()))
            }
        }
    }

    async fn exit_description(&mut self) -> String {
        match tokio::time::timeout(Duration::from_secs(2), self.child.wait()).await {
            Ok(Ok(status)) => format!("exit status {status}"),
            Ok(Err(e)) => format!("wait failed: {e}"),
            Err(_) => "still running after closing its stdout".into(),
        }
    }

    async fn shutdown(mut self) {
        let _ = self.send(&Request::Shutdown).await;
        let _ = self.stdin.shutdown().await;
        if tokio::time::timeout(Duration::from_secs(5), self.child.wait())
            .await
            .is_err()
        {
            let _ = self.child.start_kill();
        }
    }
}

/// Probe a file in a worker process.
pub async fn probe(cfg: &WorkerConfig, path: &Path) -> Result<Probe> {
    let mut w = Worker::spawn(cfg).await?;
    w.send(&Request::Probe {
        path: path.to_path_buf(),
    })
    .await?;
    let result = loop {
        match w.recv().await {
            Ok(Response::Probe(p)) => break Ok(p),
            Ok(Response::Error { message }) => break Err(MediaError::Worker(message)),
            Ok(Response::Log { level, message }) => debug!(level, "{message}"),
            Ok(other) => {
                break Err(MediaError::Protocol(format!(
                    "unexpected reply to probe: {other:?}"
                )))
            }
            Err(e) => break Err(e),
        }
    };
    w.shutdown().await;
    result
}

/// What a decode session reported when it opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionInfo {
    /// Timebase numerator.
    pub time_base_num: u32,
    /// Timebase denominator.
    pub time_base_den: u32,
    /// Stream index used.
    pub stream_index: u32,
    /// Delivered width (video).
    pub width: u32,
    /// Delivered height (video).
    pub height: u32,
    /// Source width (video).
    pub source_width: u32,
    /// Source height (video).
    pub source_height: u32,
    /// Whether per-sample seeking is in use.
    pub seeking: bool,
}

/// Counts reported at the end of a session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionStats {
    /// Items delivered (frames or chunks).
    pub items: u64,
    /// Frames or samples the decoder produced.
    pub decoded: u64,
}

/// Shared plumbing for frame and audio streams.
struct Session {
    worker: Option<Worker>,
    region: Arc<SharedRegion>,
    release_tx: mpsc::UnboundedSender<u32>,
    release_rx: mpsc::UnboundedReceiver<u32>,
    info: SessionInfo,
    stats: Option<SessionStats>,
    failed: bool,
}

impl Session {
    async fn open(cfg: &WorkerConfig, req: Request, slot_size: usize) -> Result<Self> {
        let region = Arc::new(SharedRegion::create(
            slot_size,
            u32::try_from(cfg.max_in_flight_frames.max(2))
                .map_err(|_| MediaError::Invalid("too many in-flight frames".into()))?,
        )?);
        let mut worker = Worker::spawn(cfg).await?;
        let req = match req {
            Request::DecodeVideo { req, .. } => Request::DecodeVideo {
                req,
                shm: region.spec().clone(),
            },
            Request::DecodeAudio { req, .. } => Request::DecodeAudio {
                req,
                shm: region.spec().clone(),
            },
            other => other,
        };
        worker.send(&req).await?;
        let info = loop {
            match worker.recv().await? {
                Response::Started {
                    time_base_num,
                    time_base_den,
                    stream_index,
                    width,
                    height,
                    source_width,
                    source_height,
                    seeking,
                } => {
                    break SessionInfo {
                        time_base_num,
                        time_base_den,
                        stream_index,
                        width,
                        height,
                        source_width,
                        source_height,
                        seeking,
                    }
                }
                Response::Error { message } => {
                    worker.shutdown().await;
                    return Err(MediaError::Worker(message));
                }
                Response::Log { level, message } => debug!(level, "{message}"),
                other => {
                    return Err(MediaError::Protocol(format!(
                        "unexpected reply while opening session: {other:?}"
                    )))
                }
            }
        };
        let (release_tx, release_rx) = mpsc::unbounded_channel();
        Ok(Self {
            worker: Some(worker),
            region,
            release_tx,
            release_rx,
            info,
            stats: None,
            failed: false,
        })
    }

    /// Forward any released slots to the worker, then wait for the next
    /// response. Slot releases are flushed first so the worker never stalls
    /// on a slot the parent already freed.
    async fn next_response(&mut self) -> Result<Option<Response>> {
        if self.stats.is_some() || self.failed {
            return Ok(None);
        }
        let worker = self
            .worker
            .as_mut()
            .ok_or_else(|| MediaError::WorkerExited("session closed".into()))?;
        loop {
            // Drain pending releases.
            while let Ok(slot) = self.release_rx.try_recv() {
                worker.send(&Request::SlotFree { slot }).await?;
            }
            tokio::select! {
                biased;
                Some(slot) = self.release_rx.recv() => {
                    worker.send(&Request::SlotFree { slot }).await?;
                }
                r = worker.recv() => {
                    match r {
                        Ok(Response::End { items, decoded }) => {
                            self.stats = Some(SessionStats { items, decoded });
                            if let Some(w) = self.worker.take() {
                                tokio::spawn(w.shutdown());
                            }
                            return Ok(None);
                        }
                        Ok(Response::Error { message }) => {
                            self.failed = true;
                            return Err(MediaError::Worker(message));
                        }
                        Ok(Response::Log { level, message }) => {
                            debug!(level, "{message}");
                        }
                        Ok(other) => return Ok(Some(other)),
                        Err(e) => {
                            self.failed = true;
                            return Err(e);
                        }
                    }
                }
            }
        }
    }

    fn guard(&self, slot: u32, len: usize) -> SlotGuard {
        SlotGuard::new(self.region.clone(), slot, len, self.release_tx.clone())
    }
}

/// A stream of decoded frames from one worker.
pub struct FrameStream {
    session: Session,
}

impl std::fmt::Debug for FrameStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameStream")
            .field("info", &self.session.info)
            .finish()
    }
}

impl FrameStream {
    /// Geometry and timebase.
    pub fn info(&self) -> &SessionInfo {
        &self.session.info
    }

    /// Final counts; `None` until the stream has ended cleanly.
    pub fn stats(&self) -> Option<SessionStats> {
        self.session.stats
    }

    /// libav version string of the worker.
    pub fn libav(&self) -> Option<&str> {
        self.session.worker.as_ref().map(|w| w.libav.as_str())
    }

    /// Next frame, or `None` when the session ended.
    pub async fn next(&mut self) -> Result<Option<Arc<FrameBuffer>>> {
        loop {
            match self.session.next_response().await? {
                None => return Ok(None),
                Some(Response::Frame {
                    slot,
                    len,
                    width,
                    height,
                    stride,
                    format,
                    pts,
                    t,
                    is_keyframe,
                }) => {
                    let guard = self.session.guard(slot, len);
                    let meta = FrameMeta {
                        width,
                        height,
                        stride,
                        format,
                        pts,
                        t,
                        is_keyframe,
                        source_width: self.session.info.source_width,
                        source_height: self.session.info.source_height,
                    };
                    return Ok(Some(FrameBuffer::shared(meta, guard)));
                }
                Some(other) => {
                    warn!("ignoring unexpected worker message {other:?}");
                }
            }
        }
    }

    /// Collect every remaining frame as owned buffers (tests, small ranges).
    pub async fn collect_owned(mut self) -> Result<Vec<Arc<FrameBuffer>>> {
        let mut v = Vec::new();
        while let Some(f) = self.next().await? {
            v.push(f.to_owned_frame());
        }
        Ok(v)
    }
}

/// Start decoding video. Frames arrive as `Arc<FrameBuffer>` in RGB24.
pub async fn decode_video(cfg: &WorkerConfig, req: VideoDecodeRequest) -> Result<FrameStream> {
    req.check()?;
    let mut req = req;
    if req.threads == 0 {
        req.threads = cfg.decode_threads;
    }
    // Slot size: the delivered frame is at most max_dim on its longest side,
    // so max_dim² pixels bounds it. Without a max_dim we must know the source
    // size, which costs a probe.
    let slot_size = if req.max_dim > 0 {
        PixelFormat::Rgb24.frame_size(req.max_dim, req.max_dim)
    } else {
        let p = probe(cfg, &req.path).await?;
        let s = p
            .video_stream()
            .ok_or_else(|| MediaError::NoStream("video", req.path.display().to_string()))?;
        PixelFormat::Rgb24.frame_size(s.width.unwrap_or(1920), s.height.unwrap_or(1080))
    };
    let session = Session::open(
        cfg,
        Request::DecodeVideo {
            req,
            shm: crate::shm::ShmSpec {
                path: PathBuf::new(),
                slot_size: 0,
                slots: 0,
            },
        },
        slot_size,
    )
    .await?;
    Ok(FrameStream { session })
}

/// One chunk of 16-bit mono PCM.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioChunk {
    /// Start time.
    pub t0: Timestamp,
    /// End time.
    pub t1: Timestamp,
    /// Sample rate.
    pub sample_rate: u32,
    /// Samples, little-endian signed 16-bit already converted.
    pub samples: Vec<i16>,
}

/// A stream of PCM chunks from one worker.
pub struct AudioStream {
    session: Session,
}

impl std::fmt::Debug for AudioStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioStream")
            .field("info", &self.session.info)
            .finish()
    }
}

impl AudioStream {
    /// Stream geometry.
    pub fn info(&self) -> &SessionInfo {
        &self.session.info
    }

    /// Final counts; `None` until the stream has ended cleanly.
    pub fn stats(&self) -> Option<SessionStats> {
        self.session.stats
    }

    /// Next chunk, or `None` when the session ended. PCM is copied out of the
    /// slot; audio chunks are small compared with frames.
    pub async fn next(&mut self) -> Result<Option<AudioChunk>> {
        loop {
            match self.session.next_response().await? {
                None => return Ok(None),
                Some(Response::AudioChunk {
                    slot,
                    len,
                    t0,
                    t1,
                    sample_rate,
                    samples,
                }) => {
                    let guard = self.session.guard(slot, len);
                    let bytes = guard.as_slice();
                    let n = samples.min(bytes.len() / 2);
                    let pcm: Vec<i16> = bytes[..n * 2]
                        .chunks_exact(2)
                        .map(|b| i16::from_le_bytes([b[0], b[1]]))
                        .collect();
                    drop(guard);
                    return Ok(Some(AudioChunk {
                        t0,
                        t1,
                        sample_rate,
                        samples: pcm,
                    }));
                }
                Some(other) => warn!("ignoring unexpected worker message {other:?}"),
            }
        }
    }

    /// Collect all remaining chunks.
    pub async fn collect(mut self) -> Result<Vec<AudioChunk>> {
        let mut v = Vec::new();
        while let Some(c) = self.next().await? {
            v.push(c);
        }
        Ok(v)
    }
}

/// Start decoding audio to mono PCM chunks.
pub async fn decode_audio(cfg: &WorkerConfig, req: AudioDecodeRequest) -> Result<AudioStream> {
    req.check()?;
    let slot_size = req.chunk_samples() * 2;
    let session = Session::open(
        cfg,
        Request::DecodeAudio {
            req,
            shm: crate::shm::ShmSpec {
                path: PathBuf::new(),
                slot_size: 0,
                slots: 0,
            },
        },
        slot_size,
    )
    .await?;
    Ok(AudioStream { session })
}
