//! Generates the synthetic fixture video with the `ffmpeg` CLI.
//!
//! 120 s, 640x360, 30 fps. Twelve 10-second segments, each a distinct solid
//! background with two white boxes at segment-specific positions (so
//! perceptual hashes differ between segments and stay constant within one), a burned-in
//! `HH:MM:SS.mmm` timestamp top-right, and a 440 Hz tone. Keyframes every
//! 2 s plus the hard cuts. The colours are mirrored in `src/lib.rs`; keep the
//! two lists identical.
//!
//! The timestamp needs ffmpeg's `drawtext` filter (libfreetype). Builds of
//! ffmpeg without it (Homebrew's, for one) get a fixture without the text;
//! frames within a segment are then identical, which content-addressed
//! stores deduplicate. `VI_FIXTURE_HAS_TEXT` tells tests which fixture they
//! have (`vi_testkit::fixture_has_text`).
//!
//! Two derived fixtures follow it, both for the live work:
//! - `fixture-2min-segments/`: the fixture cut into 2 s MPEG-TS segments
//!   (`seg/000001.ts` …) plus `index.json` in the live store's
//!   `SegmentIndex` format (`vi_media::segments`), written here by hand so
//!   this crate does not depend on `vi-media`.
//! - `fixture-tone-silence.wav`: 120 s of 16 kHz mono, 3 s of silence then
//!   5 s of a 440 Hz tone in every 8 s period, for chunking and endpointing
//!   logic driven by a scripted VAD.

use std::path::PathBuf;
use std::process::Command;

const COLORS: [&str; 12] = [
    "0xC03030", "0x30A030", "0x3030C0", "0xC0C030", "0xC030C0", "0x30C0C0", "0xE08020", "0x7030A0",
    "0x208070", "0x805020", "0xE080A0", "0x808080",
];
const SEGMENT_SECS: u32 = 10;
const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;
const FPS: u32 = 30;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=VI_FIXTURE_FORCE");
    println!("cargo:rerun-if-env-changed=FFMPEG");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let out = out_dir.join("fixture-2min.mp4");
    // Present when the fixture was generated without the timestamp overlay.
    let no_text_marker = out_dir.join("fixture-2min.notext");
    println!("cargo:rustc-env=VI_FIXTURE_PATH={}", out.display());
    let segments_dir = out_dir.join("fixture-2min-segments");
    println!(
        "cargo:rustc-env=VI_FIXTURE_SEGMENTS_DIR={}",
        segments_dir.display()
    );
    let tone = out_dir.join("fixture-tone-silence.wav");
    println!(
        "cargo:rustc-env=VI_FIXTURE_TONE_SILENCE_PATH={}",
        tone.display()
    );
    let ffmpeg = std::env::var("FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let force = std::env::var_os("VI_FIXTURE_FORCE").is_some();
    if force || !out.is_file() {
        let has_text = if generate(&ffmpeg, &out, true) {
            let _ = std::fs::remove_file(&no_text_marker);
            true
        } else if generate(&ffmpeg, &out, false) {
            println!(
                "cargo:warning=vi-testkit: fixture generated without the timestamp overlay (drawtext unavailable in `{ffmpeg}`)"
            );
            std::fs::write(&no_text_marker, b"").expect("write fixture marker");
            false
        } else {
            panic!(
                "vi-testkit: could not generate {} with `{ffmpeg}`; install ffmpeg (apt install ffmpeg / brew install ffmpeg) or set FFMPEG",
                out.display()
            );
        };
        emit_has_text(has_text);
    } else {
        emit_has_text(!no_text_marker.is_file());
    }
    if force || !segments_dir.join("index.json").is_file() {
        segment(&ffmpeg, &out, &segments_dir);
    }
    if force || !tone.is_file() {
        tone_silence(&ffmpeg, &tone);
    }
}

fn emit_has_text(has_text: bool) {
    println!("cargo:rustc-env=VI_FIXTURE_HAS_TEXT={has_text}");
}

/// Segment duration of the live-store copy of the fixture.
const LIVE_SEGMENT_SECS: u32 = 2;
/// MPEG-TS clock.
const TS_TIMEBASE_DEN: i64 = 90_000;

/// Cut the fixture into MPEG-TS segments with the ffmpeg segment muxer and
/// write `index.json` from the CSV segment list it produces.
fn segment(ffmpeg: &str, fixture: &std::path::Path, dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
    let seg = dir.join("seg");
    std::fs::create_dir_all(&seg).expect("create segment dir");
    let list = dir.join("seglist.csv");
    let status = Command::new(ffmpeg)
        .args(["-y", "-hide_banner", "-loglevel", "error", "-nostdin", "-i"])
        .arg(fixture)
        .args([
            "-c",
            "copy",
            "-f",
            "segment",
            "-segment_time",
            &LIVE_SEGMENT_SECS.to_string(),
            "-segment_format",
            "mpegts",
            "-segment_start_number",
            "1",
            "-reset_timestamps",
            "0",
            "-segment_list",
        ])
        .arg(&list)
        .args(["-segment_list_type", "csv"])
        .arg(seg.join("%06d.ts"))
        .status();
    assert!(
        matches!(status, Ok(s) if s.success()),
        "vi-testkit: ffmpeg segment muxer failed: {status:?}"
    );
    // `-segment_start_number 1`: the live store numbers segments from 1.
    let csv = std::fs::read_to_string(&list).expect("read segment list");
    let mut entries = Vec::new();
    for line in csv.lines().filter(|l| !l.trim().is_empty()) {
        let mut cols = line.split(',');
        let file = cols.next().expect("segment file").trim();
        let start: f64 = cols
            .next()
            .expect("segment start")
            .trim()
            .parse()
            .expect("segment start as seconds");
        let end: f64 = cols
            .next()
            .expect("segment end")
            .trim()
            .parse()
            .expect("segment end as seconds");
        let seq: u64 = file
            .trim_end_matches(".ts")
            .parse()
            .expect("segment file name is a number");
        assert_eq!(file, format!("{seq:06}.ts"), "segment file name");
        let bytes = std::fs::metadata(seg.join(file))
            .expect("segment size")
            .len();
        entries.push(serde_json::json!({
            "seq": seq,
            "file": format!("seg/{file}"),
            "t0": {"num": (start * TS_TIMEBASE_DEN as f64).round() as i64, "den": TS_TIMEBASE_DEN},
            "t1": {"num": (end * TS_TIMEBASE_DEN as f64).round() as i64, "den": TS_TIMEBASE_DEN},
            "bytes": bytes,
            "wallclock": serde_json::Value::Null,
        }));
    }
    let _ = std::fs::remove_file(&list);
    let index = serde_json::json!({
        "schema": 1,
        "timebase": {"num": 1, "den": TS_TIMEBASE_DEN},
        "segments": entries,
        "gaps": [],
        "ended": true,
    });
    let tmp = dir.join("index.json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(&index).expect("serialise index"),
    )
    .expect("write index");
    std::fs::rename(tmp, dir.join("index.json")).expect("rename index");
}

/// 120 s of 16 kHz mono PCM: in every 8 s period, 3 s of silence then 5 s of
/// a 440 Hz tone (`gt(mod(t,8),3)` gates the sine).
fn tone_silence(ffmpeg: &str, out: &std::path::Path) {
    let tmp = out.with_extension("tmp.wav");
    let status = Command::new(ffmpeg)
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=exprs='0.5*sin(2*PI*440*t)*gt(mod(t\\,8)\\,3)':sample_rate=16000:channel_layout=mono:duration=120",
            "-c:a",
            "pcm_s16le",
        ])
        .arg(&tmp)
        .status();
    assert!(
        matches!(status, Ok(s) if s.success()),
        "vi-testkit: ffmpeg tone-and-silence generation failed: {status:?}"
    );
    std::fs::rename(&tmp, out).expect("rename tone fixture");
}

fn generate(ffmpeg: &str, out: &std::path::Path, with_text: bool) -> bool {
    let tmp = out.with_extension("tmp.mp4");
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-y", "-hide_banner", "-loglevel", "error", "-nostdin"]);
    for c in COLORS {
        cmd.args([
            "-f",
            "lavfi",
            "-i",
            &format!("color=c={c}:size={WIDTH}x{HEIGHT}:rate={FPS}:duration={SEGMENT_SECS}"),
        ]);
    }
    let total = COLORS.len() as u32 * SEGMENT_SECS;
    cmd.args([
        "-f",
        "lavfi",
        "-i",
        &format!("sine=frequency=440:sample_rate=48000:duration={total}"),
    ]);
    let mut graph = String::new();
    for (i, _) in COLORS.iter().enumerate() {
        // Two white boxes whose positions shift per segment in opposite
        // directions, so neighbouring segments differ structurally even when
        // their backgrounds have the same luminance (segments 4 and 5) and no
        // timestamp text is drawn. Neither box reaches the bottom-left probe
        // pixel (`BACKGROUND_PROBE_XY` in `src/lib.rs`).
        graph.push_str(&format!(
            "[{i}:v]drawbox=x={x}:y={y}:w=200:h=120:color=white:t=fill,\
             drawbox=x={x2}:y={y2}:w=120:h=200:color=white:t=fill[s{i}];",
            x = i * 40,
            y = i * 20,
            x2 = 520 - i * 40,
            y2 = 28 + i * 12
        ));
    }
    for i in 0..COLORS.len() {
        graph.push_str(&format!("[s{i}]"));
    }
    graph.push_str(&format!("concat=n={}:v=1:a=0[cat];", COLORS.len()));
    if with_text {
        graph.push_str(
            "[cat]drawtext=text='%{pts\\:hms}':fontsize=28:fontcolor=white:box=1:boxcolor=black@0.7:x=w-tw-10:y=10[v]",
        );
    } else {
        graph.push_str("[cat]copy[v]");
    }
    let audio_in = format!("{}:a", COLORS.len());
    cmd.args(["-filter_complex", &graph, "-map", "[v]", "-map", &audio_in]);
    cmd.args([
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-crf",
        "20",
        "-g",
        "60",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-b:a",
        "64k",
        "-shortest",
        "-movflags",
        "+faststart",
    ]);
    cmd.arg(&tmp);
    match cmd.output() {
        Ok(o) if o.status.success() => std::fs::rename(&tmp, out).is_ok(),
        Ok(o) => {
            println!(
                "cargo:warning=ffmpeg exited with {} (with_text={with_text})",
                o.status
            );
            for line in String::from_utf8_lossy(&o.stderr).lines().take(4) {
                println!("cargo:warning=  ffmpeg: {line}");
            }
            let _ = std::fs::remove_file(&tmp);
            false
        }
        Err(e) => {
            println!("cargo:warning=failed to run {ffmpeg}: {e}");
            false
        }
    }
}
