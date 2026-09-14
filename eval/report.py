#!/usr/bin/env python3
"""Tables and a pareto plot for a set of run files on one benchmark.

    python3 -m eval.report lvbench /data/videoindex/eval/runs/lvbench-*.json --out docs/results/lvbench-2026-09-12.md
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import collections
import sys
from pathlib import Path

from .datasets import load
from .metrics import score
from .runners.answer import parse_letter


def rescore(run: dict, questions: dict) -> int:
    """Re-parse answers that had no letter using the option-text fallback
    (added after the first runs); returns how many changed."""
    changed = 0
    for r in run.get("results", []):
        if r.get("status") != "ok" or r.get("parsed"):
            continue
        q = questions.get(r["id"])
        if not q or not r.get("text"):
            continue
        letter = parse_letter(r["text"], q.letters, q.options)
        if letter:
            r["predicted"], r["parsed"], r["correct"] = letter, True, letter == q.answer
            changed += 1
    return changed

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("benchmark")
    ap.add_argument("runs", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--title")
    ap.add_argument("--root", help="benchmark root (default: read from the first run's config or /data/videoindex/eval/<benchmark>)")
    ap.add_argument("--submission", help="also write a Kaggle-style CSV of predictions (row,answer) for benchmarks without public answers")
    a = ap.parse_args()
    rows = []
    questions = {}
    try:
        root = a.root or f"/data/videoindex/eval/{a.benchmark}"
        questions = {q.id: q for q in load(a.benchmark, root)}
    except SystemExit:
        pass
    for path in a.runs:
        run = json.loads(Path(path).read_text())
        if questions:
            n = rescore(run, questions)
            if n:
                Path(path).write_text(json.dumps(run, indent=1))
                print(f"{Path(path).name}: {n} answers matched an option by text", file=sys.stderr)
        s = score(run)
        cfg = run["config"]
        label = cfg.get("policy", Path(path).stem)
        if cfg.get("model") and not label.startswith("gemini"):
            label += f" ({cfg['model']})"
        elif label.startswith("gemini"):
            label = f"{cfg.get('model', 'gemini')} {cfg.get('mode', '')} video".strip()
        rows.append((label, cfg, s, path))
    out = Path(a.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    assets = out.parent / "assets"
    assets.mkdir(exist_ok=True)
    plot = assets / f"{out.stem}-pareto.svg"
    ungraded = [s.get("ungraded") == s["n"] and s["n"] > 0 for _, _, s, _ in rows]
    all_ungraded = bool(rows) and all(ungraded)
    try:
        if all_ungraded:
            raise RuntimeError("predictions-only runs have no accuracy to plot")
        from report import charts  # noqa: PLC0415

        pts = [(label, s["cost_mean"], 100 * s["accuracy"], i % 8) for i, (label, _, s, _) in enumerate(rows)]
        charts.scatter_labeled(plot, pts, title=f"{a.benchmark}: accuracy against cost per question", xlabel="USD per question (list price)", ylabel="accuracy (%)", ylim=(0, 102), figsize=(6.6, 3.8))
        plot_rel = plot.relative_to(out.parent)
    except Exception as e:  # noqa: BLE001
        print(f"warning: no plot ({e})", file=sys.stderr)
        plot_rel = None
    n_q = max((r[2]["n"] for r in rows), default=0)
    md = [f"# {a.title or a.benchmark + ' results'}", "", f"Generated {dt.date.today().isoformat()} by `eval/report.py` from {len(rows)} run file(s). "
          f"Accuracy is exact match on the option letter; intervals are 95% Wilson. Costs are provider list prices per question; "
          f"indexing cost is not included for the index-based configurations.", ""]
    if plot_rel:
        md += [f"![]({plot_rel})", ""]
    md += ["| Configuration | n | accuracy | 95% CI | unparsed | cost / q | tokens / q | tool calls / q | latency p50 | latency p95 | no-decode | citation in range |",
           "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for label, cfg, s, _ in rows:
        cir = f"{100 * s['citation_in_range']:.0f}% (n={s['citation_in_range_n']})" if s["citation_in_range"] is not None else "—"
        if s.get("ungraded") == s["n"] and s["n"]:
            md.append(f"| {label} | {s['n']} | predictions only (no public answers) | — | {s['unparsed']} | ${s['cost_mean']:.3f} | {s['tokens_mean']:,.0f} | {s['tool_calls_mean']:.2f} | {s['latency_p50']:.1f} s | {s['latency_p95']:.1f} s | {100 * s['no_decode_fraction']:.0f}% | — |")
            continue
        md.append(f"| {label} | {s['n']} | **{100 * s['accuracy']:.1f}%** | {100 * s['ci'][0]:.1f}–{100 * s['ci'][1]:.1f} | {s['unparsed']} | ${s['cost_mean']:.3f} | {s['tokens_mean']:,.0f} | {s['tool_calls_mean']:.2f} | {s['latency_p50']:.1f} s | {s['latency_p95']:.1f} s | {100 * s['no_decode_fraction']:.0f}% | {cir} |")
    if any(l.startswith("gemini") for l, _, _, _ in rows):
        md += ["", "## Where this stands against Gemini agentic video", "",
               "The `gemini-…` row is Google's Gemini 3.8 Flash with `processing: \"agentic\"` asked the same questions over the same "
               "video files through the Interactions API (run once and cached; it is not re-run with every matrix). Google's own "
               "announcement (\"Introducing agentic video in Gemini\", 2026-09-01) reports agentic mode against static whole-video processing on "
               "LongVideoBench as up to 88% fewer tokens, up to 66% lower cost and up to 7% higher accuracy, without absolute scores; "
               "the numbers here are absolute, on LVBench, and directly comparable across rows because every row answered the same "
               "questions under the same scoring. VideoIndex's per-question cost excludes indexing (done once per video); Gemini's "
               "per-question cost is the whole cost. Both systems are scored with the same letter parser.", ""]
    if all_ungraded:
        md += ["", "## Predicted letters", "",
               "No public answers exist for this set, so there is no accuracy here; scoring happens on Kaggle, where a "
               "kaggle-benchmarks task asks the hosted VideoIndex API the same questions. The letter distribution is a sanity check "
               "for position bias (five-way questions: about 20% each if the answers are spread evenly).", ""]
        for label, _, s, path in rows:
            run = json.loads(Path(path).read_text())
            cnt = collections.Counter(r.get("predicted") or "—" for r in run["results"] if r.get("status") == "ok")
            md.append(f"**{label}**: " + ", ".join(f"{k} {v}" for k, v in sorted(cnt.items())))
    md += ["", "## Questions by task type" if all_ungraded else "## Accuracy by task type", ""]
    types = sorted({t for _, _, s, _ in rows for t in s["per_type"]})
    md.append("| Task type | " + " | ".join(label for label, _, _, _ in rows) + " |")
    md.append("|---|" + "---|" * len(rows))
    for t in types:
        cells = []
        for _, _, s, _ in rows:
            pt = s["per_type"].get(t)
            if pt and all_ungraded:
                cells.append(f"n={pt['n']}")
            else:
                cells.append(f"{100 * pt['accuracy']:.0f}% (n={pt['n']})" if pt else "—")
        md.append(f"| {t} | " + " | ".join(cells) + " |")
    md += ["", "## Tool-call profiles", ""]
    for label, _, s, _ in rows:
        md.append(f"**{label}**: " + "; ".join(f"`{k}` ×{v}" for k, v in s["tool_histogram"]))
        md.append("")
    md += ["## Runs", ""]
    for label, cfg, s, path in rows:
        md.append(f"- **{label}**: `{path}` — {json.dumps({k: v for k, v in cfg.items() if k not in ('index', 'config')})}")
    md.append("")
    md.append(f"Questions per run: up to {n_q}. Benchmark videos are public YouTube content and may be in model training data; read per-configuration deltas rather than absolute scores.")
    if a.submission:
        import csv  # noqa: PLC0415

        run = json.loads(Path(a.runs[0]).read_text())
        with open(a.submission, "w", newline="") as f:
            w = csv.writer(f)
            w.writerow(["", "Final Answer"])
            for r in sorted(run["results"], key=lambda r: int(r.get("kaggle_row") or 0) if str(r.get("kaggle_row", "")).isdigit() else 0):
                if r.get("status") == "ok":
                    w.writerow([r.get("kaggle_row", ""), f"Final Answer: ({r.get('predicted') or 'A'})"])
        print(a.submission)
    out.write_text("\n".join(md) + "\n")
    print(out)


if __name__ == "__main__":
    main()
