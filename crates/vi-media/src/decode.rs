//! libav probe and decode. Everything here is blocking and runs inside the
//! worker process; the parent never links these code paths at runtime.

use std::collections::VecDeque;
use std::path::Path;

use ff::format::{Pixel, Sample};
use ff::software::{resampling, scaling};
use ff::{codec, format, frame, media, threading, ChannelLayout, Discard, Rational};
use ffmpeg_next as ff;
use vi_core::model::TrackKind;
use vi_core::Timestamp;

use crate::error::{MediaError, Result};
use crate::frame::PixelFormat;
use crate::probe::{ChapterInfo, Probe, StreamInfo};
use crate::protocol::{AudioDecodeRequest, Response, VideoDecodeRequest};
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
    let duration = if duration_us > 0 {
        Timestamp::from_micros(duration_us)
    } else {
        Timestamp::ZERO
    };

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

struct VideoSession<'a> {
    sink: &'a mut dyn Sink,
    shm: &'a mut SharedRegionMut,
    scaler: Option<scaling::Context>,
    out_fmt: Pixel,
    out_dims: (u32, u32),
    src_dims: (u32, u32),
    tb: (u32, u32),
    start_time: i64,
    format: PixelFormat,
    rgb: frame::Video,
    items: u64,
}

impl VideoSession<'_> {
    fn frame_time(&self, pts: i64) -> Timestamp {
        ts_from(pts.saturating_sub(self.start_time), self.tb)
    }

    fn emit(&mut self, decoded: &frame::Video, pts: i64, t: Timestamp) -> Result<()> {
        if self.scaler.is_none() {
            let (ow, oh) = self.out_dims;
            self.scaler = Some(scaling::Context::get(
                decoded.format(),
                decoded.width(),
                decoded.height(),
                self.out_fmt,
                ow,
                oh,
                scaling::Flags::BILINEAR,
            )?);
        }
        let scaler = self
            .scaler
            .as_mut()
            .ok_or_else(|| MediaError::Libav("scaler missing".into()))?;
        scaler.run(decoded, &mut self.rgb)?;

        let (ow, oh) = self.out_dims;
        let bpp = self.format.bytes_per_pixel();
        let row_bytes = ow as usize * bpp;
        let needed = self.format.frame_size(ow, oh);
        let slot = self.sink.acquire_slot()?;
        {
            let dst = self.shm.slot_mut(slot);
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
        self.sink.send(&Response::Frame {
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
    let mut ictx = format::input(&req.path)?;

    let (sidx, tb, start_time, params, src_fps, stream_duration) = {
        let stream = match req.stream_index {
            Some(i) => ictx
                .stream(i as usize)
                .ok_or_else(|| MediaError::NoStream("video", req.path.display().to_string()))?,
            None => ictx
                .streams()
                .best(media::Type::Video)
                .ok_or_else(|| MediaError::NoStream("video", req.path.display().to_string()))?,
        };
        if stream.parameters().medium() != media::Type::Video {
            return Err(MediaError::NoStream(
                "video",
                req.path.display().to_string(),
            ));
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

    let threads = if req.threads == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
    } else {
        req.threads
    };
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
        scaler: None,
        out_fmt: Pixel::RGB24,
        out_dims,
        src_dims,
        tb,
        start_time,
        format: req.format,
        rgb: frame::Video::empty(),
        items: 0,
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

    Ok((session.items, decoded_frames))
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
    let mut ictx = format::input(&req.path)?;

    let (sidx, tb, start_time, params) = {
        let stream = match req.stream_index {
            Some(i) => ictx
                .stream(i as usize)
                .ok_or_else(|| MediaError::NoStream("audio", req.path.display().to_string()))?,
            None => ictx
                .streams()
                .best(media::Type::Audio)
                .ok_or_else(|| MediaError::NoStream("audio", req.path.display().to_string()))?,
        };
        if stream.parameters().medium() != media::Type::Audio {
            return Err(MediaError::NoStream(
                "audio",
                req.path.display().to_string(),
            ));
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
