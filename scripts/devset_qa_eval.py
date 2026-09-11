#!/usr/bin/env python3
"""Score `vi ask` on the QA dev set (dataset/devset_qa.jsonl).

    scripts/devset_qa_eval.py <index.vidx> --config FILE [--k 8] [--policy agent|retrieval-only]
                              [--budget-usd 0.3] [--limit N] [--json OUT]

Each question has `accept` strings; an answer is correct when any appears in
the answer text (case-insensitive). A citation is correct when it names the
question's video and lies within 90 s of the anchor (resolved from captions).
Reports accuracy, citation rate, correct-citation rate, cost and latency.
"""
import argparse, json, os, subprocess, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from devset_digest import find_srt, parse_srt  # noqa: E402
from devset_eval import video_map  # noqa: E402


def anchor_time(vid, anchor):
    path = find_srt(vid)
    if not path:
        return None
    needle = anchor.lower()
    for t, txt in parse_srt(path):
        if needle in txt.lower():
            return t
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("index")
    ap.add_argument("--config")
    ap.add_argument("--policy", default="agent")
    ap.add_argument("--budget-usd", type=float, default=0.3)
    ap.add_argument("--max-tool-calls", type=int, default=6)
    ap.add_argument("--limit", type=int)
    ap.add_argument("--devset", default="dataset/devset_qa.jsonl")
    ap.add_argument("--vi", default="target/release/vi")
    ap.add_argument("--json")
    a = ap.parse_args()

    ulid_to_yt = video_map(a.vi, a.config, a.index)
    yt_to_ulid = {v: k for k, v in ulid_to_yt.items() if v}
    rows = [json.loads(l) for l in open(a.devset) if l.strip()]
    if a.limit:
        rows = rows[: a.limit]
    results = []
    for q in rows:
        ulid = yt_to_ulid.get(q["video"])
        if ulid is None:
            results.append({**q, "status": "video-not-indexed"})
            continue
        t_anchor = anchor_time(q["video"], q["anchor"]) if q.get("anchor") else None
        cmd = [a.vi] + (["--config", a.config] if a.config else []) + [
            "ask", a.index, q["question"], "--json", "--policy", a.policy,
            "--budget-usd", str(a.budget_usd), "--max-tool-calls", str(a.max_tool_calls)]
        t = time.time()
        proc = subprocess.run(cmd, capture_output=True, text=True)
        ms = int((time.time() - t) * 1000)
        if proc.returncode != 0:
            results.append({**q, "status": "ask-failed", "error": proc.stderr[-300:], "ms": ms})
            continue
        text, cites, usage, partial, tools = "", [], {}, False, []
        for line in proc.stdout.splitlines():
            try:
                ev = json.loads(line)
            except json.JSONDecodeError:
                continue
            if ev["type"] == "token":
                text += ev["text"]
            elif ev["type"] == "citation":
                cites.append(ev)
            elif ev["type"] == "tool_call":
                tools.append(ev["tool"])
            elif ev["type"] == "done":
                usage, partial = ev["usage"], ev["partial"]
        low = text.lower()
        correct = any(acc.lower() in low for acc in q["accept"])
        cite_video_ok = any(c["video_id"] == ulid for c in cites)
        cite_time_ok = t_anchor is not None and any(
            c["video_id"] == ulid and c["t0"] - 90 <= t_anchor <= c["t1"] + 90 for c in cites)
        results.append({**q, "status": "ok", "correct": correct, "cited": bool(cites),
                        "cite_video_ok": cite_video_ok, "cite_time_ok": cite_time_ok,
                        "answer": text.strip()[:300], "tools": tools, "usage": usage, "partial": partial, "ms": ms})
        print(f"{q['id']} {'OK ' if correct else 'MISS'} cite={'V' if cite_video_ok else '-'}{'T' if cite_time_ok else '-'} ${usage.get('cost_usd', 0):.3f} {ms/1000:.1f}s :: {text.strip()[:110]!r}", flush=True)

    scored = [r for r in results if r["status"] == "ok"]
    n = len(scored) or 1
    report = {
        "n": len(scored),
        "accuracy": round(sum(r["correct"] for r in scored) / n, 3),
        "cited": round(sum(r["cited"] for r in scored) / n, 3),
        "cite_video_ok": round(sum(r["cite_video_ok"] for r in scored) / n, 3),
        "cite_time_ok": round(sum(r["cite_time_ok"] for r in scored) / n, 3),
        "partial": sum(r["partial"] for r in scored),
        "cost_usd_total": round(sum(r["usage"].get("cost_usd", 0) for r in scored), 4),
        "cost_usd_mean": round(sum(r["usage"].get("cost_usd", 0) for r in scored) / n, 4),
        "tokens_mean": round(sum(r["usage"].get("tokens_in", 0) + r["usage"].get("tokens_out", 0) for r in scored) / n),
        "tool_calls_mean": round(sum(r["usage"].get("tool_calls", 0) for r in scored) / n, 2),
        "p50_s": round(sorted(r["ms"] for r in scored)[len(scored) // 2] / 1000, 1) if scored else None,
        "not_scored": [(r["id"], r["status"]) for r in results if r["status"] != "ok"],
    }
    print(json.dumps(report, indent=1))
    if a.json:
        json.dump({"report": report, "results": results}, open(a.json, "w"), indent=1)


if __name__ == "__main__":
    main()
