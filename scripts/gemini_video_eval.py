#!/usr/bin/env python3
"""Run the QA dev set against Gemini's agentic video understanding, scored
exactly like `devset_qa_eval.py`, so the two systems are compared on the
same questions over the same video files.

    export GEMINI_API_KEY=...
    scripts/gemini_video_eval.py --model gemini-3.8-flash --json /data/videoindex/eval/qa-gemini-agentic.json
    scripts/gemini_video_eval.py --mode static ...        # whole video, default sampling

Each question is one `interactions.create` call with the video file (uploaded
once through the Files API, URIs cached in --file-cache for 40 hours) and the
question. `processing: "agentic"` lets Gemini navigate the video itself; the
static mode feeds the whole video. Scoring: an answer is correct when it
contains an accepted string (case-insensitive); a citation is any
[HH:MM:SS] / MM:SS timestamp in the answer, and it is "in time" when it lies
within 90 s of the caption anchor, matching the VideoIndex eval. Cost uses
the paid-tier list prices in --price-in/--price-out per million tokens; the
default is Gemini 3.x Flash through 2026-12-31 ($0.75 in, $3.75 out, thought
tokens billed as output).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from devset_qa_eval import anchor_time  # noqa: E402

API = "https://generativelanguage.googleapis.com"
VIDEOS = Path("/data/videoindex/videos")
PROMPT = (
    "You are answering a question about this video for a viewer who will jump to the moment you cite. "
    "Answer in one or two sentences, then cite the timestamp(s) where the answer is shown or said, "
    "in the form [HH:MM:SS]. If the video does not answer the question, say so.\n\nQuestion: {q}"
)
TS_RE = re.compile(r"\[?(?:(\d{1,2}):)?(\d{1,2}):(\d{2})\]?")


def key() -> str:
    k = os.environ.get("GEMINI_API_KEY")
    if not k:
        sys.exit("GEMINI_API_KEY is not set")
    return k


def request(method: str, url: str, body: bytes | None = None, headers: dict | None = None, timeout=600):
    h = {"x-goog-api-key": key()}
    h.update(headers or {})
    req = urllib.request.Request(url, data=body, method=method, headers=h)
    for attempt in range(6):
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                return r.status, dict(r.headers), r.read()
        except urllib.error.HTTPError as e:
            text = e.read().decode(errors="replace")
            if e.code in (429, 500, 502, 503, 504) and attempt < 5:
                wait = min(60, 2 ** attempt * 3)
                print(f"  http {e.code}, retry in {wait}s: {text[:120]}", file=sys.stderr)
                time.sleep(wait)
                continue
            raise RuntimeError(f"http {e.code}: {text[:800]}") from None
    raise RuntimeError("unreachable")


def yt_to_file() -> dict[str, Path]:
    out = {}
    for info in VIDEOS.glob("*.info.json"):
        try:
            d = json.load(open(info))
        except json.JSONDecodeError:
            continue
        mp4 = info.with_name(info.name.replace(".info.json", ".mp4"))
        if mp4.is_file() and d.get("id"):
            out[d["id"]] = mp4
    return out


def upload(path: Path, cache: dict, cache_path: Path) -> dict:
    """Resumable Files API upload; returns {"uri", "name", "mime_type", "uploaded_at"}."""
    ent = cache.get(str(path))
    if ent and time.time() - ent["uploaded_at"] < 40 * 3600:
        status, _, body = request("GET", f"{API}/v1beta/{ent['name']}")
        if status == 200 and json.loads(body).get("state") == "ACTIVE":
            return ent
    size = path.stat().st_size
    print(f"  uploading {path.name} ({size/1e6:.0f} MB)", file=sys.stderr)
    status, headers, _ = request(
        "POST", f"{API}/upload/v1beta/files",
        body=json.dumps({"file": {"display_name": path.stem[:24]}}).encode(),
        headers={
            "X-Goog-Upload-Protocol": "resumable", "X-Goog-Upload-Command": "start",
            "X-Goog-Upload-Header-Content-Length": str(size),
            "X-Goog-Upload-Header-Content-Type": "video/mp4", "Content-Type": "application/json",
        })
    upload_url = headers.get("X-Goog-Upload-URL") or headers.get("x-goog-upload-url")
    if not upload_url:
        raise RuntimeError(f"no upload url in {headers}")
    with open(path, "rb") as f:
        data = f.read()
    _, _, body = request(
        "POST", upload_url, body=data,
        headers={"Content-Length": str(size), "X-Goog-Upload-Offset": "0",
                 "X-Goog-Upload-Command": "upload, finalize"}, timeout=1800)
    info = json.loads(body)["file"]
    name = info["name"]
    while info.get("state") != "ACTIVE":
        if info.get("state") == "FAILED":
            raise RuntimeError(f"file processing failed: {info}")
        time.sleep(5)
        _, _, body = request("GET", f"{API}/v1beta/{name}")
        info = json.loads(body)
    ent = {"uri": info["uri"], "name": name, "mime_type": info.get("mimeType", "video/mp4"),
           "uploaded_at": time.time()}
    cache[str(path)] = ent
    cache_path.write_text(json.dumps(cache, indent=1))
    return ent


def interact(model: str, video: dict, question: str, mode: str, thinking: str | None) -> dict:
    item = {"type": "video", "uri": video["uri"], "mime_type": video["mime_type"]}
    if mode == "agentic":
        item["processing"] = "agentic"
    body = {"model": model, "input": [item, {"type": "text", "text": PROMPT.format(q=question)}],
            "store": False}
    if thinking:
        body["generation_config"] = {"thinking_level": thinking}
    _, _, resp = request("POST", f"{API}/v1beta/interactions", body=json.dumps(body).encode(),
                         headers={"Content-Type": "application/json"}, timeout=900)
    d = json.loads(resp)
    # Background/async completion: poll until a terminal status.
    while d.get("status") in ("queued", "in_progress") and d.get("id"):
        time.sleep(3)
        _, _, resp = request("GET", f"{API}/v1beta/interactions/{d['id']}")
        d = json.loads(resp)
    return d


def output_text(d: dict) -> str:
    text = []
    for step in d.get("steps", []) or d.get("outputs", []) or []:
        if step.get("type") == "model_output":
            for c in step.get("content", []):
                if c.get("type") == "text":
                    text.append(c.get("text", ""))
    if not text and isinstance(d.get("output_text"), str):
        text.append(d["output_text"])
    return "\n".join(text)


def step_counts(d: dict) -> dict:
    counts: dict[str, int] = {}
    for step in d.get("steps", []) or []:
        counts[step.get("type", "?")] = counts.get(step.get("type", "?"), 0) + 1
    return counts


def gemini_cost(usage: dict, price_in: float, price_out: float, price_cached: float = 0.075) -> float:
    """List-price estimate from an Interactions usage object. Prompt text and the
    frames/transcript the agent loads (`total_tool_use_tokens`) are billed as
    input, cached tokens at the cache rate, output and thinking as output."""
    tin = (usage.get("total_input_tokens", 0) or 0) + (usage.get("total_tool_use_tokens", 0) or 0)
    tcached = usage.get("total_cached_tokens", 0) or 0
    tout = (usage.get("total_output_tokens", 0) or 0) + (usage.get("total_thought_tokens", 0) or 0)
    return tin / 1e6 * price_in + tcached / 1e6 * price_cached + tout / 1e6 * price_out


def timestamps(text: str) -> list[float]:
    out = []
    for h, m, s in TS_RE.findall(text):
        out.append((int(h) if h else 0) * 3600 + int(m) * 60 + int(s))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--model", default="gemini-3.8-flash")
    ap.add_argument("--mode", choices=["agentic", "static"], default="agentic")
    ap.add_argument("--thinking", choices=["minimal", "low", "medium", "high"])
    ap.add_argument("--devset", default="dataset/devset_qa.jsonl")
    ap.add_argument("--limit", type=int)
    ap.add_argument("--only", help="comma-separated question ids")
    ap.add_argument("--file-cache", default="/data/videoindex/eval/gemini-files.json")
    ap.add_argument("--price-in", type=float, default=0.75, help="USD per 1M input tokens")
    ap.add_argument("--price-out", type=float, default=3.75, help="USD per 1M output (and thought) tokens")
    ap.add_argument("--json")
    a = ap.parse_args()

    files = yt_to_file()
    cache_path = Path(a.file_cache)
    cache = json.loads(cache_path.read_text()) if cache_path.is_file() else {}
    rows = [json.loads(l) for l in open(a.devset) if l.strip()]
    if a.only:
        keep = set(a.only.split(","))
        rows = [r for r in rows if r["id"] in keep]
    if a.limit:
        rows = rows[: a.limit]
    results = []
    for q in rows:
        path = files.get(q["video"])
        if path is None:
            results.append({**q, "status": "video-not-local"})
            continue
        try:
            video = upload(path, cache, cache_path)
        except Exception as e:  # noqa: BLE001
            results.append({**q, "status": "upload-failed", "error": str(e)[:300]})
            print(f"{q['id']} upload failed: {e}", file=sys.stderr)
            continue
        t_anchor = anchor_time(q["video"], q["anchor"]) if q.get("anchor") else None
        t = time.time()
        try:
            d = interact(a.model, video, q["question"], a.mode, a.thinking)
        except Exception as e:  # noqa: BLE001
            results.append({**q, "status": "request-failed", "error": str(e)[:500], "ms": int((time.time() - t) * 1000)})
            print(f"{q['id']} request failed: {str(e)[:200]}", file=sys.stderr)
            continue
        ms = int((time.time() - t) * 1000)
        text = output_text(d)
        usage = d.get("usage", {}) or {}
        tin = usage.get("total_input_tokens", 0) or 0
        tout = usage.get("total_output_tokens", 0) or 0
        tthink = usage.get("total_thought_tokens", 0) or 0
        ttool = usage.get("total_tool_use_tokens", 0) or 0
        tcached = usage.get("total_cached_tokens", 0) or 0
        cost = gemini_cost(usage, a.price_in, a.price_out)
        low = text.lower()
        correct = any(acc.lower() in low for acc in q["accept"])
        ts = timestamps(text)
        cite_time_ok = t_anchor is not None and any(abs(x - t_anchor) <= 90 for x in ts)
        results.append({
            **q, "status": "ok" if d.get("status") in (None, "completed") else d.get("status"),
            "correct": correct, "cited": bool(ts), "cite_video_ok": bool(ts), "cite_time_ok": cite_time_ok,
            "answer": text.strip()[:400], "steps": step_counts(d),
            "usage": {"tokens_in": tin, "tokens_out": tout, "tokens_thought": tthink, "tokens_tool_use": ttool,
                      "tokens_cached": tcached,
                      "total_tokens": usage.get("total_tokens", tin + tout + tthink + ttool), "cost_usd": round(cost, 5),
                      "raw": usage},
            "ms": ms,
        })
        print(f"{q['id']} {'OK ' if correct else 'MISS'} cite={'T' if cite_time_ok else ('V' if ts else '-')} "
              f"${cost:.3f} {ms/1000:.1f}s tok=in{tin}+tool{ttool}+out{tout}+think{tthink} steps={step_counts(d)} :: {text.strip()[:100]!r}", flush=True)
        if a.json:
            json.dump({"model": a.model, "mode": a.mode, "results": results}, open(a.json, "w"), indent=1)

    scored = [r for r in results if r["status"] == "ok"]
    n = len(scored) or 1
    report = {
        "model": a.model, "mode": a.mode, "n": len(scored),
        "accuracy": round(sum(r["correct"] for r in scored) / n, 3),
        "cited": round(sum(r["cited"] for r in scored) / n, 3),
        "cite_video_ok": round(sum(r["cite_video_ok"] for r in scored) / n, 3),
        "cite_time_ok": round(sum(r["cite_time_ok"] for r in scored) / n, 3),
        "cost_usd_total": round(sum(r["usage"]["cost_usd"] for r in scored), 4),
        "cost_usd_mean": round(sum(r["usage"]["cost_usd"] for r in scored) / n, 4),
        "tokens_mean": round(sum(r["usage"]["total_tokens"] for r in scored) / n),
        "tokens_in_mean": round(sum(r["usage"]["tokens_in"] for r in scored) / n),
        "p50_s": round(sorted(r["ms"] for r in scored)[len(scored) // 2] / 1000, 1) if scored else None,
        "not_scored": [(r["id"], r["status"]) for r in results if r["status"] != "ok"],
    }
    print(json.dumps(report, indent=1))
    if a.json:
        json.dump({"report": report, "model": a.model, "mode": a.mode, "results": results}, open(a.json, "w"), indent=1)


if __name__ == "__main__":
    main()
