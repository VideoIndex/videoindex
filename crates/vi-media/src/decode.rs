//! libav probe and decode. Everything here is blocking and runs inside the
//! worker process; the parent never links these code paths at runtime.

use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;

use ff::format::{Pixel, Sample};
use ff::software::{resampling, scaling};
use ff::{codec, format, frame, media, threading, ChannelLayout, Discard, Rational};
use ffmpeg_next as ff;
use vi_core::model::TrackKind;
use vi_core::Timestamp;

use crate::error::{MediaError, Result};
use crate::frame::PixelFormat;
use crate::probe::{ChapterInfo, Probe, StreamInfo};
use crate::protocol::{AudioDecodeRequest, LiveDecodeRequest, Response, VideoDecodeRequest};
use crate::segments::{MediaInput, SegmentEntry, SegmentFeed, SegmentIndex};
use crate::shm::SharedRegionMut;

/// Where decoded items go: a slot allocator plus a response channel. The
/// worker implements it over stdout and the shared region.
pub(crate) trait Sink {
    /// Send a message to the parent.
    fn send(&mut self, resp: &Response) -> Result<()>;
    /// Block until a slot is free; observes `Cancel`.
    fn acquire_slot(&mut self) -> Result<u32>;
    /// Return an error if the parent asked to cancel.
    fn poll_cancel(&mut self) -> Result<()>;
}

/// Keyframe interval assumed when the caller does not know it.
const DEFAULT_KEYFRAME_INTERVAL_SECS: f64 = 5.0;

/// Initialise libav once per process.
pub(crate) fn init() -> Result<()> {
    ff::init()?;
    ff::log::set_level(ff::log::Level::Error);
    Ok(())
}

/// libavformat version string.
pub(crate) fn libav_version() -> String {
    let v = ff::format::version();
    format!("avformat {}.{}.{}", v >> 16, (v >> 8) & 0xff, v & 0xff)
}

fn rational_parts(r: Rational) -> (u32, u32) {
    match (u32::try_from(r.numerator()), u32::try_from(r.denominator())) {
        (Ok(n), Ok(d)) if d != 0 => (n, d),
        _ => (1, Timestamp::MICROS),
    }
}

fn rational_f64(r: Rational) -> Option<f64> {
    if r.denominator() == 0 || r.numerator() <= 0 {
        None
    } else {
        Some(f64::from(r.numerator()) / f64::from(r.denominator()))
    }
}

fn ts_from(pts: i64, tb: (u32, u32)) -> Timestamp {
    Timestamp::new(pts.saturating_mul(i64::from(tb.0)), tb.1)
}

fn is_again(e: &ff::Error) -> bool {
    matches!(e, ff::Error::Other { errno } if *errno == libc::EAGAIN) || matches!(e, ff::Error::Eof)
}

fn no_pts() -> i64 {
    // AV_NOPTS_VALUE
    i64::MIN
}

// ---------------------------------------------------------------- probe --

pub(crate) fn probe_impl(path: &Path) -> Result<Probe> {
    init()?;
    let size_bytes = std::fs::metadata(path)?.len();
    let mut ictx = format::input(path)?;

    let duration_us = ictx.duration();
    let duration = (duration_us > 0).then(|| Timestamp::from_micros(duration_us));

    let mut streams = Vec::new();
    let mut earliest_start = None::<Timestamp>;
    for s in ictx.streams() {
        let params = s.parameters();
        let kind = match params.medium() {
            media::Type::Video => TrackKind::Video,
            media::Type::Audio => TrackKind::Audio,
            media::Type::Subtitle => TrackKind::Subtitle,
            _ => continue,
        };
        let codec_id = params.id();
        let (codec_name, codec_long) = ff::decoder::find(codec_id)
            .map(|c| (c.name().to_string(), c.description().to_string()))
            .unwrap_or_else(|| (format!("{codec_id:?}").to_lowercase(), String::new()));
        let tb = rational_parts(s.time_base());
        let start_time = s.start_time();
        if start_time != no_pts() {
            let st = ts_from(start_time, tb);
            earliest_start = Some(match earliest_start {
                Some(e) if e <= st => e,
                _ => st,
            });
        }
        let dur = (s.duration() > 0).then(|| ts_from(s.duration(), tb));
        let frames = (s.frames() > 0).then_some(s.frames());
        let meta = s.metadata();
        let language = meta.get("language").map(str::to_string);
        let title = meta.get("title").map(str::to_string);
        let is_default = s
            .disposition()
            .contains(ff::format::stream::Disposition::DEFAULT);
        let mut info = StreamInfo {
            index: s.index() as u32,
            kind,
            codec: codec_name,
            codec_long_name: codec_long,
            time_base_num: tb.0,
            time_base_den: tb.1,
            start_time: if start_time == no_pts() {
                0
            } else {
                start_time
            },
            duration: dur,
            frames,
            width: None,
            height: None,
            pix_fmt: None,
            fps: rational_f64(s.rate()),
            avg_fps: rational_f64(s.avg_frame_rate()),
            sample_rate: None,
            channels: None,
            sample_fmt: None,
            bit_rate: (params.bit_rate() > 0).then_some(params.bit_rate()),
            language,
            title,
            is_default,
        };
        // Opening the decoder is the portable way to learn geometry and
        // sample layout through the safe API.
        if let Ok(ctx) = codec::context::Context::from_parameters(params) {
            match kind {
                TrackKind::Video => {
                    if let Ok(v) = ctx.decoder().video() {
                        info.width = Some(v.width());
                        info.height = Some(v.height());
                        if v.format() != Pixel::None {
                            info.pix_fmt = v.format().descriptor().map(|d| d.name().to_string());
                        }
                    }
                }
                TrackKind::Audio => {
                    if let Ok(a) = ctx.decoder().audio() {
                        info.sample_rate = Some(a.rate());
                        info.channels = Some(u32::from(a.channels()));
                        if a.format() != Sample::None {
                            info.sample_fmt = Some(a.format().name().to_string());
                        }
                    }
                }
                TrackKind::Subtitle => {}
            }
        }
        streams.push(info);
    }

    let chapters = ictx
        .chapters()
        .map(|c| {
            let tb = rational_parts(c.time_base());
            ChapterInfo {
                id: c.id(),
                t0: ts_from(c.start(), tb),
                t1: ts_from(c.end(), tb),
                title: c.metadata().get("title").map(str::to_string),
            }
        })
        .collect();

    let metadata = ictx
        .metadata()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let fmt = ictx.format();
    let format_name = fmt.name().to_string();
    let format_long_name = fmt.description().to_string();
    let bit_rate = ictx.bit_rate();

    // Keyframe interval: demux (no decode) the first stretch of the best
    // video stream and take the median gap between key packets.
    let best_video = ictx
        .streams()
        .best(media::Type::Video)
        .map(|s| (s.index(), rational_parts(s.time_base())));
    let keyframe_interval_secs = best_video.and_then(|(idx, tb)| {
        let mut key_pts = Vec::new();
        let mut seen = 0usize;
        for (s, p) in ictx.packets() {
            if s.index() != idx {
                continue;
            }
            seen += 1;
            if p.is_key() {
                if let Some(pts) = p.pts().or(p.dts()) {
                    key_pts.push(pts);
                }
            }
            if key_pts.len() >= 12 || seen >= 3000 {
                break;
            }
        }
        key_pts.sort_unstable();
        let mut gaps: Vec<f64> = key_pts
            .windows(2)
            .map(|w| ts_from(w[1] - w[0], tb).as_secs_f64())
            .filter(|g| *g > 0.0)
            .collect();
        if gaps.is_empty() {
            return None;
        }
        gaps.sort_by(|a, b| a.total_cmp(b));
        Some(gaps[gaps.len() / 2])
    });

    Ok(Probe {
        path: path.display().to_string(),
        size_bytes,
        format_name,
        format_long_name,
        duration,
        start_time: earliest_start.unwrap_or(Timestamp::ZERO),
        bit_rate,
        streams,
        chapters,
        metadata,
        keyframe_interval_secs,
        libav: libav_version(),
    })
}

// ---------------------------------------------------------------- video --

/// Output geometry for a `max_dim` request.
fn scaled_dims(sw: u32, sh: u32, max_dim: u32) -> (u32, u32) {
    if max_dim == 0 || (sw <= max_dim && sh <= max_dim) {
        return (sw.max(1), sh.max(1));
    }
    let scale = f64::from(max_dim) / f64::from(sw.max(sh));
    let w = ((f64::from(sw) * scale).round() as u32).max(1);
    let h = ((f64::from(sh) * scale).round() as u32).max(1);
    (w, h)
}

/// Scales decoded frames to the delivery geometry and copies each into a
/// shared-memory slot, announcing it to the parent.
struct FrameWriter {
    /// The scaler and the input geometry it was built for; rebuilt when a
    /// frame arrives in another format or size.
    scaler: Option<(scaling::Context, (Pixel, u32, u32))>,
    out_dims: (u32, u32),
    format: PixelFormat,
    rgb: frame::Video,
    items: u64,
}

impl FrameWriter {
    fn new(out_dims: (u32, u32), format: PixelFormat) -> Self {
        Self {
            scaler: None,
            out_dims,
            format,
            rgb: frame::Video::empty(),
            items: 0,
        }
    }

    fn emit(
        &mut self,
        sink: &mut dyn Sink,
        shm: &mut SharedRegionMut,
        decoded: &frame::Video,
        pts: i64,
        t: Timestamp,
    ) -> Result<()> {
        let key = (decoded.format(), decoded.width(), decoded.height());
        if self.scaler.as_ref().is_none_or(|(_, k)| *k != key) {
            let (ow, oh) = self.out_dims;
            self.scaler = Some((
                scaling::Context::get(
                    key.0,
                    key.1,
                    key.2,
                    Pixel::RGB24,
                    ow,
                    oh,
                    scaling::Flags::BILINEAR,
                )?,
                key,
            ));
        }
        let (scaler, _) = self
            .scaler
            .as_mut()
            .ok_or_else(|| MediaError::Libav("scaler missing".into()))?;
        scaler.run(decoded, &mut self.rgb)?;

        let (ow, oh) = self.out_dims;
        let bpp = self.format.bytes_per_pixel();
        let row_bytes = ow as usize * bpp;
        let needed = self.format.frame_size(ow, oh);
        let slot = sink.acquire_slot()?;
        {
            let dst = shm.slot_mut(slot);
            if dst.len() < needed {
                return Err(MediaError::Protocol(format!(
                    "shm slot of {} bytes cannot hold a {}x{} frame ({} bytes)",
                    dst.len(),
                    ow,
                    oh,
                    needed
                )));
            }
            let src = self.rgb.data(0);
            let src_stride = self.rgb.stride(0);
            for y in 0..oh as usize {
                let s = &src[y * src_stride..y * src_stride + row_bytes];
                dst[y * row_bytes..(y + 1) * row_bytes].copy_from_slice(s);
            }
        }
        self.items += 1;
        sink.send(&Response::Frame {
            slot,
            len: needed,
            width: ow,
            height: oh,
            stride: row_bytes,
            format: self.format,
            pts,
            t,
            is_keyframe: decoded.is_key(),
        })
    }
}

struct VideoSession<'a> {
    sink: &'a mut dyn Sink,
    shm: &'a mut SharedRegionMut,
    writer: FrameWriter,
    src_dims: (u32, u32),
    tb: (u32, u32),
    start_time: i64,
}

impl VideoSession<'_> {
    fn frame_time(&self, pts: i64) -> Timestamp {
        ts_from(pts.saturating_sub(self.start_time), self.tb)
    }

    fn emit(&mut self, decoded: &frame::Video, pts: i64, t: Timestamp) -> Result<()> {
        self.writer
            .emit(&mut *self.sink, &mut *self.shm, decoded, pts, t)
    }
}

/// Decoder threads for a request: 0 means every CPU.
fn resolve_threads(requested: usize) -> usize {
    if requested == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
    } else {
        requested
    }
}

/// Decode video at the requested rate. Returns `(items, decoded_frames)`.
pub(crate) fn decode_video(
    req: &VideoDecodeRequest,
    keyframe_interval_hint: Option<f64>,
    shm: &mut SharedRegionMut,
    sink: &mut dyn Sink,
) -> Result<(u64, u64)> {
    init()?;
    req.check()?;
    if req.format != PixelFormat::Rgb24 {
        return Err(MediaError::Invalid(
            "only rgb24 delivery is implemented in M0".into(),
        ));
    }
    match &req.input {
        MediaInput::File { path } => {
            decode_video_file(req, path, keyframe_interval_hint, shm, sink)
        }
        MediaInput::Segments(feed) => decode_video_segments(req, feed, shm, sink),
    }
}

/// The file decode: one container, seeking per sample when the rate is far
/// below the keyframe rate, sequential otherwise. Batch indexing runs on
/// this path and its outputs must not change.
fn decode_video_file(
    req: &VideoDecodeRequest,
    path: &Path,
    keyframe_interval_hint: Option<f64>,
    shm: &mut SharedRegionMut,
    sink: &mut dyn Sink,
) -> Result<(u64, u64)> {
    let mut ictx = format::input(path)?;

    let (sidx, tb, start_time, params, src_fps, stream_duration) = {
        let stream = match req.stream_index {
            Some(i) => ictx
                .stream(i as usize)
                .ok_or_else(|| MediaError::NoStream("video", path.display().to_string()))?,
            None => ictx
                .streams()
                .best(media::Type::Video)
                .ok_or_else(|| MediaError::NoStream("video", path.display().to_string()))?,
        };
        if stream.parameters().medium() != media::Type::Video {
            return Err(MediaError::NoStream("video", path.display().to_string()));
        }
        let tb = rational_parts(stream.time_base());
        let st = stream.start_time();
        let st = if st == no_pts() { 0 } else { st };
        let fps = rational_f64(stream.avg_frame_rate())
            .or_else(|| rational_f64(stream.rate()))
            .unwrap_or(30.0);
        let dur = (stream.duration() > 0).then(|| ts_from(stream.duration(), tb).as_secs_f64());
        (stream.index(), tb, st, stream.parameters(), fps, dur)
    };

    let duration_secs = stream_duration.or_else(|| {
        let d = ictx.duration();
        (d > 0).then(|| d as f64 / 1e6)
    });
    let t_end = match (req.t1_secs, duration_secs) {
        (Some(t1), Some(d)) => t1.min(d),
        (Some(t1), None) => t1,
        (None, Some(d)) => d,
        (None, None) => f64::INFINITY,
    };

    let threads = resolve_threads(req.threads);
    let mut ctx = codec::context::Context::from_parameters(params)?;
    let mut tcfg = threading::Config::count(threads);
    tcfg.kind = threading::Type::Frame;
    ctx.set_threading(tcfg);
    let mut dec = ctx.decoder();
    // When sampling far below the source rate, skipping non-reference frames
    // is lossless for the frames we keep and saves a large share of decode
    // work. The chosen frame may then be up to two frames after the target;
    // its true PTS is reported.
    let skip_nonref = req.fps * 2.0 < src_fps;
    if skip_nonref {
        dec.skip_frame(Discard::NonReference);
    }
    let mut vdec = dec.video()?;

    let src_dims = (vdec.width(), vdec.height());
    let out_dims = scaled_dims(src_dims.0, src_dims.1, req.max_dim);

    let interval = 1.0 / req.fps;
    let kf = keyframe_interval_hint.unwrap_or(DEFAULT_KEYFRAME_INTERVAL_SECS);
    let seeking = interval >= 2.0 * kf;

    sink.send(&Response::Started {
        time_base_num: tb.0,
        time_base_den: tb.1,
        stream_index: sidx as u32,
        width: out_dims.0,
        height: out_dims.1,
        source_width: src_dims.0,
        source_height: src_dims.1,
        seeking,
    })?;

    let mut session = VideoSession {
        sink,
        shm,
        writer: FrameWriter::new(out_dims, req.format),
        src_dims,
        tb,
        start_time,
    };
    let _ = session.src_dims;

    let mut decoded_frames = 0u64;
    let mut frame = frame::Video::empty();
    // Half a source frame of tolerance so a target that lands a hair after a
    // frame's PTS still takes that frame.
    let eps = 0.5 / src_fps;

    let seek_to = |ictx: &mut format::context::Input, secs: f64| -> Result<()> {
        let abs = secs + ts_from(start_time, tb).as_secs_f64();
        let us = (abs * 1e6).round() as i64;
        ictx.seek(us, ..us)?;
        Ok(())
    };

    if seeking {
        let mut k = 0u64;
        loop {
            let target = req.t0_secs + k as f64 * interval;
            if target >= t_end {
                break;
            }
            session.sink.poll_cancel()?;
            seek_to(&mut ictx, target)?;
            vdec.flush();
            let mut emitted = false;
            let mut eof = false;
            'pk: loop {
                let next = {
                    let mut it = ictx.packets();
                    it.next()
                };
                match next {
                    Some((s, p)) => {
                        if s.index() != sidx {
                            continue;
                        }
                        vdec.send_packet(&p)?;
                    }
                    None => {
                        vdec.send_eof()?;
                        eof = true;
                    }
                }
                loop {
                    match vdec.receive_frame(&mut frame) {
                        Ok(()) => {
                            decoded_frames += 1;
                            let pts = frame.pts().or_else(|| frame.timestamp()).unwrap_or(0);
                            let t = session.frame_time(pts);
                            if t.as_secs_f64() + eps >= target {
                                session.emit(&frame, pts, t)?;
                                emitted = true;
                                break 'pk;
                            }
                        }
                        Err(e) if is_again(&e) => break,
                        Err(e) => return Err(e.into()),
                    }
                }
                if eof {
                    break;
                }
            }
            if !emitted {
                break;
            }
            k += 1;
        }
    } else {
        if req.t0_secs > 0.0 {
            seek_to(&mut ictx, req.t0_secs)?;
        }
        let mut next_target = req.t0_secs;
        let mut done = false;
        let mut handle = |frame: &frame::Video, session: &mut VideoSession<'_>| -> Result<bool> {
            let pts = frame.pts().or_else(|| frame.timestamp()).unwrap_or(0);
            let t = session.frame_time(pts);
            let secs = t.as_secs_f64();
            if next_target >= t_end {
                return Ok(true);
            }
            if secs + eps >= next_target {
                session.emit(frame, pts, t)?;
                // Advance past every target this frame satisfies.
                while secs + eps >= next_target {
                    next_target += interval;
                }
                if next_target >= t_end {
                    return Ok(true);
                }
            }
            Ok(false)
        };

        let mut packets_seen = 0u64;
        loop {
            let next = {
                let mut it = ictx.packets();
                it.next()
            };
            let Some((s, p)) = next else { break };
            if s.index() != sidx {
                continue;
            }
            packets_seen += 1;
            if packets_seen % 64 == 0 {
                session.sink.poll_cancel()?;
            }
            vdec.send_packet(&p)?;
            loop {
                match vdec.receive_frame(&mut frame) {
                    Ok(()) => {
                        decoded_frames += 1;
                        if handle(&frame, &mut session)? {
                            done = true;
                            break;
                        }
                    }
                    Err(e) if is_again(&e) => break,
                    Err(e) => return Err(e.into()),
                }
            }
            if done {
                break;
            }
        }
        if !done {
            vdec.send_eof()?;
            loop {
                match vdec.receive_frame(&mut frame) {
                    Ok(()) => {
                        decoded_frames += 1;
                        if handle(&frame, &mut session)? {
                            break;
                        }
                    }
                    Err(e) if is_again(&e) => break,
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }

    Ok((session.writer.items, decoded_frames))
}

// ---------------------------------------------------------------- audio --

/// Decode audio to mono PCM chunks. Returns `(chunks, samples_decoded)`.
pub(crate) fn decode_audio(
    req: &AudioDecodeRequest,
    shm: &mut SharedRegionMut,
    sink: &mut dyn Sink,
) -> Result<(u64, u64)> {
    init()?;
    req.check()?;
    match &req.input {
        MediaInput::File { path } => decode_audio_file(req, path, shm, sink),
        MediaInput::Segments(feed) => decode_audio_segments(req, feed, shm, sink),
    }
}

/// The file decode; batch indexing runs on this path and its outputs must
/// not change.
fn decode_audio_file(
    req: &AudioDecodeRequest,
    path: &Path,
    shm: &mut SharedRegionMut,
    sink: &mut dyn Sink,
) -> Result<(u64, u64)> {
    let mut ictx = format::input(path)?;

    let (sidx, tb, start_time, params) = {
        let stream = match req.stream_index {
            Some(i) => ictx
                .stream(i as usize)
                .ok_or_else(|| MediaError::NoStream("audio", path.display().to_string()))?,
            None => ictx
                .streams()
                .best(media::Type::Audio)
                .ok_or_else(|| MediaError::NoStream("audio", path.display().to_string()))?,
        };
        if stream.parameters().medium() != media::Type::Audio {
            return Err(MediaError::NoStream("audio", path.display().to_string()));
        }
        let tb = rational_parts(stream.time_base());
        let st = stream.start_time();
        let st = if st == no_pts() { 0 } else { st };
        (stream.index(), tb, st, stream.parameters())
    };

    let ctx = codec::context::Context::from_parameters(params)?;
    let mut adec = ctx.decoder().audio()?;

    sink.send(&Response::Started {
        time_base_num: tb.0,
        time_base_den: tb.1,
        stream_index: sidx as u32,
        width: 0,
        height: 0,
        source_width: 0,
        source_height: 0,
        seeking: false,
    })?;

    if req.t0_secs > 0.0 {
        let abs = req.t0_secs + ts_from(start_time, tb).as_secs_f64();
        let us = (abs * 1e6).round() as i64;
        ictx.seek(us, ..us)?;
    }

    let rate = req.sample_rate;
    let chunk = req.chunk_samples();
    let hop = chunk - req.overlap_samples();
    let slot_bytes = shm.spec().slot_size;
    if slot_bytes < chunk * 2 {
        return Err(MediaError::Protocol(format!(
            "shm slot of {slot_bytes} bytes cannot hold a chunk of {chunk} samples"
        )));
    }

    let mut resampler: Option<resampling::Context> = None;
    let mut out = frame::Audio::empty();
    let mut pending: VecDeque<i16> = VecDeque::with_capacity(chunk + 4096);
    // Sample index (at `rate`) of `pending[0]`, relative to `base`.
    let mut pending_start: u64 = 0;
    let mut base: Option<Timestamp> = None;
    let chunks = std::cell::Cell::new(0u64);
    let mut samples_decoded = 0u64;
    let t_end = req.t1_secs;

    let emit_chunk = |pending: &mut VecDeque<i16>,
                      pending_start: &mut u64,
                      n: usize,
                      base: Timestamp,
                      sink: &mut dyn Sink,
                      shm: &mut SharedRegionMut|
     -> Result<()> {
        let slot = sink.acquire_slot()?;
        {
            let dst = shm.slot_mut(slot);
            for (i, s) in pending.iter().take(n).enumerate() {
                dst[i * 2..i * 2 + 2].copy_from_slice(&s.to_le_bytes());
            }
        }
        let t0 = base.add(Timestamp::new(*pending_start as i64, rate));
        let t1 = base.add(Timestamp::new((*pending_start + n as u64) as i64, rate));
        chunks.set(chunks.get() + 1);
        sink.send(&Response::AudioChunk {
            slot,
            len: n * 2,
            t0,
            t1,
            sample_rate: rate,
            samples: n,
        })?;
        let drop_n = hop.min(n);
        pending.drain(..drop_n);
        *pending_start += drop_n as u64;
        Ok(())
    };

    let mut aframe = frame::Audio::empty();
    let mut done = false;
    let mut packets_seen = 0u64;

    let mut process = |aframe: &frame::Audio,
                       resampler: &mut Option<resampling::Context>,
                       out: &mut frame::Audio,
                       pending: &mut VecDeque<i16>,
                       pending_start: &mut u64,
                       base: &mut Option<Timestamp>,
                       sink: &mut dyn Sink,
                       shm: &mut SharedRegionMut|
     -> Result<bool> {
        let pts = aframe.pts().or_else(|| aframe.timestamp()).unwrap_or(0);
        let t = ts_from(pts.saturating_sub(start_time), tb);
        let secs = t.as_secs_f64();
        if secs < req.t0_secs - 0.05 {
            return Ok(false);
        }
        if let Some(t1) = t_end {
            if secs >= t1 {
                return Ok(true);
            }
        }
        if base.is_none() {
            *base = Some(t);
        }
        if resampler.is_none() {
            *resampler = Some(resampling::Context::get(
                aframe.format(),
                aframe.channel_layout(),
                aframe.rate(),
                Sample::I16(ff::format::sample::Type::Packed),
                ChannelLayout::default(1),
                rate,
            )?);
        }
        let rs = resampler
            .as_mut()
            .ok_or_else(|| MediaError::Libav("resampler missing".into()))?;
        *out = frame::Audio::empty();
        rs.run(aframe, out)?;
        let n = out.samples();
        if n > 0 {
            let bytes = &out.data(0)[..n * 2];
            pending.extend(
                bytes
                    .chunks_exact(2)
                    .map(|b| i16::from_le_bytes([b[0], b[1]])),
            );
            samples_decoded += n as u64;
        }
        while pending.len() >= chunk {
            let b = base.unwrap_or(Timestamp::ZERO);
            emit_chunk(pending, pending_start, chunk, b, sink, shm)?;
        }
        Ok(false)
    };

    loop {
        let next = {
            let mut it = ictx.packets();
            it.next()
        };
        let Some((s, p)) = next else { break };
        if s.index() != sidx {
            continue;
        }
        packets_seen += 1;
        if packets_seen % 64 == 0 {
            sink.poll_cancel()?;
        }
        adec.send_packet(&p)?;
        loop {
            match adec.receive_frame(&mut aframe) {
                Ok(()) => {
                    if process(
                        &aframe,
                        &mut resampler,
                        &mut out,
                        &mut pending,
                        &mut pending_start,
                        &mut base,
                        sink,
                        shm,
                    )? {
                        done = true;
                        break;
                    }
                }
                Err(e) if is_again(&e) => break,
                Err(e) => return Err(e.into()),
            }
        }
        if done {
            break;
        }
    }
    if !done {
        adec.send_eof()?;
        loop {
            match adec.receive_frame(&mut aframe) {
                Ok(()) => {
                    if process(
                        &aframe,
                        &mut resampler,
                        &mut out,
                        &mut pending,
                        &mut pending_start,
                        &mut base,
                        sink,
                        shm,
                    )? {
                        break;
                    }
                }
                Err(e) if is_again(&e) => break,
                Err(e) => return Err(e.into()),
            }
        }
    }
    // Drain the resampler's internal delay.
    if let Some(rs) = resampler.as_mut() {
        loop {
            // `flush` does not allocate its output (unlike `run`), and
            // libswresample rejects an unallocated frame as "output changed",
            // so size it explicitly.
            out = frame::Audio::new(
                Sample::I16(ff::format::sample::Type::Packed),
                8192,
                ChannelLayout::default(1),
            );
            out.set_rate(rate);
            let delay = rs.flush(&mut out)?;
            let n = out.samples();
            if n > 0 {
                let bytes = &out.data(0)[..n * 2];
                pending.extend(
                    bytes
                        .chunks_exact(2)
                        .map(|b| i16::from_le_bytes([b[0], b[1]])),
                );
                samples_decoded += n as u64;
            }
            if delay.is_none() || n == 0 {
                break;
            }
        }
    }
    let b = base.unwrap_or(Timestamp::ZERO);
    while pending.len() >= chunk {
        emit_chunk(&mut pending, &mut pending_start, chunk, b, sink, shm)?;
    }
    // Final partial chunk: only if it holds more than the overlap, otherwise
    // it is entirely contained in the previous chunk.
    if !pending.is_empty() && (chunks.get() == 0 || pending.len() > req.overlap_samples()) {
        let n = pending.len();
        emit_chunk(&mut pending, &mut pending_start, n, b, sink, shm)?;
    }
    Ok((chunks.get(), samples_decoded))
}

// ------------------------------------------------------------- segments --
//
// A segmented recording (live C2). Every segment is opened on its own: the
// S0.5 measurement put one open at about 100 ms, a third of the budget, and
// libav logs `Packet corrupt` at each boundary when TS segments are read as
// one byte stream. libav reports a segment's times from its own start, so
// the recording's timeline comes from the index: a frame or sample at
// in-segment PTS `p` sits at `entry.t0 + (p - base)`, `base` being the
// segment's first video PTS (audio's when there is no video). A hole in the
// index is therefore a hole in the times, and a `Gap` for live sessions.

/// Poll interval while a following feed has no new segment.
const FOLLOW_POLL: Duration = Duration::from_millis(100);

/// Holes shorter than this between consecutive segments are rounding, not
/// gaps.
const GAP_MIN_SECS: f64 = 0.001;

/// An audio clock that disagrees with the index by more than this at a
/// segment boundary is re-anchored (an index that does not follow the PTS,
/// or the other way round).
const AUDIO_REANCHOR_SECS: f64 = 0.1;

/// Ignore audio this far before a window's start, as the file decoder does.
const AUDIO_LEAD_SECS: f64 = 0.05;

/// One stream of an open segment.
#[derive(Clone, Copy)]
struct SegStream {
    index: usize,
    tb: (u32, u32),
}

/// One segment, open, with the mapping onto the recording's timeline.
struct OpenSegment {
    ictx: format::context::Input,
    /// The segment's `t0` on the recording timeline.
    t0: Timestamp,
    /// In-segment time of the base stream's first packet.
    base: Timestamp,
    video: Option<SegStream>,
    audio: Option<SegStream>,
}

impl OpenSegment {
    /// Open a listed segment, telling the parent (at debug level) which file
    /// so tests can count opens.
    fn open(
        feed: &SegmentFeed,
        entry: &SegmentEntry,
        video_index: Option<u32>,
        audio_index: Option<u32>,
        sink: &mut dyn Sink,
    ) -> Result<Self> {
        let path = feed.segment_path(entry);
        sink.send(&Response::Log {
            level: "debug".into(),
            message: format!("opening segment {}", path.display()),
        })?;
        let ictx = format::input(&path)?;
        let pick = |kind: media::Type, wanted: Option<u32>| -> Result<Option<(SegStream, i64)>> {
            let stream = match wanted {
                Some(i) => {
                    let s = ictx.stream(i as usize).ok_or_else(|| {
                        MediaError::NoStream(
                            if kind == media::Type::Video {
                                "video"
                            } else {
                                "audio"
                            },
                            path.display().to_string(),
                        )
                    })?;
                    if s.parameters().medium() != kind {
                        return Ok(None);
                    }
                    s
                }
                None => match ictx.streams().best(kind) {
                    Some(s) => s,
                    None => return Ok(None),
                },
            };
            let st = stream.start_time();
            Ok(Some((
                SegStream {
                    index: stream.index(),
                    tb: rational_parts(stream.time_base()),
                },
                if st == no_pts() { 0 } else { st },
            )))
        };
        let video = pick(media::Type::Video, video_index)?;
        let audio = pick(media::Type::Audio, audio_index)?;
        let base = video
            .or(audio)
            .map(|(s, st)| ts_from(st, s.tb))
            .unwrap_or(Timestamp::ZERO);
        Ok(Self {
            ictx,
            t0: entry.t0,
            base,
            video: video.map(|(s, _)| s),
            audio: audio.map(|(s, _)| s),
        })
    }

    /// Recording time of an in-segment PTS.
    fn rec_time(&self, pts: i64, tb: (u32, u32)) -> Timestamp {
        self.t0.add(ts_from(pts, tb).sub(self.base))
    }

    fn video_stream(&self, dir: &Path) -> Result<SegStream> {
        self.video
            .ok_or_else(|| MediaError::NoStream("video", dir.display().to_string()))
    }

    fn audio_stream(&self, dir: &Path) -> Result<SegStream> {
        self.audio
            .ok_or_else(|| MediaError::NoStream("audio", dir.display().to_string()))
    }

    /// Codec of a stream, to notice a change between segments.
    fn codec_id(&self, s: SegStream) -> Option<codec::Id> {
        self.ictx.stream(s.index).map(|st| st.parameters().id())
    }

    /// The next packet of stream `index`, skipping the others; `None` at the
    /// end of the segment.
    fn next_packet_of(&mut self, index: usize) -> Option<codec::packet::Packet> {
        loop {
            let next = {
                let mut it = self.ictx.packets();
                it.next()
            };
            match next {
                Some((s, p)) if s.index() == index => return Some(p),
                Some(_) => continue,
                None => return None,
            }
        }
    }

    /// The next packet of either stream: `(true, packet)` for video,
    /// `(false, packet)` for audio; `None` at the end of the segment.
    fn next_av_packet(
        &mut self,
        video: usize,
        audio: Option<usize>,
    ) -> Option<(bool, codec::packet::Packet)> {
        loop {
            let next = {
                let mut it = self.ictx.packets();
                it.next()
            };
            match next {
                Some((s, p)) if s.index() == video => return Some((true, p)),
                Some((s, p)) if Some(s.index()) == audio => return Some((false, p)),
                Some(_) => continue,
                None => return None,
            }
        }
    }
}

/// A video decoder for one segment.
struct SegVideoDecoder {
    dec: ff::decoder::Video,
    src_fps: f64,
    src_dims: (u32, u32),
}

fn open_video_decoder(
    seg: &OpenSegment,
    s: SegStream,
    threads: usize,
    fps_req: f64,
) -> Result<SegVideoDecoder> {
    let stream = seg
        .ictx
        .stream(s.index)
        .ok_or_else(|| MediaError::Libav("video stream vanished".into()))?;
    let src_fps = rational_f64(stream.avg_frame_rate())
        .or_else(|| rational_f64(stream.rate()))
        .unwrap_or(30.0);
    let mut ctx = codec::context::Context::from_parameters(stream.parameters())?;
    let mut tcfg = threading::Config::count(threads);
    tcfg.kind = threading::Type::Frame;
    ctx.set_threading(tcfg);
    let mut dec = ctx.decoder();
    // Same rule as the file decoder: far below the source rate, dropping
    // non-reference frames is lossless for the frames kept.
    if fps_req * 2.0 < src_fps {
        dec.skip_frame(Discard::NonReference);
    }
    let vdec = dec.video()?;
    let src_dims = (vdec.width(), vdec.height());
    Ok(SegVideoDecoder {
        dec: vdec,
        src_fps,
        src_dims,
    })
}

fn open_audio_decoder(seg: &OpenSegment, s: SegStream) -> Result<ff::decoder::Audio> {
    let stream = seg
        .ictx
        .stream(s.index)
        .ok_or_else(|| MediaError::Libav("audio stream vanished".into()))?;
    let ctx = codec::context::Context::from_parameters(stream.parameters())?;
    Ok(ctx.decoder().audio()?)
}

/// Picks the frames a fixed-rate sample keeps, on any timeline: the first
/// frame at or after each target, with half a source frame of tolerance so
/// a target that lands a hair after a frame's PTS still takes that frame.
struct FrameSampler {
    next_target: f64,
    interval: f64,
    t_end: f64,
    eps: f64,
}

impl FrameSampler {
    fn done(&self) -> bool {
        self.next_target >= self.t_end
    }

    /// Whether a frame at `secs` is kept; advances past every target the
    /// frame satisfies.
    fn take(&mut self, secs: f64) -> bool {
        if self.done() || secs + self.eps < self.next_target {
            return false;
        }
        while secs + self.eps >= self.next_target {
            self.next_target += self.interval;
        }
        true
    }
}

/// Receive every frame the decoder has ready, keep the sampled ones. Returns
/// `(frames decoded, sampler exhausted)`.
#[allow(clippy::too_many_arguments)]
fn receive_video_frames(
    seg: &OpenSegment,
    s: SegStream,
    vdec: &mut ff::decoder::Video,
    frame: &mut frame::Video,
    sampler: &mut FrameSampler,
    writer: &mut FrameWriter,
    shm: &mut SharedRegionMut,
    sink: &mut dyn Sink,
) -> Result<(u64, bool)> {
    let mut decoded = 0u64;
    loop {
        match vdec.receive_frame(frame) {
            Ok(()) => {
                decoded += 1;
                let pts = frame.pts().or_else(|| frame.timestamp()).unwrap_or(0);
                let t = seg.rec_time(pts, s.tb);
                if sampler.take(t.as_secs_f64()) {
                    writer.emit(sink, shm, frame, pts, t)?;
                }
                if sampler.done() {
                    return Ok((decoded, true));
                }
            }
            Err(e) if is_again(&e) => return Ok((decoded, false)),
            Err(e) => return Err(e.into()),
        }
    }
}

/// Resamples decoded audio to mono PCM at `rate` and cuts it into chunks of
/// `chunk` samples every `hop` samples, timing them by sample count from the
/// first sample's recording time (the file decoder's rule), so chunk
/// boundaries do not depend on where segments are cut.
struct AudioChunker {
    rate: u32,
    chunk: usize,
    hop: usize,
    /// The resampler and the input layout it was built for.
    resampler: Option<(resampling::Context, (Sample, u16, u32))>,
    pending: VecDeque<i16>,
    /// Sample index (at `rate`) of `pending[0]`, relative to `base`.
    pending_start: u64,
    base: Option<Timestamp>,
    chunks: u64,
    samples: u64,
}

impl AudioChunker {
    fn new(rate: u32, chunk: usize, hop: usize) -> Self {
        Self {
            rate,
            chunk,
            hop: hop.max(1),
            resampler: None,
            pending: VecDeque::with_capacity(chunk + 4096),
            pending_start: 0,
            base: None,
            chunks: 0,
            samples: 0,
        }
    }

    /// Recording time at which the clock expects the next sample.
    fn expected_next(&self) -> Option<Timestamp> {
        self.base.map(|b| {
            b.add(Timestamp::new(
                (self.pending_start + self.pending.len() as u64) as i64,
                self.rate,
            ))
        })
    }

    /// Resample a decoded frame whose first sample sits at `t` and emit the
    /// chunks that became complete.
    fn push(
        &mut self,
        aframe: &frame::Audio,
        t: Timestamp,
        sink: &mut dyn Sink,
        shm: &mut SharedRegionMut,
    ) -> Result<()> {
        if self.base.is_none() {
            self.base = Some(t);
        }
        let key = (aframe.format(), aframe.channels(), aframe.rate());
        if self.resampler.as_ref().is_none_or(|(_, k)| *k != key) {
            // A frame without a named layout (some PCM demuxers) still has
            // a channel count; swresample wants a layout on both sides.
            let layout = aframe.channel_layout();
            let layout = if layout.is_empty() || layout.channels() == 0 {
                ChannelLayout::default(i32::from(aframe.channels()))
            } else {
                layout
            };
            self.resampler = Some((
                resampling::Context::get(
                    aframe.format(),
                    layout,
                    aframe.rate(),
                    Sample::I16(ff::format::sample::Type::Packed),
                    ChannelLayout::default(1),
                    self.rate,
                )?,
                key,
            ));
        }
        let (rs, _) = self
            .resampler
            .as_mut()
            .ok_or_else(|| MediaError::Libav("resampler missing".into()))?;
        let mut out = frame::Audio::empty();
        rs.run(aframe, &mut out)?;
        self.append(&out);
        self.emit_full(sink, shm)
    }

    fn append(&mut self, out: &frame::Audio) {
        let n = out.samples();
        if n > 0 {
            let bytes = &out.data(0)[..n * 2];
            self.pending.extend(
                bytes
                    .chunks_exact(2)
                    .map(|b| i16::from_le_bytes([b[0], b[1]])),
            );
            self.samples += n as u64;
        }
    }

    fn emit_full(&mut self, sink: &mut dyn Sink, shm: &mut SharedRegionMut) -> Result<()> {
        while self.pending.len() >= self.chunk {
            self.emit_chunk(self.chunk, sink, shm)?;
        }
        Ok(())
    }

    fn emit_chunk(
        &mut self,
        n: usize,
        sink: &mut dyn Sink,
        shm: &mut SharedRegionMut,
    ) -> Result<()> {
        let slot = sink.acquire_slot()?;
        {
            let dst = shm.slot_mut(slot);
            for (i, s) in self.pending.iter().take(n).enumerate() {
                dst[i * 2..i * 2 + 2].copy_from_slice(&s.to_le_bytes());
            }
        }
        let base = self.base.unwrap_or(Timestamp::ZERO);
        let t0 = base.add(Timestamp::new(self.pending_start as i64, self.rate));
        let t1 = base.add(Timestamp::new(
            (self.pending_start + n as u64) as i64,
            self.rate,
        ));
        self.chunks += 1;
        sink.send(&Response::AudioChunk {
            slot,
            len: n * 2,
            t0,
            t1,
            sample_rate: self.rate,
            samples: n,
        })?;
        let drop_n = self.hop.min(n);
        self.pending.drain(..drop_n);
        self.pending_start += drop_n as u64;
        Ok(())
    }

    /// Drain the resampler's delay and emit what is pending, including a
    /// final partial chunk when it holds more than the overlap (else it is
    /// entirely inside the previous chunk).
    fn flush(&mut self, sink: &mut dyn Sink, shm: &mut SharedRegionMut) -> Result<()> {
        if let Some((rs, _)) = self.resampler.as_mut() {
            loop {
                // `flush` does not allocate its output; size it explicitly.
                let mut out = frame::Audio::new(
                    Sample::I16(ff::format::sample::Type::Packed),
                    8192,
                    ChannelLayout::default(1),
                );
                out.set_rate(self.rate);
                let delay = rs.flush(&mut out)?;
                let n = out.samples();
                if n > 0 {
                    let bytes = &out.data(0)[..n * 2];
                    self.pending.extend(
                        bytes
                            .chunks_exact(2)
                            .map(|b| i16::from_le_bytes([b[0], b[1]])),
                    );
                    self.samples += n as u64;
                }
                if delay.is_none() || n == 0 {
                    break;
                }
            }
        }
        self.emit_full(sink, shm)?;
        let overlap = self.chunk - self.hop.min(self.chunk);
        if !self.pending.is_empty() && (self.chunks == 0 || self.pending.len() > overlap) {
            let n = self.pending.len();
            self.emit_chunk(n, sink, shm)?;
        }
        Ok(())
    }

    /// Forget the clock and the resampler; the next sample re-anchors.
    fn reset(&mut self) {
        self.resampler = None;
        self.pending.clear();
        self.pending_start = 0;
        self.base = None;
    }
}

/// Feed one audio packet's frames to the chunker. `range` bounds the
/// recording times kept (`None` for a live pass); returns true once a frame
/// at or past the range's end arrived.
#[allow(clippy::too_many_arguments)]
fn receive_audio_frames(
    seg: &OpenSegment,
    s: SegStream,
    adec: &mut ff::decoder::Audio,
    aframe: &mut frame::Audio,
    chunker: &mut AudioChunker,
    range: Option<(f64, Option<f64>)>,
    first_in_segment: &mut bool,
    shm: &mut SharedRegionMut,
    sink: &mut dyn Sink,
) -> Result<bool> {
    loop {
        match adec.receive_frame(aframe) {
            Ok(()) => {
                let pts = aframe.pts().or_else(|| aframe.timestamp()).unwrap_or(0);
                let t = seg.rec_time(pts, s.tb);
                if let Some((t0, t1)) = range {
                    let secs = t.as_secs_f64();
                    if secs < t0 - AUDIO_LEAD_SECS {
                        continue;
                    }
                    if t1.is_some_and(|t1| secs >= t1) {
                        return Ok(true);
                    }
                }
                if *first_in_segment {
                    *first_in_segment = false;
                    // The index says where this segment starts; if the
                    // sample clock disagrees by more than rounding, trust the
                    // index and start a new run of chunks here.
                    if let Some(expected) = chunker.expected_next() {
                        let drift = (t.as_secs_f64() - expected.as_secs_f64()).abs();
                        if drift > AUDIO_REANCHOR_SECS {
                            sink.send(&Response::Log {
                                level: "debug".into(),
                                message: format!(
                                    "audio clock {drift:.3} s off the index at {t}; re-anchoring"
                                ),
                            })?;
                            chunker.flush(sink, shm)?;
                            chunker.reset();
                        }
                    }
                }
                chunker.push(aframe, t, sink, shm)?;
            }
            Err(e) if is_again(&e) => return Ok(false),
            Err(e) => return Err(e.into()),
        }
    }
}

/// Send EOF to an audio decoder and push what it still holds; the times of
/// those frames continue the sample clock.
fn drain_audio_decoder(
    adec: &mut ff::decoder::Audio,
    aframe: &mut frame::Audio,
    chunker: &mut AudioChunker,
    shm: &mut SharedRegionMut,
    sink: &mut dyn Sink,
) -> Result<()> {
    adec.send_eof()?;
    loop {
        match adec.receive_frame(aframe) {
            Ok(()) => {
                let t = chunker.expected_next().unwrap_or(Timestamp::ZERO);
                chunker.push(aframe, t, sink, shm)?;
            }
            Err(e) if is_again(&e) => return Ok(()),
            Err(e) => return Err(e.into()),
        }
    }
}

/// The index, or an empty one while a following feed's writer has not
/// created it yet.
fn load_index_for(feed: &SegmentFeed) -> Result<SegmentIndex> {
    match feed.load_index() {
        Ok(idx) => Ok(idx),
        Err(MediaError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound && feed.follow => {
            Ok(SegmentIndex::new())
        }
        Err(e) => Err(e),
    }
}

fn started_response(s: SegStream, out_dims: (u32, u32), src_dims: (u32, u32)) -> Response {
    Response::Started {
        time_base_num: s.tb.0,
        time_base_den: s.tb.1,
        stream_index: s.index as u32,
        width: out_dims.0,
        height: out_dims.1,
        source_width: src_dims.0,
        source_height: src_dims.1,
        seeking: false,
    }
}

/// A window decode over a recording: open only the segments that hold a
/// sampling target inside `[t0, t1)`, in order, and sample across them on
/// the recording's timeline.
fn decode_video_segments(
    req: &VideoDecodeRequest,
    feed: &SegmentFeed,
    shm: &mut SharedRegionMut,
    sink: &mut dyn Sink,
) -> Result<(u64, u64)> {
    let index = feed.load_index()?;
    let dir = feed.dir.as_path();
    let t0 = Timestamp::from_secs_f64(req.t0_secs, Timestamp::MICROS);
    let t1 = req
        .t1_secs
        .map(|s| Timestamp::from_secs_f64(s, Timestamp::MICROS))
        .unwrap_or_else(|| index.head());
    let covering: Vec<SegmentEntry> = index.covering(t0, t1).to_vec();
    let mut sampler = FrameSampler {
        next_target: req.t0_secs,
        interval: 1.0 / req.fps,
        t_end: req.t1_secs.unwrap_or(f64::INFINITY),
        eps: 0.5 / 30.0,
    };
    let threads = resolve_threads(req.threads);
    let mut writer: Option<FrameWriter> = None;
    let mut frame = frame::Video::empty();
    let mut decoded = 0u64;
    for entry in &covering {
        if sampler.done() {
            break;
        }
        if sampler.next_target >= entry.t1.as_secs_f64() {
            // No target falls inside this segment.
            continue;
        }
        let mut seg = OpenSegment::open(feed, entry, req.stream_index, None, sink)?;
        let vs = seg.video_stream(dir)?;
        let mut vdec = open_video_decoder(&seg, vs, threads, req.fps)?;
        sampler.eps = 0.5 / vdec.src_fps;
        if writer.is_none() {
            let out_dims = scaled_dims(vdec.src_dims.0, vdec.src_dims.1, req.max_dim);
            sink.send(&started_response(vs, out_dims, vdec.src_dims))?;
            writer = Some(FrameWriter::new(out_dims, req.format));
        }
        let w = writer
            .as_mut()
            .ok_or_else(|| MediaError::Libav("frame writer missing".into()))?;
        let mut packets_seen = 0u64;
        let mut exhausted = false;
        while let Some(p) = seg.next_packet_of(vs.index) {
            packets_seen += 1;
            if packets_seen % 64 == 0 {
                sink.poll_cancel()?;
            }
            vdec.dec.send_packet(&p)?;
            let (n, done) = receive_video_frames(
                &seg,
                vs,
                &mut vdec.dec,
                &mut frame,
                &mut sampler,
                w,
                shm,
                sink,
            )?;
            decoded += n;
            if done {
                exhausted = true;
                break;
            }
        }
        if !exhausted {
            vdec.dec.send_eof()?;
            let (n, _) = receive_video_frames(
                &seg,
                vs,
                &mut vdec.dec,
                &mut frame,
                &mut sampler,
                w,
                shm,
                sink,
            )?;
            decoded += n;
        }
    }
    let items = match writer {
        Some(w) => w.items,
        None => {
            // Nothing to open (a window past the head, or one that holds no
            // sampling target). The parent still expects the geometry, so
            // read it from the nearest listed segment.
            let nearest = index
                .covering(t0, t1)
                .first()
                .or_else(|| index.segments.last())
                .ok_or_else(|| MediaError::NoStream("segment", dir.display().to_string()))?;
            let seg = OpenSegment::open(feed, nearest, req.stream_index, None, sink)?;
            let vs = seg.video_stream(dir)?;
            let vdec = open_video_decoder(&seg, vs, threads, req.fps)?;
            let out_dims = scaled_dims(vdec.src_dims.0, vdec.src_dims.1, req.max_dim);
            sink.send(&started_response(vs, out_dims, vdec.src_dims))?;
            0
        }
    };
    Ok((items, decoded))
}

/// A window decode of audio over a recording: the covering segments in
/// order, one audio decoder and one sample clock across contiguous
/// segments, re-anchored at holes and discontinuities.
fn decode_audio_segments(
    req: &AudioDecodeRequest,
    feed: &SegmentFeed,
    shm: &mut SharedRegionMut,
    sink: &mut dyn Sink,
) -> Result<(u64, u64)> {
    let index = feed.load_index()?;
    let dir = feed.dir.as_path();
    let t0 = Timestamp::from_secs_f64(req.t0_secs, Timestamp::MICROS);
    let t1 = req
        .t1_secs
        .map(|s| Timestamp::from_secs_f64(s, Timestamp::MICROS))
        .unwrap_or_else(|| index.head());
    let covering: Vec<SegmentEntry> = index.covering(t0, t1).to_vec();
    let chunk = req.chunk_samples();
    let slot_bytes = shm.spec().slot_size;
    if slot_bytes < chunk * 2 {
        return Err(MediaError::Protocol(format!(
            "shm slot of {slot_bytes} bytes cannot hold a chunk of {chunk} samples"
        )));
    }
    let mut chunker = AudioChunker::new(req.sample_rate, chunk, chunk - req.overlap_samples());
    let mut aframe = frame::Audio::empty();
    let mut adec: Option<(Option<codec::Id>, ff::decoder::Audio)> = None;
    let mut started = false;
    let mut prev_t1: Option<Timestamp> = None;
    let range = Some((req.t0_secs, req.t1_secs));
    'segments: for entry in &covering {
        let mut seg = OpenSegment::open(feed, entry, None, req.stream_index, sink)?;
        let a = seg.audio_stream(dir)?;
        if !started {
            sink.send(&started_response(a, (0, 0), (0, 0)))?;
            started = true;
        }
        let hole = prev_t1.is_some_and(|p| entry.t0.as_secs_f64() - p.as_secs_f64() > GAP_MIN_SECS);
        let codec_now = seg.codec_id(a);
        if hole || entry.discontinuity || adec.as_ref().is_some_and(|(id, _)| *id != codec_now) {
            if let Some((_, dec)) = adec.as_mut() {
                drain_audio_decoder(dec, &mut aframe, &mut chunker, shm, sink)?;
            }
            chunker.flush(sink, shm)?;
            chunker.reset();
            adec = None;
        }
        if adec.is_none() {
            adec = Some((codec_now, open_audio_decoder(&seg, a)?));
        }
        let (_, dec) = adec
            .as_mut()
            .ok_or_else(|| MediaError::Libav("audio decoder missing".into()))?;
        let mut first = true;
        let mut packets_seen = 0u64;
        while let Some(p) = seg.next_packet_of(a.index) {
            packets_seen += 1;
            if packets_seen % 64 == 0 {
                sink.poll_cancel()?;
            }
            dec.send_packet(&p)?;
            if receive_audio_frames(
                &seg,
                a,
                dec,
                &mut aframe,
                &mut chunker,
                range,
                &mut first,
                shm,
                sink,
            )? {
                break 'segments;
            }
        }
        prev_t1 = Some(entry.t1);
    }
    if let Some((_, dec)) = adec.as_mut() {
        drain_audio_decoder(dec, &mut aframe, &mut chunker, shm, sink)?;
    }
    chunker.flush(sink, shm)?;
    if !started {
        let nearest = index
            .segments
            .last()
            .ok_or_else(|| MediaError::NoStream("segment", dir.display().to_string()))?;
        let seg = OpenSegment::open(feed, nearest, None, req.stream_index, sink)?;
        let a = seg.audio_stream(dir)?;
        sink.send(&started_response(a, (0, 0), (0, 0)))?;
    }
    Ok((chunker.chunks, chunker.samples))
}

/// One pass over a recording: frames at `fps`, audio in chunks, a `Tick`
/// whenever the decoded stream time crosses another `tick_secs`, a `Gap` at
/// every hole in the index. Under `follow` the worker waits at the index for
/// the next segment and returns when the index says `ended`; otherwise it
/// returns after the last listed segment. Returns `(items, frames decoded)`.
pub(crate) fn decode_live(
    req: &LiveDecodeRequest,
    shm: &mut SharedRegionMut,
    sink: &mut dyn Sink,
) -> Result<(u64, u64)> {
    init()?;
    req.check()?;
    let feed = req
        .input
        .as_segments()
        .ok_or_else(|| MediaError::Invalid("decode_live needs a segment feed".into()))?;
    let dir = feed.dir.as_path();
    let chunk = req.chunk_samples();
    let slot_bytes = shm.spec().slot_size;
    if slot_bytes < chunk * 2 {
        return Err(MediaError::Protocol(format!(
            "shm slot of {slot_bytes} bytes cannot hold a chunk of {chunk} samples"
        )));
    }
    let threads = resolve_threads(req.threads);
    let mut chunker = AudioChunker::new(req.sample_rate, chunk, chunk);
    let mut sampler = FrameSampler {
        next_target: f64::NAN,
        interval: 1.0 / req.fps,
        t_end: f64::INFINITY,
        eps: 0.5 / 30.0,
    };
    let mut writer: Option<FrameWriter> = None;
    let mut adec: Option<(Option<codec::Id>, ff::decoder::Audio)> = None;
    let mut frame = frame::Video::empty();
    let mut aframe = frame::Audio::empty();
    let mut last_seq: Option<u64> = None;
    let mut prev_t1: Option<Timestamp> = None;
    let mut next_tick: Option<f64> = None;
    let mut last_tick: Option<Timestamp> = None;
    let mut done_until: Option<Timestamp> = None;
    let mut decoded_frames = 0u64;

    let mut index = load_index_for(feed)?;
    loop {
        let entry = index
            .segments
            .iter()
            .find(|e| last_seq.is_none_or(|s| e.seq > s))
            .cloned();
        let Some(entry) = entry else {
            if index.ended || !feed.follow {
                break;
            }
            sink.poll_cancel()?;
            std::thread::sleep(FOLLOW_POLL);
            index = load_index_for(feed)?;
            continue;
        };

        // A hole in the index is a gap in the stream: finish the audio run,
        // tell the parent, and let the sample clock re-anchor after it.
        let hole = prev_t1.filter(|p| entry.t0.as_secs_f64() - p.as_secs_f64() > GAP_MIN_SECS);
        if hole.is_some() || entry.discontinuity {
            if let Some((_, dec)) = adec.as_mut() {
                drain_audio_decoder(dec, &mut aframe, &mut chunker, shm, sink)?;
            }
            chunker.flush(sink, shm)?;
            chunker.reset();
            adec = None;
            if let Some(p) = hole {
                sink.send(&Response::Gap {
                    t0: p,
                    t1: entry.t0,
                })?;
            }
        }

        let mut seg = OpenSegment::open(feed, &entry, None, None, sink)?;
        let vs = seg.video_stream(dir)?;
        let mut vdec = open_video_decoder(&seg, vs, threads, req.fps)?;
        sampler.eps = 0.5 / vdec.src_fps;
        if sampler.next_target.is_nan() {
            sampler.next_target = entry.t0.as_secs_f64();
        }
        if writer.is_none() {
            let out_dims = scaled_dims(vdec.src_dims.0, vdec.src_dims.1, req.max_dim);
            sink.send(&started_response(vs, out_dims, vdec.src_dims))?;
            writer = Some(FrameWriter::new(out_dims, PixelFormat::Rgb24));
        }
        if next_tick.is_none() {
            next_tick = Some(entry.t0.as_secs_f64() + req.tick_secs);
        }
        let w = writer
            .as_mut()
            .ok_or_else(|| MediaError::Libav("frame writer missing".into()))?;
        let audio = seg.audio;
        if let Some(a) = audio {
            let codec_now = seg.codec_id(a);
            if adec.as_ref().is_some_and(|(id, _)| *id != codec_now) {
                if let Some((_, dec)) = adec.as_mut() {
                    drain_audio_decoder(dec, &mut aframe, &mut chunker, shm, sink)?;
                }
                chunker.flush(sink, shm)?;
                chunker.reset();
                adec = None;
            }
            if adec.is_none() {
                adec = Some((codec_now, open_audio_decoder(&seg, a)?));
            }
        }

        let mut first_audio = true;
        let mut packets_seen = 0u64;
        while let Some((is_video, p)) = seg.next_av_packet(vs.index, audio.map(|a| a.index)) {
            packets_seen += 1;
            if packets_seen % 64 == 0 {
                sink.poll_cancel()?;
            }
            if is_video {
                vdec.dec.send_packet(&p)?;
                let (n, _) = receive_video_frames(
                    &seg,
                    vs,
                    &mut vdec.dec,
                    &mut frame,
                    &mut sampler,
                    w,
                    shm,
                    sink,
                )?;
                decoded_frames += n;
            } else if let (Some(a), Some((_, dec))) = (audio, adec.as_mut()) {
                dec.send_packet(&p)?;
                receive_audio_frames(
                    &seg,
                    a,
                    dec,
                    &mut aframe,
                    &mut chunker,
                    None,
                    &mut first_audio,
                    shm,
                    sink,
                )?;
            }
        }
        // The video decoder is per segment: drain it now. The audio decoder
        // and its clock continue into the next contiguous segment.
        vdec.dec.send_eof()?;
        let (n, _) = receive_video_frames(
            &seg,
            vs,
            &mut vdec.dec,
            &mut frame,
            &mut sampler,
            w,
            shm,
            sink,
        )?;
        decoded_frames += n;

        prev_t1 = Some(entry.t1);
        last_seq = Some(entry.seq);
        done_until = Some(entry.t1);
        // A tick whenever the decoded stream time crosses the next
        // multiple of `tick_secs`; `head` is what has been delivered,
        // never past a listed segment's end.
        if let Some(due) = next_tick {
            let head = entry.t1.as_secs_f64();
            if head >= due {
                sink.send(&Response::Tick { head: entry.t1 })?;
                last_tick = Some(entry.t1);
                let mut n = due;
                while head >= n {
                    n += req.tick_secs;
                }
                next_tick = Some(n);
            }
        }
    }

    if writer.is_none() {
        return Err(MediaError::NoStream("segment", dir.display().to_string()));
    }
    if let Some((_, dec)) = adec.as_mut() {
        drain_audio_decoder(dec, &mut aframe, &mut chunker, shm, sink)?;
    }
    chunker.flush(sink, shm)?;
    if let Some(head) = done_until {
        if last_tick.is_none_or(|t| t < head) {
            sink.send(&Response::Tick { head })?;
        }
    }
    let frames = writer.map(|w| w.items).unwrap_or(0);
    Ok((frames + chunker.chunks, decoded_frames))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_dims_keep_aspect() {
        assert_eq!(scaled_dims(1280, 720, 640), (640, 360));
        assert_eq!(scaled_dims(720, 1280, 640), (360, 640));
        assert_eq!(scaled_dims(320, 240, 640), (320, 240));
        assert_eq!(scaled_dims(1920, 1080, 0), (1920, 1080));
    }

    #[test]
    fn timestamp_from_pts() {
        let t = ts_from(90_000, (1, 90_000));
        assert_eq!(t.as_secs_f64(), 1.0);
    }
}
