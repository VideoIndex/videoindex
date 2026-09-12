#!/usr/bin/env python3
"""Gemini agentic video as a configuration: each question goes to
`interactions.create` with the video file (uploaded once through the Files
API) and `processing: "agentic"`, mirroring Google's own setup in the
agentic-video announcement. Same sample, prompt shape, letter parser and run
file format as the other runners, so it lands on the same table and plot.

    export GEMINI_API_KEY=...   # or `set -a; source .env; set +a`
    python3 -m eval.runners.gemini lvbench --root /data/videoindex/eval/lvbench --model gemini-3.8-flash \
        --fraction 0.25 --seed 1 --jobs 2 --out /data/videoindex/eval/runs/lvbench-f0.25-s1-gemini-agentic.json

`--mode static` sends the whole video instead (about 300 tokens per second of
video at default resolution; expensive on hour-long inputs). Cost uses list
prices: prompt text and loaded media at --price-in, cached tokens at $0.075/M,
output and thinking at --price-out.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from gemini_video_eval import gemini_cost, interact, output_text, step_counts, upload  # noqa: E402

from ..datasets import Question, load, sample_size, stratified_sample  # noqa: E402
from .answer import parse_letter  # noqa: E402


def answer_one(q: Question, video: dict, model: str, mode: str, thinking: str | None, price_in: float, price_out: float) -> dict:
    t = time.time()
    prompt = q.prompt(tools=False).replace("about this video (do not ask for more)", "in this video")
    try:
        d = interact(model, video, prompt, mode, thinking)
    except Exception as e:  # noqa: BLE001
        return {"id": q.id, "status": "request-failed", "error": str(e)[:400], "ms": int((time.time() - t) * 1000)}
    ms = int((time.time() - t) * 1000)
    text = output_text(d)
    usage = d.get("usage", {}) or {}
    letter = parse_letter(text, q.letters)
    tin = (usage.get("total_input_tokens", 0) or 0) + (usage.get("total_tool_use_tokens", 0) or 0)
    tout = (usage.get("total_output_tokens", 0) or 0) + (usage.get("total_thought_tokens", 0) or 0)
    return {
        "id": q.id, "status": "ok" if d.get("status") in (None, "completed") else str(d.get("status")),
        "video_key": q.video_key, "task_types": q.task_types, "video_type": q.video_type,
        "answer": q.answer, "predicted": letter, "correct": letter == q.answer, "parsed": letter is not None,
        "text": text.strip()[-600:], "citations": [], "tools": [f"processing_call×{step_counts(d).get('processing_call', 0)}"] if mode == "agentic" else [],
        "usage": {"tokens_in": tin, "tokens_out": tout, "cost_usd": round(gemini_cost(usage, price_in, price_out), 5),
                  "tool_calls": step_counts(d).get("processing_call", 0), "provider_calls": 1, "raw": usage},
        "partial": False, "ms": ms, "time_reference": q.time_reference,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("benchmark")
    ap.add_argument("--root", required=True)
    ap.add_argument("--model", default="gemini-3.8-flash")
    ap.add_argument("--mode", choices=["agentic", "static"], default="agentic")
    ap.add_argument("--thinking", choices=["minimal", "low", "medium", "high"])
    ap.add_argument("--price-in", type=float, default=0.75)
    ap.add_argument("--price-out", type=float, default=3.75)
    ap.add_argument("--sample", type=int)
    ap.add_argument("--fraction", type=float)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--jobs", type=int, default=2)
    ap.add_argument("--limit", type=int)
    ap.add_argument("--only-indexed", action="store_true", default=True,
                    help="restrict to videos in video_map.json so the question set matches the index-based runs (default)")
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--file-cache", default="/data/videoindex/eval/gemini-files.json")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    root = Path(a.root)
    files = {p.stem: p for p in (root / "videos").glob("*.mp4")}
    if a.only_indexed and (root / "video_map.json").is_file():
        mapped = set(json.loads((root / "video_map.json").read_text()))
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
            if r.get("status") == "ok":
                results[r["id"]] = r
    todo = [q for q in qs if q.id not in results]
    cache_path = Path(a.file_cache)
    cache = json.loads(cache_path.read_text()) if cache_path.is_file() else {}
    config_record = {"benchmark": a.benchmark, "policy": f"gemini-{a.mode}", "model": a.model, "mode": a.mode, "thinking": a.thinking,
                     "sample": n, "fraction": a.fraction, "seed": a.seed, "started": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}
    print(f"{len(qs)} questions ({len(todo)} to run) with {a.model} {a.mode}", file=sys.stderr)

    def flush():
        out_path.write_text(json.dumps({"config": config_record, "n_questions": len(qs), "results": list(results.values())}, indent=1))

    # Upload each needed video once, serially (uploads are large), before the questions fan out.
    videos: dict[str, dict] = {}
    for key in sorted({q.video_key for q in todo}):
        try:
            videos[key] = upload(files[key], cache, cache_path)
        except Exception as e:  # noqa: BLE001
            print(f"upload failed for {key}: {str(e)[:200]}", file=sys.stderr)
    done = 0
    with ThreadPoolExecutor(max_workers=a.jobs) as ex:
        futs = {ex.submit(answer_one, q, videos[q.video_key], a.model, a.mode, a.thinking, a.price_in, a.price_out): q for q in todo if q.video_key in videos}
        for fut in as_completed(futs):
            r = fut.result()
            results[r["id"]] = r
            done += 1
            print(f"[{done}/{len(futs)}] {r['id']} {'OK ' if r.get('correct') else ('MISS' if r.get('status') == 'ok' else 'ERR ')} pred={r.get('predicted')} gt={r.get('answer')} ${r.get('usage', {}).get('cost_usd', 0):.3f} {r.get('ms', 0)/1000:.1f}s", flush=True)
            if done % 5 == 0:
                flush()
    flush()
    scored = [r for r in results.values() if r.get("status") == "ok"]
    print(json.dumps({"n": len(scored), "accuracy": round(sum(r["correct"] for r in scored) / max(1, len(scored)), 3),
                      "unparsed": sum(not r["parsed"] for r in scored),
                      "cost_usd_total": round(sum(r["usage"]["cost_usd"] for r in scored), 3)}, indent=1))


if __name__ == "__main__":
    main()
