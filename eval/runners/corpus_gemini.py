#!/usr/bin/env python3
"""Gemini agentic video over a whole library, as close as the API allows.

Gemini's Interactions API takes at most 10 video files per request, and a
36-hour library does not fit a 1M-token context in static mode, so each
question is answered map-reduce style: the library is split into batches of
`--batch-size` videos (10 by default), every batch goes to one
`interactions.create` with all its videos marked `processing: "agentic"`, the
same question and a catalog of the attached titles; the partial answers are
then merged by a text-only call to the same model. Batches run in parallel,
so the question's latency is the slowest batch plus the merge; the run file
also keeps the sequential sum. Cost is the sum of every call at list prices.

    set -a; source .env; set +a
    python3 -m eval.runners.corpus_gemini --model gemini-3.8-flash --jobs 2 \
        --out /data/videoindex/eval/runs/corpus-gemini-agentic.json

Files are uploaded once through the Files API (cache in --file-cache; 48 h
expiry, about 5.4 GB for the 30 videos against the 20 GB project quota).
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
from gemini_video_eval import API, gemini_cost, output_text, request, step_counts, upload  # noqa: E402

from ..datasets.corpus import CorpusQuestion, catalog, load  # noqa: E402

MEDIA = Path("/data/videoindex/videos")


def hms(s: float) -> str:
    s = int(s)
    return f"{s // 3600:02d}:{s % 3600 // 60:02d}:{s % 60:02d}"


def interact(body: dict, timeout: int = 900) -> dict:
    _, _, resp = request("POST", f"{API}/v1beta/interactions", body=json.dumps(body).encode(),
                         headers={"Content-Type": "application/json"}, timeout=timeout)
    d = json.loads(resp)
    while d.get("status") in ("queued", "in_progress") and d.get("id"):
        time.sleep(3)
        _, _, resp = request("GET", f"{API}/v1beta/interactions/{d['id']}")
        d = json.loads(resp)
    return d


def usage_summary(d: dict, price_in: float, price_out: float) -> dict:
    u = d.get("usage", {}) or {}
    return {"tokens_in": (u.get("total_input_tokens", 0) or 0) + (u.get("total_tool_use_tokens", 0) or 0),
            "tokens_out": (u.get("total_output_tokens", 0) or 0) + (u.get("total_thought_tokens", 0) or 0),
            "tokens_tool_use": u.get("total_tool_use_tokens", 0) or 0, "tokens_thought": u.get("total_thought_tokens", 0) or 0,
            "cost_usd": round(gemini_cost(u, price_in, price_out), 5), "tool_calls": step_counts(d).get("processing_call", 0), "raw": u}


def batch_prompt(videos: list[dict], first: int, total: int, q: CorpusQuestion) -> str:
    lines = [f"Video {first + i}: {v['title']} ({hms(v['duration_s'])})" for i, v in enumerate(videos)]
    return (
        f"You are given {len(videos)} videos, numbers {first} to {first + len(videos) - 1} of a library of {total} talks. "
        "They are attached in this order:\n" + "\n".join(lines) + "\n\n"
        "Answer the question below for these attached videos only, referring to each video by its exact title from the "
        "list above. Timestamps must be positions within that video.\n\n" + q.prompt()
    )


def merge_prompt(parts: list[tuple[str, str]], total: int, q: CorpusQuestion) -> str:
    body = "\n\n".join(f"### Partial answer for {rng}\n{txt.strip() or '(no answer)'}" for rng, txt in parts)
    return (
        f"A library of {total} talks was searched in {len(parts)} parts and each part was answered separately. "
        "Combine the partial answers below into one final answer to the question, in the same format the question asks "
        "for. Keep every video that a partial answer found evidence for, with its title, timestamps and quote; drop "
        "videos a partial answer explicitly reported as irrelevant; do not add anything that is not in the partial answers. "
        "For summary or ranking questions, synthesise across the parts. Recompute the final 'Videos: N' line.\n\n"
        f"## Question\n{q.prompt()}\n\n## Partial answers\n{body}"
    )


def run_batch(model: str, files: list[dict], videos: list[dict], first: int, total: int, q: CorpusQuestion, price_in: float, price_out: float, thinking: str | None) -> dict:
    """One agentic call over a batch; splits the batch in two on a request
    error (size limits), so a question always gets an answer for every video."""
    t = time.time()
    items = [{"type": "video", "uri": f["uri"], "mime_type": f["mime_type"], "processing": "agentic"} for f in files]
    body = {"model": model, "input": items + [{"type": "text", "text": batch_prompt(videos, first, total, q)}], "store": False}
    if thinking:
        body["generation_config"] = {"thinking_level": thinking}
    try:
        d = interact(body)
    except Exception as e:  # noqa: BLE001
        if len(files) > 1:
            h = len(files) // 2
            a = run_batch(model, files[:h], videos[:h], first, total, q, price_in, price_out, thinking)
            b = run_batch(model, files[h:], videos[h:], first + h, total, q, price_in, price_out, thinking)
            return {"range": a["range"].split("-")[0] + "-" + b["range"].split("-")[-1], "split": [a, b],
                    "text": a["text"] + "\n\n" + b["text"], "ms": int((time.time() - t) * 1000),
                    "usage": {k: a["usage"][k] + b["usage"][k] for k in ("tokens_in", "tokens_out", "tokens_tool_use", "tokens_thought", "cost_usd", "tool_calls")},
                    "status": "ok" if a["status"] == b["status"] == "ok" else "partial-failure"}
        return {"range": f"{first}-{first + len(files) - 1}", "status": "request-failed", "error": str(e)[:500], "text": "",
                "usage": {k: 0 for k in ("tokens_in", "tokens_out", "tokens_tool_use", "tokens_thought", "cost_usd", "tool_calls")}, "ms": int((time.time() - t) * 1000)}
    return {"range": f"{first}-{first + len(files) - 1}", "status": "ok" if d.get("status") in (None, "completed") else str(d.get("status")),
            "text": output_text(d), "usage": usage_summary(d, price_in, price_out), "steps": step_counts(d), "ms": int((time.time() - t) * 1000)}


def answer_one(q: CorpusQuestion, cat: list[dict], files: dict[str, dict], a) -> dict:
    total = len(cat)
    batches = [cat[i:i + a.batch_size] for i in range(0, total, a.batch_size)]
    t = time.time()
    with ThreadPoolExecutor(max_workers=len(batches)) as ex:
        futs = []
        first = 1
        for b in batches:
            futs.append(ex.submit(run_batch, a.model, [files[v["key"]] for v in b], b, first, total, q, a.price_in, a.price_out, a.thinking))
            first += len(b)
        parts = [f.result() for f in futs]
    t_merge = time.time()
    body = {"model": a.model, "input": [{"type": "text", "text": merge_prompt([(p["range"], p["text"]) for p in parts], total, q)}], "store": False}
    try:
        d = interact(body)
        merged = output_text(d)
        merge_usage = usage_summary(d, a.price_in, a.price_out)
        status = "ok" if all(p["status"] == "ok" for p in parts) else "partial-failure"
    except Exception as e:  # noqa: BLE001
        merged, merge_usage, status = "\n\n".join(p["text"] for p in parts), {k: 0 for k in ("tokens_in", "tokens_out", "tokens_tool_use", "tokens_thought", "cost_usd", "tool_calls")}, "merge-failed:" + str(e)[:200]
    ms_total = int((time.time() - t) * 1000)
    usage = {k: sum(p["usage"][k] for p in parts) + merge_usage[k] for k in ("tokens_in", "tokens_out", "tokens_tool_use", "tokens_thought", "cost_usd", "tool_calls")}
    usage["cost_usd"] = round(usage["cost_usd"], 5)
    usage["provider_calls"] = len(parts) + 1
    usage["wallclock_ms"] = ms_total
    return {"id": q.id, "kind": q.kind, "status": status, "text": merged.strip(), "citations": [], "cited_keys": [],
            "tools": [f"agentic_batch×{len(parts)}", "merge"], "usage": usage, "partial": False, "ms": ms_total,
            "ms_sequential": sum(p["ms"] for p in parts) + int((time.time() - t_merge) * 1000),
            "batches": [{k: v for k, v in p.items() if k != "text"} | {"text": p["text"][-3000:]} for p in parts], "merge_usage": merge_usage}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--index", default="/data/videoindex/indexes/dataset.vidx", help="index whose catalog names the library (the videos come from the media cache)")
    ap.add_argument("--model", default="gemini-3.8-flash")
    ap.add_argument("--thinking", choices=["minimal", "low", "medium", "high"])
    ap.add_argument("--batch-size", type=int, default=10)
    ap.add_argument("--price-in", type=float, default=0.75)
    ap.add_argument("--price-out", type=float, default=3.75)
    ap.add_argument("--jobs", type=int, default=2, help="questions in flight (each one opens batch-count + 1 interactions)")
    ap.add_argument("--only")
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--file-cache", default="/data/videoindex/eval/gemini-files.json")
    ap.add_argument("--label")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    qs = load()
    if a.only:
        keep = set(a.only.split(","))
        qs = [q for q in qs if q.id in keep]
    cat = catalog(a.index)
    cache_path = Path(a.file_cache)
    cache = json.loads(cache_path.read_text()) if cache_path.is_file() else {}
    files: dict[str, dict] = {}
    for v in cat:
        p = MEDIA / f"{v['content_hash']}.mp4"
        if not p.is_file():
            sys.exit(f"missing media {p}")
        files[v["key"]] = upload(p, cache, cache_path)
    print(f"{len(files)} videos on the Files API", file=sys.stderr)
    out_path = Path(a.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    results: dict[str, dict] = {}
    if a.resume and out_path.is_file():
        for r in json.loads(out_path.read_text()).get("results", []):
            if r.get("status") == "ok" and r.get("text"):
                results[r["id"]] = r
    todo = [q for q in qs if q.id not in results]
    n_batches = -(-len(cat) // a.batch_size)
    config_record = {"benchmark": "corpus", "system": "gemini", "policy": "gemini-agentic-mapreduce", "model": a.model, "thinking": a.thinking,
                     "batch_size": a.batch_size, "batches": n_batches, "label": a.label or f"Gemini agentic video ({a.model}, {n_batches}×{a.batch_size} videos + merge)",
                     "price_in": a.price_in, "price_out": a.price_out, "started": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}
    print(f"{len(qs)} questions ({len(todo)} to run) with {a.model}, {n_batches} batches of {a.batch_size}", file=sys.stderr)

    def flush():
        out_path.write_text(json.dumps({"config": config_record, "catalog": cat, "n_questions": len(qs), "results": [results[k] for k in sorted(results)]}, indent=1, ensure_ascii=False))

    done = 0
    with ThreadPoolExecutor(max_workers=a.jobs) as ex:
        futs = {ex.submit(answer_one, q, cat, files, a): q for q in todo}
        for fut in as_completed(futs):
            r = fut.result()
            results[r["id"]] = r
            done += 1
            print(f"[{done}/{len(todo)}] {r['id']} {r['status']:10s} ${r['usage']['cost_usd']:.3f} {r['ms']/1000:.0f}s (seq {r['ms_sequential']/1000:.0f}s) "
                  f"tool_calls={r['usage']['tool_calls']} tok_in={r['usage']['tokens_in']} thought={r['usage']['tokens_thought']}", flush=True)
            flush()
    flush()
    ok = [r for r in results.values() if r["status"] == "ok"]
    print(json.dumps({"n": len(ok), "not_ok": len(results) - len(ok), "cost_usd_total": round(sum(r["usage"]["cost_usd"] for r in results.values()), 3)}, indent=1))


if __name__ == "__main__":
    main()
