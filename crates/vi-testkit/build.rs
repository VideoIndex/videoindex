//! Generates the synthetic fixture video with the `ffmpeg` CLI.
//!
//! 120 s, 640x360, 30 fps. Twelve 10-second segments, each a distinct solid
//! background with a white box at a segment-specific position (so perceptual
//! hashes differ between segments and stay constant within one), a burned-in
//! `HH:MM:SS.mmm` timestamp top-right, and a 440 Hz tone. Keyframes every
//! 2 s plus the hard cuts. The colours are mirrored in `src/lib.rs`; keep the
//! two lists identical.

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
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let out = out_dir.join("fixture-2min.mp4");
    println!("cargo:rustc-env=VI_FIXTURE_PATH={}", out.display());
    if out.is_file() && std::env::var_os("VI_FIXTURE_FORCE").is_none() {
        return;
    }
    let ffmpeg = std::env::var("FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    if !generate(&ffmpeg, &out, true) && !generate(&ffmpeg, &out, false) {
        panic!(
            "vi-testkit: could not generate {} with `{ffmpeg}`; install ffmpeg (apt install ffmpeg / brew install ffmpeg) or set FFMPEG",
            out.display()
        );
    }
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
        // A white box whose position shifts per segment.
        graph.push_str(&format!(
            "[{i}:v]drawbox=x={x}:y={y}:w=200:h=120:color=white:t=fill[s{i}];",
            x = i * 40,
            y = i * 20
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
    match cmd.status() {
        Ok(s) if s.success() => std::fs::rename(&tmp, out).is_ok(),
        Ok(s) => {
            println!("cargo:warning=ffmpeg exited with {s} (with_text={with_text})");
            let _ = std::fs::remove_file(&tmp);
            false
        }
        Err(e) => {
            println!("cargo:warning=failed to run {ffmpeg}: {e}");
            false
        }
    }
}
