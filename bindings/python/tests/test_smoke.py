"""Smoke test for the Python binding: index a generated clip, search,
timeline, view, frame. Needs ffmpeg on PATH (like the Rust tests)."""
import os
import shutil
import subprocess
import sys
import tempfile

import pytest

import videoindex as vi


def make_clip(path, secs=20):
    ffmpeg = shutil.which("ffmpeg")
    if not ffmpeg:
        pytest.skip("ffmpeg not installed")
    subprocess.run(
        [ffmpeg, "-y", "-hide_banner", "-loglevel", "error", "-f", "lavfi",
         "-i", f"testsrc2=size=320x180:rate=10:duration={secs}",
         "-f", "lavfi", "-i", f"sine=frequency=440:sample_rate=16000:duration={secs}",
         "-c:v", "libx264", "-preset", "ultrafast", "-g", "20", "-pix_fmt", "yuv420p",
         "-c:a", "aac", "-shortest", path],
        check=True,
    )


def worker_env():
    # The decode worker re-executes the current binary; a Python process
    # cannot act as the worker, so point at the built vidx binary or worker.
    root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
    for cand in ("target/release/vi-media-worker", "target/debug/vi-media-worker", "target/release/vidx", "target/debug/vidx"):
        p = os.path.join(root, cand)
        if os.path.exists(p):
            return p
    pytest.skip("no vi-media-worker binary built")


def test_version():
    assert vi.version() and vi.__version__ == vi.version()


def test_index_search_view_frame(tmp_path):
    worker = worker_env()
    clip = str(tmp_path / "clip.mp4")
    make_clip(clip)
    cfg = vi.Config.from_toml(
        f"""
        [media]
        cache_dir = "{tmp_path / 'videos'}"
        [media.worker]
        path = "{worker}"
        """
    )
    idx = vi.Index.create(str(tmp_path / "t.vidx"), config=cfg)
    job = idx.add(clip, policy="m0")
    events = list(job.progress())
    assert any(e["type"] == "job_started" for e in events)
    assert events[-1]["type"] == "job_finished" and events[-1]["ok"]
    report = job.wait()
    assert report["error"] is None
    assert report["reports"][0]["index_state"] == "coarse"
    videos = idx.videos()
    assert len(videos) == 1
    vid = videos[0]["id"]
    assert abs(videos[0]["duration"] - 20.0) < 1.0, videos[0]["duration"]

    # No text in a synthetic clip: search is empty but well-formed.
    assert idx.search("anything", text_only=True) == []
    st = idx.status()
    assert st["videos"][0]["frame_samples"] >= 19

    grid = idx.view(vid, t0=0, t1=9, fps=1, cols=3)
    assert grid["png"][:8] == b"\x89PNG\r\n\x1a\n"
    assert grid["width"] > 0 and len(grid["timestamps"]) == 9

    frame = idx.frame(vid, t=5.0)
    assert frame.dtype.name == "uint8"
    assert frame.ndim == 3 and frame.shape[2] == 3
    assert frame.shape[1] == 320 and frame.shape[0] == 180

    assert idx.timeline(vid, level="shot") == []
    # A second add of the same file is a no-op with the same video id.
    again = idx.add(clip, policy="m0").wait()
    assert again["reports"][0]["skipped"] is True
    assert again["reports"][0]["video_id"] == vid


def test_ask_without_provider_reports_partial(tmp_path):
    worker = worker_env()
    cfg = vi.Config.from_toml(f"[media.worker]\npath = \"{worker}\"")
    idx = vi.Index.create(str(tmp_path / "e.vidx"), config=cfg)
    out = idx.ask("hello", budget=vi.Budget(max_tool_calls=1)).collect()
    assert out["partial"] is True
    assert "agent_llm" in (out.get("text", "") + str(out))  or out["usage"]["provider_calls"] == 0


if __name__ == "__main__":
    sys.exit(pytest.main([__file__, "-q"]))
