#!/usr/bin/env python3
"""Answer the corpus question set with `vidx ask` over a whole index (no
`--video` restriction: the agent must find the videos itself).

    python3 -m eval.runners.corpus --index /data/videoindex/indexes/dataset.vidx --config config/gcp-a100.toml \
        --policy agent --jobs 3 --out /data/videoindex/eval/runs/corpus-agent-sonnet.json
    python3 -m eval.runners.corpus ... --policy agent --model gemini-3.8-flash --out .../corpus-agent-gemini.json
    python3 -m eval.runners.corpus ... --policy retrieval-only --out .../corpus-retrieval-only.json

The run file keeps the answer text, the structured citations (mapped to
YouTube ids through the index catalog), the tool calls, usage and wall-clock
per question. Scoring happens afterwards in `eval.judge`, which treats every
system's answer the same way.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

from ..datasets.corpus import CorpusQuestion, catalog, load


def ask(vi: str, config: str | None, index: str, q: CorpusQuestion, a, key_of: dict[str, str]) -> dict:
    cmd = [vi] + (["--config", config] if config else []) + [
        "ask", index, q.prompt(), "--json", "--policy", a.policy, "--budget-usd", str(a.budget_usd),
        "--budget-tokens", str(a.budget_tokens), "--budget-secs", str(a.budget_secs), "--max-tool-calls", str(a.max_tool_calls),
        "--max-answer-tokens", str(a.max_answer_tokens),
    ] + (["--model", a.model] if a.model else [])
    t = time.time()
    proc = subprocess.run(cmd, capture_output=True, text=True)
    ms = int((time.time() - t) * 1000)
    if proc.returncode != 0:
        return {"id": q.id, "kind": q.kind, "status": "ask-failed", "error": proc.stderr[-600:], "ms": ms}
    text, cites, tools, calls, usage, partial, reason = "", [], [], [], {}, False, None
    for line in proc.stdout.splitlines():
        try:
            ev = json.loads(line)
        except json.JSONDecodeError:
            continue
        if ev["type"] == "token":
            text += ev["text"]
        elif ev["type"] == "citation":
            cites.append({"video_key": key_of.get(ev.get("video_id"), ev.get("video_id")), "t0": ev["t0"], "t1": ev["t1"], "kind": ev.get("kind")})
        elif ev["type"] == "tool_call":
            tools.append(ev["tool"])
            # One record per call: the loop turn that issued it (calls sharing
            # a turn ran concurrently) and, once the result arrives, its wall time.
            args = ev.get("args") or {}
            calls.append({"tool": ev["tool"], "turn": ev.get("turn"), "ms": None,
                          "windows": len(args["windows"]) if isinstance(args.get("windows"), list) else (1 if "t0" in args else 0)})
        elif ev["type"] == "tool_result":
            for c in calls:
                if c["tool"] == ev["tool"] and c["ms"] is None and c["turn"] == ev.get("turn"):
                    c["ms"] = ev.get("ms")
                    break
        elif ev["type"] == "done":
            usage, partial, reason = ev["usage"], ev["partial"], ev.get("reason")
    return {"id": q.id, "kind": q.kind, "status": "ok", "text": text.strip(), "citations": cites,
            "cited_keys": sorted({c["video_key"] for c in cites}), "tools": tools, "calls": calls, "usage": usage, "partial": partial,
            "stop_reason": reason, "ms": ms}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--index", default="/data/videoindex/indexes/dataset.vidx")
    ap.add_argument("--config")
    ap.add_argument("--vi", default="target/release/vidx")
    ap.add_argument("--policy", default="agent", choices=["agent", "retrieval-only"])
    ap.add_argument("--model", help="chat model (a [providers.*] name or model id); default: the config's agent_llm role")
    ap.add_argument("--budget-usd", type=float, default=1.0)
    ap.add_argument("--budget-tokens", type=int, default=200000)
    ap.add_argument("--budget-secs", type=int, default=300)
    ap.add_argument("--max-tool-calls", type=int, default=12)
    ap.add_argument("--max-answer-tokens", type=int, default=4000, help="output tokens per model turn (answer length)")
    ap.add_argument("--jobs", type=int, default=3)
    ap.add_argument("--only", help="comma-separated question ids")
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--label", help="row label for the report (default: policy + model)")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    qs = load()
    if a.only:
        keep = set(a.only.split(","))
        qs = [q for q in qs if q.id in keep]
    cat = catalog(a.index)
    key_of = {v["id"]: v["key"] for v in cat}
    out_path = Path(a.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    results: dict[str, dict] = {}
    if a.resume and out_path.is_file():
        for r in json.loads(out_path.read_text()).get("results", []):
            if r.get("status") == "ok" and r.get("text"):
                results[r["id"]] = r
    todo = [q for q in qs if q.id not in results]
    version = subprocess.run([a.vi, "--version"], capture_output=True, text=True).stdout.strip()
    config_record = {
        "benchmark": "corpus", "system": "videoindex", "policy": a.policy, "model": a.model,
        "label": a.label or f"VideoIndex {a.policy} ({a.model or 'agent_llm default'})",
        "budget_usd": a.budget_usd, "budget_tokens": a.budget_tokens, "budget_secs": a.budget_secs, "max_tool_calls": a.max_tool_calls, "max_answer_tokens": a.max_answer_tokens,
        "index": a.index, "config": a.config, "vi_version": version, "started": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    print(f"{len(qs)} questions ({len(todo)} to run) policy={a.policy} model={a.model or 'default'} jobs={a.jobs}", file=sys.stderr)

    def flush():
        out_path.write_text(json.dumps({"config": config_record, "catalog": cat, "n_questions": len(qs), "results": [results[k] for k in sorted(results)]}, indent=1, ensure_ascii=False))

    done = 0
    with ThreadPoolExecutor(max_workers=a.jobs) as ex:
        futs = {ex.submit(ask, a.vi, a.config, a.index, q, a, key_of): q for q in todo}
        for fut in as_completed(futs):
            r = fut.result()
            results[r["id"]] = r
            done += 1
            u = r.get("usage", {})
            print(f"[{done}/{len(todo)}] {r['id']} {r['status']:10s} ${u.get('cost_usd', 0):.3f} {r.get('ms', 0)/1000:.0f}s tools={len(r.get('tools', []))} "
                  f"cited={len(r.get('cited_keys', []))} {'PARTIAL ' + str(r.get('stop_reason')) if r.get('partial') else ''}", flush=True)
            flush()
    flush()
    ok = [r for r in results.values() if r["status"] == "ok"]
    print(json.dumps({"n": len(ok), "failed": len(results) - len(ok), "cost_usd_total": round(sum(r["usage"].get("cost_usd", 0) for r in ok), 3),
                      "partial": sum(bool(r.get("partial")) for r in ok)}, indent=1))


if __name__ == "__main__":
    main()
