#!/usr/bin/env python3
"""Run a configuration matrix on a benchmark sample and write the report.

    python3 -m eval.run eval/configs/lvbench-first.toml --fraction 0.25 [--seed 1] [--jobs 4] [--only agent,retrieval-only]

The TOML names the benchmark, root, index, config and a list of `[[runs]]`
(policy runs answered through `vi ask`, or `baseline = "uniform"` runs). Every
run answers the same stratified sample; run files land next to each other and
`eval.report` turns them into `docs/results/<benchmark>-<date>.md`.
"""
from __future__ import annotations

import argparse
import datetime as dt
import subprocess
import sys
import tomllib
from pathlib import Path


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("config")
    ap.add_argument("--fraction", type=float)
    ap.add_argument("--sample", type=int)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--only", help="comma-separated run names")
    ap.add_argument("--runs-dir", default="/data/videoindex/eval/runs")
    ap.add_argument("--vi", default="target/eval/release/vi")
    ap.add_argument("--out")
    ap.add_argument("--title")
    ap.add_argument("--redo-unparsed", action="store_true", help="re-ask stored answers that had no option letter (index-based runs)")
    ap.add_argument("--run-reference", action="store_true", help="also execute runs marked reference = true (external systems such as Gemini; otherwise their cached run file is only reported)")
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()
    cfg = tomllib.loads(Path(a.config).read_text())
    bench = cfg["benchmark"]
    tag = f"f{a.fraction:g}" if a.fraction else (f"n{a.sample}" if a.sample else "full")
    only = set(a.only.split(",")) if a.only else None
    sampling = (["--fraction", str(a.fraction)] if a.fraction else []) + (["--sample", str(a.sample)] if a.sample else []) + ["--seed", str(a.seed)]
    outs = []
    for run in cfg["runs"]:
        name = run["name"]
        out = Path(a.runs_dir) / f"{bench}-{tag}-s{a.seed}-{name}.json"
        # The report covers every run of the matrix whose file exists, whether or
        # not this invocation executed it (`--only`, cached references).
        if out.is_file() or not (only and name not in only) and not (run.get("reference") and not a.run_reference):
            outs.append(str(out))
        if only and name not in only:
            continue
        if run.get("reference") and not a.run_reference:
            if not out.is_file():
                print(f"(reference run {name} has no cached file at {out}; pass --run-reference to execute it)", file=sys.stderr)
            continue
        if run.get("baseline") == "gemini":
            cmd = [sys.executable, "-m", "eval.runners.gemini", bench, "--root", cfg["root"], "--model", run.get("model", "gemini-3.8-flash"),
                   "--mode", run.get("mode", "agentic"), "--jobs", str(min(a.jobs, 2)), "--resume", "--out", str(out)] + sampling
        elif run.get("baseline") == "uniform":
            cmd = [sys.executable, "-m", "eval.runners.baselines", bench, "--root", cfg["root"], "--frames", str(run.get("frames", 32)),
                   "--model", run.get("model", "claude-sonnet-5"), "--jobs", str(min(a.jobs, 3)), "--resume", "--out", str(out)] + sampling
        else:
            cmd = [sys.executable, "-m", "eval.runners.answer", bench, "--root", cfg["root"], "--index", cfg["index"], "--config", cfg["config"],
                   "--vi", a.vi, "--policy", run["policy"], "--max-tool-calls", str(run.get("max_tool_calls", 6)),
                   "--budget-usd", str(run.get("budget_usd", 0.5)), "--budget-tokens", str(run.get("budget_tokens", 120000)),
                   "--jobs", str(a.jobs), "--resume", "--out", str(out)] + (["--redo-unparsed"] if a.redo_unparsed else []) + sampling
        print("$", " ".join(cmd), file=sys.stderr, flush=True)
        if not a.dry_run:
            subprocess.run(cmd, check=False)
    report = a.out or f"docs/results/{bench}-{tag}-{dt.date.today().isoformat()}.md"
    title = a.title or f"{bench}: {'{:.0%}'.format(a.fraction) if a.fraction else (a.sample or 'all')} stratified sample, seed {a.seed}"
    outs = [o for o in dict.fromkeys(outs) if Path(o).is_file()]
    cmd = [sys.executable, "-m", "eval.report", bench, *outs, "--out", report, "--title", title]
    print("$", " ".join(cmd), file=sys.stderr, flush=True)
    if not a.dry_run:
        subprocess.run(cmd, check=False)


if __name__ == "__main__":
    main()
