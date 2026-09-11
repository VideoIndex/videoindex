//! Black-box tests of the `vi` binary against the synthetic fixture.

#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::process::Command;

use vi_testkit as fx;

fn vi() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vi"))
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = vi().args(args).output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn init_index_status_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let idx = dir.path().join("t.vidx");
    let idx_s = idx.to_str().unwrap();
    let fixture = fx::fixture_path();
    let fixture_s = fixture.to_str().unwrap();

    let (ok, out, err) = run(&["init", idx_s]);
    assert!(ok, "{err}");
    assert!(out.contains("created"), "{out}");
    assert!(Path::new(idx_s).join("manifest.json").is_file());
    let (ok, _, _) = run(&["init", idx_s]);
    assert!(!ok, "init twice must fail");

    let (ok, out, err) = run(&["--json", "index", idx_s, fixture_s, "--fps", "2"]);
    assert!(ok, "{err}");
    // Last JSON document on stdout is the report array; progress events precede it.
    let reports_start = out.rfind("[\n").unwrap();
    let reports: serde_json::Value = serde_json::from_str(&out[reports_start..]).unwrap();
    let r = &reports[0];
    assert_eq!(r["ok"], true);
    assert_eq!(r["index_state"], "coarse");
    let samples = r["stages"]["sample"]["items_done"].as_u64().unwrap();
    assert!((238..=241).contains(&samples), "{samples}");
    assert!(
        out.contains("\"type\":\"progress\""),
        "progress events in JSON mode"
    );

    let (ok, out, err) = run(&["status", idx_s]);
    assert!(ok, "{err}");
    assert!(out.contains("duration 00:02:00.000"), "{out}");
    assert!(out.contains(&format!("samples {samples}")), "{out}");
    assert!(out.contains("coarse"), "{out}");
    assert!(out.contains("size:"), "{out}");

    let (ok, out, _) = run(&["--json", "status", idx_s]);
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["videos"][0]["frame_samples"].as_u64().unwrap(), samples);
    assert_eq!(v["videos"][0]["thumbnails"].as_u64().unwrap(), samples);
    assert_eq!(v["blob_count"].as_u64().unwrap(), samples);
    assert!(v["dir_bytes"].as_u64().unwrap() > 0);

    // Second run is skipped; force re-runs.
    let (ok, out, _) = run(&["index", idx_s, fixture_s]);
    assert!(ok);
    assert!(out.contains("already indexed"), "{out}");
    let (ok, _, _) = run(&["--json", "index", idx_s, fixture_s, "--force", "--fps", "1"]);
    assert!(ok);
    let (_, out, _) = run(&["--json", "status", idx_s]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let n = v["videos"][0]["frame_samples"].as_u64().unwrap();
    assert!((118..=121).contains(&n), "{n}");
    assert_eq!(
        v["videos"].as_array().unwrap().len(),
        1,
        "same content, one video"
    );
}

#[test]
fn probe_and_doctor() {
    let (ok, out, err) = run(&["--json", "probe", fx::fixture_path().to_str().unwrap()]);
    assert!(ok, "{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["streams"][0]["codec"], "h264");
    assert!(
        (v["duration"]["num"].as_f64().unwrap() / v["duration"]["den"].as_f64().unwrap() - 120.0)
            .abs()
            < 0.6
    );

    let (ok, out, err) = run(&["--json", "doctor"]);
    assert!(ok, "{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["cpus"].as_u64().unwrap() >= 1);
    assert!(v["memory_total"].as_u64().unwrap() > 0);
    assert!(v["libav"].as_str().unwrap().starts_with("avformat"));
    assert!(v["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == "ffmpeg"));
    assert!(v["gpu"]["detail"].is_string());

    let (ok, out, _) = run(&["doctor"]);
    assert!(ok);
    assert!(out.contains("cpu:") && out.contains("memory:") && out.contains("gpu:"));
}

#[test]
fn errors_have_stable_exit_codes() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nope.vidx");
    let st = vi()
        .args(["status", missing.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!st.status.success());
    assert!(String::from_utf8_lossy(&st.stderr).contains("not an index"));

    let idx = dir.path().join("t.vidx");
    assert!(run(&["init", idx.to_str().unwrap()]).0);
    let st = vi()
        .args([
            "index",
            idx.to_str().unwrap(),
            fx::fixture_path().to_str().unwrap(),
            "--policy",
            "lecture_default",
        ])
        .output()
        .unwrap();
    assert_eq!(
        st.status.code(),
        Some(6),
        "unsupported operators exit 6: {}",
        String::from_utf8_lossy(&st.stderr)
    );
    let st = vi()
        .args([
            "index",
            idx.to_str().unwrap(),
            fx::fixture_path().to_str().unwrap(),
            "--policy",
            "nope",
        ])
        .output()
        .unwrap();
    assert_eq!(st.status.code(), Some(2));
    let st = vi()
        .args(["probe", dir.path().join("missing.mp4").to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(st.status.code(), Some(4));
}
