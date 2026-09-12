#!/usr/bin/env python3
"""Uniform-sampling baseline: no index. N frames sampled evenly over the whole
video are tiled into labelled grids, the transcript (when the video has
captions) is attached as text, and one VLM call answers the question.

    python3 -m eval.runners.baselines lvbench --root /data/videoindex/eval/lvbench \
        --frames 32 --model claude-sonnet-5 --sample 300 --seed 1 --out /data/videoindex/eval/runs/lvbench-uniform32.json

Uses the Anthropic Messages API directly (ANTHROPIC_API_KEY) so the baseline is
independent of the VideoIndex stack; the only shared code is the question
loader and the letter parser. Cost is list price for the model.
"""
from __future__ import annotations

import argparse
import base64
import io
import json
import os
import re
import subprocess
import sys
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

from PIL import Image, ImageDraw

from ..datasets import Question, load
from ..datasets import sample_size, stratified_sample
from .answer import parse_letter

PRICES = {  # USD per million tokens, list price
    "claude-sonnet-5": (3.0, 15.0),
    "claude-haiku-4-5-20251001": (1.0, 5.0),
}


def duration_secs(path: Path) -> float:
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(path)],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    return float(out)


def grab(path: Path, t: float, w: int = 320) -> Image.Image:
    out = subprocess.run(
        ["ffmpeg", "-v", "error", "-ss", f"{t:.2f}", "-i", str(path), "-frames:v", "1", "-vf", f"scale={w}:-2", "-f", "image2pipe", "-vcodec", "mjpeg", "-"],
        capture_output=True, check=True,
    ).stdout
    return Image.open(io.BytesIO(out)).convert("RGB")


def hms(t: float) -> str:
    t = int(t)
    return f"{t // 3600}:{t % 3600 // 60:02d}:{t % 60:02d}"


def grids(path: Path, n: int, cols: int = 4, per_grid: int = 16) -> list[tuple[str, str]]:
    """Labelled JPEG grids (base64) covering the video uniformly."""
    dur = duration_secs(path)
    times = [dur * (i + 0.5) / n for i in range(n)]
    frames = []
    for t in times:
        try:
            frames.append((t, grab(path, t)))
        except subprocess.CalledProcessError:
            continue
    out = []
    for start in range(0, len(frames), per_grid):
        chunk = frames[start:start + per_grid]
        w, h = chunk[0][1].size
        rows = (len(chunk) + cols - 1) // cols
        sheet = Image.new("RGB", (cols * w, rows * (h + 18)), "black")
        draw = ImageDraw.Draw(sheet)
        for i, (t, im) in enumerate(chunk):
            x, y = (i % cols) * w, (i // cols) * (h + 18)
            sheet.paste(im, (x, y + 18))
            draw.text((x + 4, y + 2), hms(t), fill="white")
        buf = io.BytesIO()
        sheet.save(buf, "JPEG", quality=80)
        out.append(("image/jpeg", base64.b64encode(buf.getvalue()).decode()))
    return out


def transcript_text(root: Path, key: str, limit_chars: int = 60000) -> str:
    for p in sorted((root / "videos").glob(f"{key}*.srt")):
        text = re.sub(r"\d+\n\d\d:\d\d:\d\d,\d+ --> .*\n", "", p.read_text(errors="replace"))
        text = re.sub(r"\n{2,}", "\n", text)
        # Keep timestamps coarse: every ~cue block already stripped; prefix nothing.
        return text[:limit_chars]
    return ""


def call_anthropic(model: str, images: list[tuple[str, str]], prompt: str, max_tokens: int = 800) -> tuple[str, dict]:
    key = os.environ.get("ANTHROPIC_API_KEY")
    if not key:
        raise SystemExit("ANTHROPIC_API_KEY not set")
    content = [{"type": "image", "source": {"type": "base64", "media_type": m, "data": d}} for m, d in images]
    content.append({"type": "text", "text": prompt})
    body = json.dumps({"model": model, "max_tokens": max_tokens, "messages": [{"role": "user", "content": content}]}).encode()
    req = urllib.request.Request(
        "https://api.anthropic.com/v1/messages", data=body, method="POST",
        headers={"x-api-key": key, "anthropic-version": "2023-06-01", "content-type": "application/json"},
    )
    for attempt in range(5):
        try:
            with urllib.request.urlopen(req, timeout=300) as r:
                d = json.loads(r.read())
            text = "".join(c.get("text", "") for c in d.get("content", []) if c.get("type") == "text")
            return text, d.get("usage", {})
        except urllib.error.HTTPError as e:
            msg = e.read().decode(errors="replace")
            if e.code in (429, 500, 529) and attempt < 4:
                time.sleep(2 ** attempt * 3)
                continue
            raise RuntimeError(f"anthropic {e.code}: {msg[:300]}") from None
    raise RuntimeError("unreachable")


def answer_one(root: Path, q: Question, path: Path, frames: int, model: str, use_transcript: bool) -> dict:
    t = time.time()
    try:
        imgs = grids(path, frames)
        tr = transcript_text(root, q.video_key) if use_transcript else ""
        prompt = (
            f"These are {frames} frames sampled uniformly over the whole video, each labelled with its timestamp."
            + (f"\n\nTranscript (may be auto-generated):\n{tr}\n\n" if tr else "\n\n")
            + q.prompt(tools=False)
        )
        text, usage = call_anthropic(model, imgs, prompt)
    except Exception as e:  # noqa: BLE001
        return {"id": q.id, "status": "failed", "error": str(e)[:300], "ms": int((time.time() - t) * 1000)}
    pin, pout = PRICES.get(model, (3.0, 15.0))
    tin, tout = usage.get("input_tokens", 0), usage.get("output_tokens", 0)
    letter = parse_letter(text, q.letters)
    return {
        "id": q.id, "status": "ok", "video_key": q.video_key, "task_types": q.task_types, "video_type": q.video_type,
        "answer": q.answer, "predicted": letter, "correct": letter == q.answer, "parsed": letter is not None,
        "text": text.strip()[-600:], "citations": [], "tools": [],
        "usage": {"tokens_in": tin, "tokens_out": tout, "cost_usd": tin / 1e6 * pin + tout / 1e6 * pout, "tool_calls": 0, "provider_calls": 1},
        "partial": False, "ms": int((time.time() - t) * 1000), "time_reference": q.time_reference,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("benchmark")
    ap.add_argument("--root", required=True)
    ap.add_argument("--frames", type=int, default=32)
    ap.add_argument("--model", default="claude-sonnet-5")
    ap.add_argument("--no-transcript", action="store_true")
    ap.add_argument("--sample", type=int)
    ap.add_argument("--fraction", type=float)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--jobs", type=int, default=3)
    ap.add_argument("--limit", type=int)
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    root = Path(a.root)
    files = {p.stem: p for p in (root / "videos").glob("*.mp4")}
    # Same question pool as the index-based runners: videos that are indexed.
    vmap_path = root / "video_map.json"
    if vmap_path.is_file():
        mapped = set(json.loads(vmap_path.read_text()))
        files = {k: v for k, v in files.items() if k in mapped}
    pool = load(a.benchmark, root)
    n = sample_size(len(pool), a.sample, a.fraction)
    qs = stratified_sample(pool, n, a.seed) if n else pool
    qs = [q for q in qs if q.video_key in files]
    if a.limit:
        qs = qs[: a.limit]
    out_path = Path(a.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    results: dict[str, dict] = {}
    if a.resume and out_path.is_file():
        for r in json.loads(out_path.read_text()).get("results", []):
            results[r["id"]] = r
    todo = [q for q in qs if q.id not in results]
    config_record = {"benchmark": a.benchmark, "policy": f"uniform-{a.frames}" + ("" if not a.no_transcript else "-notranscript"),
                     "model": a.model, "frames": a.frames, "transcript": not a.no_transcript, "sample": n, "fraction": a.fraction, "seed": a.seed,
                     "started": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}

    def flush():
        out_path.write_text(json.dumps({"config": config_record, "n_questions": len(qs), "results": list(results.values())}, indent=1))

    done = 0
    with ThreadPoolExecutor(max_workers=a.jobs) as ex:
        futs = {ex.submit(answer_one, root, q, files[q.video_key], a.frames, a.model, not a.no_transcript): q for q in todo}
        for fut in as_completed(futs):
            r = fut.result()
            results[r["id"]] = r
            done += 1
            print(f"[{done}/{len(todo)}] {r['id']} {'OK ' if r.get('correct') else 'MISS'} pred={r.get('predicted')} gt={r.get('answer')} ${r.get('usage', {}).get('cost_usd', 0):.3f}", flush=True)
            if done % 5 == 0:
                flush()
    flush()
    scored = [r for r in results.values() if r.get("status") == "ok"]
    print(json.dumps({"n": len(scored), "accuracy": round(sum(r["correct"] for r in scored) / max(1, len(scored)), 3),
                      "cost_usd_total": round(sum(r["usage"]["cost_usd"] for r in scored), 3)}, indent=1))


if __name__ == "__main__":
    main()
