#!/usr/bin/env python3
"""Answer a benchmark's questions with `vidx ask` under one configuration.

    python3 -m eval.runners.answer lvbench --root /data/videoindex/eval/lvbench \
        --index /data/videoindex/indexes/eval-lvbench.vidx --config config/gcp-a100.toml \
        --policy agent --max-tool-calls 6 --budget-usd 0.3 --sample 300 --seed 1 --jobs 4 \
        --out /data/videoindex/eval/runs/lvbench-agent.json

Each question goes to `vidx ask --json --video <id>` with the multiple-choice
prompt; the run file keeps the events, usage, parsed letter and correctness.
`--sample N` takes a stratified sample (by task type, then video) so a subset
run is representative; `--resume` skips questions already in `--out`.
"""
from __future__ import annotations

import argparse
import json
import random
import re
import subprocess
import sys
import time
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

from ..datasets import Question, load, sample_size, stratified_sample

ANSWER_RE = re.compile(r"answer\s*[:\-]?\s*\(?\s*([A-H])\s*\)?\s*\.?\s*$", re.I | re.M)
LONE_RE = re.compile(r"(?:^|\s)\(?([A-H])\)?\s*\.?\s*$")


def parse_letter(text: str, letters: list[str], options: list[str] | None = None) -> str | None:
    """The chosen option letter. Prefers an explicit `Answer: X`, then a lone
    letter on the last line, then `(X)` in the last line. When `options` are
    given and no letter was found, an answer that restates exactly one
    option's text (models do this with long sentence options) counts as
    choosing it; two or more matching options stay unparsed."""
    text = text.strip().replace("**", "")
    m = ANSWER_RE.findall(text)
    if m and m[-1].upper() in letters:
        return m[-1].upper()
    lines = [l for l in text.splitlines() if l.strip()]
    if lines:
        last = lines[-1]
        m2 = LONE_RE.search(last)
        if m2 and m2.group(1).upper() in letters:
            return m2.group(1).upper()
        for l in letters:
            if re.search(rf"\(({l})\)", last):
                return l
    if options:
        norm = lambda s: re.sub(r"[^a-z0-9 ]+", " ", s.lower()).split()  # noqa: E731
        tail = " ".join(norm(text[-800:]))
        hits = [l for l, o in zip(letters, options) if len(norm(o)) >= 4 and " ".join(norm(o)) in tail]
        if len(hits) == 1:
            return hits[0]
    return None


def ask_one(vi: str, config: str | None, index: str, video_id: str, q: Question, policy: str, budget_usd: float, max_tool_calls: int, budget_tokens: int,
            model: str | None = None) -> dict:
    cmd = [vi] + (["--config", config] if config else []) + [
        "ask", index, q.prompt(tools=policy == "agent"), "--json", "--video", video_id, "--policy", policy,
        "--budget-usd", str(budget_usd), "--max-tool-calls", str(max_tool_calls), "--budget-tokens", str(budget_tokens),
    ] + (["--model", model] if model else [])
    t = time.time()
    proc = subprocess.run(cmd, capture_output=True, text=True)
    ms = int((time.time() - t) * 1000)
    if proc.returncode != 0:
        return {"id": q.id, "status": "ask-failed", "error": proc.stderr[-400:], "ms": ms}
    text, cites, tools, calls, usage, partial = "", [], [], [], {}, False
    for line in proc.stdout.splitlines():
        try:
            ev = json.loads(line)
        except json.JSONDecodeError:
            continue
        if ev["type"] == "token":
            text += ev["text"]
        elif ev["type"] == "citation":
            cites.append({"t0": ev["t0"], "t1": ev["t1"], "kind": ev.get("kind")})
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
            usage, partial = ev["usage"], ev["partial"]
    letter = parse_letter(text, q.letters, q.options)
    known = q.answer in q.letters
    return {
        "id": q.id, "status": "ok", "video_key": q.video_key, "task_types": q.task_types, "video_type": q.video_type,
        "answer": q.answer if known else None, "predicted": letter, "correct": (letter == q.answer) if known else None,
        "parsed": letter is not None, "kaggle_row": q.extra.get("kaggle_row"),
        "text": text.strip()[-600:], "citations": cites, "tools": tools, "calls": calls, "usage": usage, "partial": partial, "ms": ms,
        "time_reference": q.time_reference,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("benchmark")
    ap.add_argument("--root", required=True)
    ap.add_argument("--index", required=True)
    ap.add_argument("--config")
    ap.add_argument("--vi", default="target/release/vidx")
    ap.add_argument("--policy", default="agent")
    ap.add_argument("--model", help="chat model for the agent (a [providers.*] name or model id); default: the config's agent_llm role")
    ap.add_argument("--label", help="row label for the report (default: policy + model)")
    ap.add_argument("--budget-usd", type=float, default=0.5)
    ap.add_argument("--budget-tokens", type=int, default=120000)
    ap.add_argument("--retry-empty", type=int, default=1, help="re-ask when the answer text is empty (model ended a forced turn with no content)")
    ap.add_argument("--max-tool-calls", type=int, default=6)
    ap.add_argument("--sample", type=int, help="number of questions (stratified by task type)")
    ap.add_argument("--fraction", type=float, help="fraction of the benchmark, e.g. 0.25 (stratified by task type)")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--limit", type=int)
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--redo-unparsed", action="store_true", help="with --resume, also re-ask questions whose stored answer had no option letter")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    root = Path(a.root)
    pool = load(a.benchmark, root)
    vmap = json.loads((root / "video_map.json").read_text()) if (root / "video_map.json").is_file() else {}
    # Sample from the whole benchmark first, so the same (seed, size) names the
    # same questions for every configuration and every acquisition state; then
    # keep the ones whose video is indexed and report how many are missing.
    n = sample_size(len(pool), a.sample, a.fraction)
    qs = stratified_sample(pool, n, a.seed) if n else pool
    missing = [q for q in qs if q.video_key not in vmap]
    qs = [q for q in qs if q.video_key in vmap]
    if missing:
        print(f"{len(missing)} sampled questions skipped: their videos are not indexed", file=sys.stderr)
    if a.limit:
        qs = qs[: a.limit]
    out_path = Path(a.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    results: dict[str, dict] = {}
    if a.resume and out_path.is_file():
        # Keep scored answers; failed or errored questions are asked again.
        for r in json.loads(out_path.read_text()).get("results", []):
            if r.get("status") == "ok" and not (a.redo_unparsed and not r.get("parsed")):
                results[r["id"]] = r
    todo = [q for q in qs if q.id not in results]
    config_record = {
        "benchmark": a.benchmark, "policy": a.policy, "model": a.model, "label": a.label, "budget_usd": a.budget_usd, "budget_tokens": a.budget_tokens,
        "max_tool_calls": a.max_tool_calls, "sample": n, "fraction": a.fraction, "seed": a.seed, "config": a.config, "index": a.index,
        "skipped_not_indexed": len(missing),
        "vi_version": subprocess.run([a.vi, "--version"], capture_output=True, text=True).stdout.strip(),
        "started": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    print(f"{len(qs)} questions ({len(todo)} to run) with policy={a.policy} model={a.model or 'default'} jobs={a.jobs}", file=sys.stderr)

    def flush():
        out_path.write_text(json.dumps({"config": config_record, "n_questions": len(qs), "results": list(results.values())}, indent=1))

    done = 0
    with ThreadPoolExecutor(max_workers=a.jobs) as ex:
        def ask_with_retry(q: Question) -> dict:
            r = ask_one(a.vi, a.config, a.index, vmap[q.video_key], q, a.policy, a.budget_usd, a.max_tool_calls, a.budget_tokens, a.model)
            tries = 0
            while r.get("status") == "ok" and not r.get("text") and tries < a.retry_empty:
                tries += 1
                again = ask_one(a.vi, a.config, a.index, vmap[q.video_key], q, a.policy, a.budget_usd, a.max_tool_calls, a.budget_tokens, a.model)
                if again.get("status") != "ok":
                    # Keep the first (scored, empty) attempt rather than losing its spend.
                    r["retry_failed"] = again.get("error", again.get("status"))
                    break
                # The question is charged for both attempts, on every measure.
                for k in ("cost_usd", "tokens_in", "tokens_out", "tool_calls", "provider_calls", "wallclock_ms"):
                    again["usage"][k] = again["usage"].get(k, 0) + r["usage"].get(k, 0)
                again["ms"] = again.get("ms", 0) + r.get("ms", 0)
                again["retries"] = tries
                r = again
            return r

        futs = {ex.submit(ask_with_retry, q): q for q in todo}
        for fut in as_completed(futs):
            r = fut.result()
            results[r["id"]] = r
            done += 1
            ok = r.get("correct")
            mark = "PRED" if ok is None and r.get("status") == "ok" else ("OK " if ok else ("MISS" if r.get("status") == "ok" else "ERR "))
            print(f"[{done}/{len(todo)}] {r['id']} {mark} pred={r.get('predicted')} gt={r.get('answer')} ${r.get('usage', {}).get('cost_usd', 0):.3f} {r.get('ms', 0)/1000:.1f}s", flush=True)
            if done % 5 == 0:
                flush()
    flush()
    scored = [r for r in results.values() if r.get("status") == "ok"]
    graded = [r for r in scored if r.get("correct") is not None]
    acc = sum(r["correct"] for r in graded) / max(1, len(graded))
    print(json.dumps({"n": len(scored), "graded": len(graded), "accuracy": round(acc, 3) if graded else None, "unparsed": sum(not r["parsed"] for r in scored),
                      "cost_usd_total": round(sum(r["usage"].get("cost_usd", 0) for r in scored), 3)}, indent=1))


if __name__ == "__main__":
    main()
