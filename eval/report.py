#!/usr/bin/env python3
"""Tables and a pareto plot for a set of run files on one benchmark.

    python3 -m eval.report lvbench /data/videoindex/eval/runs/lvbench-*.json --out docs/results/lvbench-2026-09-12.md
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
from pathlib import Path

from .metrics import score

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("benchmark")
    ap.add_argument("runs", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--title")
    a = ap.parse_args()
    rows = []
    for path in a.runs:
        run = json.loads(Path(path).read_text())
        s = score(run)
        cfg = run["config"]
        label = cfg.get("policy", Path(path).stem)
        if cfg.get("model"):
            label += f" ({cfg['model']})"
        rows.append((label, cfg, s, path))
    out = Path(a.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    assets = out.parent / "assets"
    assets.mkdir(exist_ok=True)
    plot = assets / f"{out.stem}-pareto.svg"
    try:
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
        md.append(f"| {label} | {s['n']} | **{100 * s['accuracy']:.1f}%** | {100 * s['ci'][0]:.1f}–{100 * s['ci'][1]:.1f} | {s['unparsed']} | ${s['cost_mean']:.3f} | {s['tokens_mean']:,.0f} | {s['tool_calls_mean']:.2f} | {s['latency_p50']:.1f} s | {s['latency_p95']:.1f} s | {100 * s['no_decode_fraction']:.0f}% | {cir} |")
    md += ["", "## Accuracy by task type", ""]
    types = sorted({t for _, _, s, _ in rows for t in s["per_type"]})
    md.append("| Task type | " + " | ".join(label for label, _, _, _ in rows) + " |")
    md.append("|---|" + "---|" * len(rows))
    for t in types:
        cells = []
        for _, _, s, _ in rows:
            pt = s["per_type"].get(t)
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
    out.write_text("\n".join(md) + "\n")
    print(out)


if __name__ == "__main__":
    main()
