#!/usr/bin/env python3
"""Build the benchmark report (Markdown + charts + PDF) from the evaluation
artefacts under /data/videoindex/eval and the indexing logs.

    python3 scripts/report/benchmark_report.py [--eval-dir DIR] [--out PDF]

Inputs (all optional except the VideoIndex QA/retrieval runs):
  retrieval.json, retrieval2.json          scripts/devset_eval.py runs (before/after the FTS fix)
  qa-agent.json, qa-agent2.json            scripts/devset_qa_eval.py, agent policy
  qa-retrieval-only.json                   scripts/devset_qa_eval.py, retrieval-only baseline
  qa-gemini-agentic.json                   scripts/gemini_video_eval.py (Gemini agentic video)
  qa-gemini-static.json                    optional static-mode run
  /data/videoindex/logs/dataset-index*.log indexing logs for the per-video timing table
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import statistics
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from report import charts  # noqa: E402
from report.mdreport import build_pdf  # noqa: E402
import index_log_timings  # noqa: E402

# Generated reports live in the internal repository next to this one.
INTERNAL = Path(os.environ.get("VI_INTERNAL", ROOT.parent / "vi_internal"))
OUT_DIR = INTERNAL / "reports"
ASSETS = OUT_DIR / "assets"
GEN = OUT_DIR / "generated"

# Measured on the idle machine, 2026-09-12 (docs/MACHINE.md).
VISUAL_TIMINGS = {
    "policies": ["Decode + pHash + thumbnails", "Image embedding only", "OCR only", "Full visual pass"],
    "CPU build": [39.7, 52.9, 95.9, 112.3],
    "CUDA build, first": [39.5, 38.4, 175.9, 177.4],
    "CUDA build, fixed": [None, None, 48.2, 50.2],
}
PER_CALL = [  # onnx_bench, 640x360 frame, 20 iterations
    ("SigLIP image embed, batch 1", 270, 11.7),
    ("SigLIP image embed, batch 8", 1623, 46),
    ("SigLIP text embed", 83, 6.9),
    ("bge-small, 32 spans", 165, 8.3),
    ("RapidOCR detect + recognise", 262, 26),
]
GEMINI_CLAIMS = {  # blog.google, "Introducing agentic video in Gemini", 2026-09-01
    "tokens": -88, "cost": -66, "accuracy": +7,
}
GEMINI_PRICE_IN, GEMINI_PRICE_OUT, GEMINI_PRICE_CACHED = 0.75, 3.75, 0.075


def load(path: Path):
    return json.loads(path.read_text()) if path.is_file() else None


def gemini_cost(raw: dict) -> float:
    tin = (raw.get("total_input_tokens", 0) or 0) + (raw.get("total_tool_use_tokens", 0) or 0)
    tc = raw.get("total_cached_tokens", 0) or 0
    tout = (raw.get("total_output_tokens", 0) or 0) + (raw.get("total_thought_tokens", 0) or 0)
    return tin / 1e6 * GEMINI_PRICE_IN + tc / 1e6 * GEMINI_PRICE_CACHED + tout / 1e6 * GEMINI_PRICE_OUT


def model_label(model: str | None) -> str:
    """`gemini-3.8-flash` -> `Gemini 3.8 Flash`."""
    if not model:
        return "Gemini"
    parts = model.replace("gemini-", "").split("-")
    return "Gemini " + " ".join(p.capitalize() if p.isalpha() else p for p in parts)


def gemini_summary(run: dict | None) -> dict | None:
    """Re-derive the Gemini summary from raw usage so the cost model is the one
    documented here, whatever the harness assumed when it ran."""
    if not run:
        return None
    res = [r for r in run["results"] if r.get("status") == "ok"]
    if not res:
        return None
    n = len(res)
    costs = [gemini_cost(r["usage"]["raw"]) for r in res]
    tot = [r["usage"]["raw"].get("total_tokens", 0) for r in res]
    tool = [r["usage"]["raw"].get("total_tool_use_tokens", 0) or 0 for r in res]
    think = [r["usage"]["raw"].get("total_thought_tokens", 0) or 0 for r in res]
    calls = [r.get("steps", {}).get("processing_call", 0) for r in res]
    return {
        "model": run.get("model"), "mode": run.get("mode"), "n": n,
        "accuracy": sum(r["correct"] for r in res) / n,
        "cited": sum(r["cited"] for r in res) / n,
        "cite_time_ok": sum(r["cite_time_ok"] for r in res) / n,
        "cost_mean": sum(costs) / n, "cost_total": sum(costs),
        "tokens_mean": sum(tot) / n, "tool_tokens_mean": sum(tool) / n, "thought_tokens_mean": sum(think) / n,
        "processing_calls_mean": sum(calls) / n,
        "p50_s": statistics.median(r["ms"] for r in res) / 1000,
        "p90_s": sorted(r["ms"] for r in res)[int(0.9 * (n - 1))] / 1000,
        "misses": [r for r in res if not r["correct"]],
        "not_scored": [(r["id"], r.get("status")) for r in run["results"] if r.get("status") != "ok"],
    }


def vi_qa_summary(run: dict | None) -> dict | None:
    if not run:
        return None
    rep = run["report"]
    res = [r for r in run["results"] if r["status"] == "ok"]
    n = len(res) or 1
    return {
        **rep,
        "tokens_in_mean": sum(r["usage"].get("tokens_in", 0) for r in res) / n,
        "tokens_out_mean": sum(r["usage"].get("tokens_out", 0) for r in res) / n,
        "p90_s": sorted(r["ms"] for r in res)[int(0.9 * (len(res) - 1))] / 1000 if res else None,
        "misses": [r for r in res if not r["correct"]],
    }


def subset(summary: dict | None, run: dict | None, ids: set) -> dict | None:
    """Recompute a VideoIndex QA summary over a subset of question ids so a
    partial comparison run is scored against the same questions."""
    if not summary or not run:
        return None
    res = [r for r in run["results"] if r["status"] == "ok" and r["id"] in ids]
    if not res:
        return None
    n = len(res)
    return {
        **summary, "n": n,
        "accuracy": sum(r["correct"] for r in res) / n,
        "cited": sum(r["cited"] for r in res) / n,
        "cite_video_ok": sum(r["cite_video_ok"] for r in res) / n,
        "cite_time_ok": sum(r["cite_time_ok"] for r in res) / n,
        "cost_usd_mean": sum(r["usage"].get("cost_usd", 0) for r in res) / n,
        "cost_usd_total": sum(r["usage"].get("cost_usd", 0) for r in res),
        "tokens_mean": sum(r["usage"].get("tokens_in", 0) + r["usage"].get("tokens_out", 0) for r in res) / n,
        "tool_calls_mean": sum(r["usage"].get("tool_calls", 0) for r in res) / n,
        "p50_s": statistics.median(r["ms"] for r in res) / 1000,
        "p90_s": sorted(r["ms"] for r in res)[int(0.9 * (n - 1))] / 1000,
        "misses": [r for r in res if not r["correct"]],
    }


def pct(x, digits=1):
    return f"{100 * x:.{digits}f}%"


def hms(secs):
    secs = int(round(secs))
    return f"{secs // 3600}:{secs % 3600 // 60:02d}:{secs % 60:02d}"


def dataset_table(index_status: dict | None):
    if not index_status:
        return "", []
    rows = []
    for v in index_status["videos"]:
        vid = v["video"]
        d = vid["duration"]
        dur = d["num"] / d["den"] if isinstance(d, dict) else float(d)
        rows.append((vid.get("title") or vid["source_uri"], vid.get("channel") or "", dur,
                     v["transcript_spans"], v["ocr_spans"], v["frame_samples"], v["segments"]))
    rows.sort(key=lambda r: -r[2])
    lines = ["| Video | Channel | Duration | Transcript spans | OCR lines | Frames (1 fps) | Shots |", "|---|---|---|---|---|---|---|"]
    for t, ch, dur, ts, ocr, fr, seg in rows:
        lines.append(f"| {t[:70].replace('|', '/')} | {ch} | {hms(dur)} | {ts:,} | {ocr:,} | {fr:,} | {seg} |")
    tot_dur = sum(r[2] for r in rows)
    lines.append(f"| **{len(rows)} videos** | | **{hms(tot_dur)}** | **{sum(r[3] for r in rows):,}** | **{sum(r[4] for r in rows):,}** | **{sum(r[5] for r in rows):,}** | **{sum(r[6] for r in rows):,}** |")
    return "\n".join(lines), rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--eval-dir", default="/data/videoindex/eval")
    ap.add_argument("--logs", default="/data/videoindex/logs/dataset-index.log,/data/videoindex/logs/dataset-index2.log")
    ap.add_argument("--index", default="/data/videoindex/indexes/dataset.vidx")
    ap.add_argument("--config", default="config/gcp-a100.toml")
    ap.add_argument("--out", default=str(OUT_DIR / f"videoindex-benchmark-{dt.date.today().isoformat()}.pdf"))
    ap.add_argument("--html")
    a = ap.parse_args()
    ev = Path(a.eval_dir)
    ASSETS.mkdir(parents=True, exist_ok=True)
    GEN.mkdir(parents=True, exist_ok=True)

    r1, r2 = load(ev / "retrieval.json"), load(ev / "retrieval2.json")
    r_fine = load(ev / "retrieval-fine.json")
    qa_fine = vi_qa_summary(load(ev / "qa-agent-fine.json"))
    qa1, qa2, qro = vi_qa_summary(load(ev / "qa-agent.json")), vi_qa_summary(load(ev / "qa-agent2.json")), vi_qa_summary(load(ev / "qa-retrieval-only.json"))
    gem_run = load(ev / "qa-gemini-agentic.json")
    gem = gemini_summary(gem_run)
    gem_static = gemini_summary(load(ev / "qa-gemini-static.json"))
    gem_ids = {r["id"] for r in (gem_run or {}).get("results", []) if r.get("status") == "ok"}
    qa2_sub = subset(qa2, load(ev / "qa-agent2.json"), gem_ids) if gem else None
    qro_sub = subset(qro, load(ev / "qa-retrieval-only.json"), gem_ids) if gem else None
    status = None
    try:
        vi = ROOT / "target" / "release" / "vi"
        out = subprocess.run([str(vi), "--config", a.config, "status", a.index, "--json"], capture_output=True, text=True, check=True).stdout
        status = json.loads(out)
    except Exception as e:  # noqa: BLE001
        print(f"warning: no index status ({e})", file=sys.stderr)
    ds_table, ds_rows = dataset_table(status)

    # ---------------------------------------------------------------- charts
    R1, R2 = r1["report"], r2["report"]
    charts.grouped_bars(
        ASSETS / "retrieval-before-after.svg",
        ["video@1", "hit@1", "hit@5", "MRR"],
        [("FTS ANDs every word (64 scored)", [R1["overall"][k] for k in ("video@1", "hit@1", "hit@5", "mrr")]),
         ("OR-ed terms, stopwords removed (72 scored)", [R2["overall"][k] for k in ("video@1", "hit@1", "hit@5", "mrr")])],
        title="Retrieval on the 72-question dev set, before and after the query fix", ylim=(0, 1.08), fmt="{:.2f}",
        note="vidx search, k = 5, hit = result within 30 s of the anchored ground truth. Source: scripts/devset_eval.py")
    kinds = ["transcript", "ocr", "visual"]
    charts.grouped_bars(
        ASSETS / "retrieval-by-kind.svg",
        [f"{k} (n={R2[k]['n']})" for k in kinds],
        [("hit@1", [R2[k]["hit@1"] for k in kinds]), ("hit@5", [R2[k]["hit@5"] for k in kinds]), ("MRR", [R2[k]["mrr"] for k in kinds])],
        title="Retrieval by question type (after the fix)", ylim=(0, 1.08), fmt="{:.2f}", figsize=(7.2, 3.0))

    qa_labels, qa_acc, qa_cost, qa_tok, qa_lat = [], [], [], [], []
    for lab, s in (("VideoIndex agent (fixed retrieval)", qa2), ("VideoIndex agent (first run)", qa1), ("VideoIndex retrieval-only", qro)):
        if s:
            qa_labels.append(lab); qa_acc.append(s["accuracy"]); qa_cost.append(s["cost_usd_mean"]); qa_tok.append(s["tokens_mean"]); qa_lat.append(s["p50_s"])
    charts.hbars(ASSETS / "qa-accuracy.svg", qa_labels, [100 * x for x in qa_acc], title="Question answering accuracy, 52 questions",
                 xlabel="accuracy (%)", fmt="{:.1f}%", xlim=(0, 108), highlight=qa_labels[0], figsize=(7.2, 2.4),
                 note="Correct = answer contains an accepted string. Source: scripts/devset_qa_eval.py")
    charts.scatter_labeled(ASSETS / "qa-cost-accuracy.svg",
                           [(lab, c, 100 * acc, 0) for lab, c, acc in zip(qa_labels, qa_cost, qa_acc)],
                           title="Accuracy against cost per question (VideoIndex runs)", xlabel="USD per question (Claude Sonnet 5 list price)",
                           ylabel="accuracy (%)", ylim=(60, 102), figsize=(6.4, 3.4))

    # Indexing timings per video
    videos = []
    for log in a.logs.split(","):
        if Path(log).is_file():
            videos.extend(index_log_timings.parse(log))
    by_id = {}
    for v in videos:
        if "elapsed" in v:
            k = v.get("video_id", v["uri"])
            if k not in by_id or v["elapsed"] > by_id[k]["elapsed"]:
                by_id[k] = v
    tv = sorted(by_id.values(), key=lambda v: v["duration"] / v["elapsed"])
    names = index_log_timings.titles(a.index)
    labels = [(names.get(v.get("video_id"), "") or Path(v["uri"]).stem[:10])[:44].replace("|", "/") for v in tv]
    speeds = [v["duration"] / v["elapsed"] for v in tv]
    charts.dot_strip(ASSETS / "indexing-speed.svg", labels, speeds, title="Coarse indexing speed per video (CPU ONNX build, loaded machine)",
                     xlabel="× real time", median=statistics.median(speeds), figsize=(7.4, 7.6))
    ocr_rate = [v["stages"].get("ocr", (0, 0))[0] / (v["duration"] / 3600) for v in tv]
    charts.scatter_labeled(ASSETS / "indexing-ocr-vs-speed.svg",
                           [("", o, s_, 0) for o, s_ in zip(ocr_rate, speeds)],
                           title="Indexing speed against OCR density", xlabel="OCR lines per hour of video", ylabel="× real time", figsize=(6.4, 3.4),
                           note="One dot per video. Slide-heavy recordings (thousands of OCR lines per hour) are the slow ones on the CPU build.")

    pols = VISUAL_TIMINGS["policies"]
    charts.grouped_bars(ASSETS / "gpu-vs-cpu.svg", pols,
                        [("CPU build", VISUAL_TIMINGS["CPU build"]),
                         ("CUDA build, first", VISUAL_TIMINGS["CUDA build, first"]),
                         ("CUDA build, fixed", VISUAL_TIMINGS["CUDA build, fixed"])],
                        title="Visual pass on a 47-minute 720p lecture, idle machine (seconds, lower is better)", ylabel="seconds", fmt="{:.0f}",
                        figsize=(7.2, 3.4), note="Fixed = heuristic cuDNN algorithm search + bucketed OCR widths. Decode alone is 39.5 s; the fixed CUDA build sits 10 s above it.")
    charts.hbars(ASSETS / "per-call-speedup.svg", [p[0] for p in PER_CALL], [p[1] / p[2] for p in PER_CALL],
                 title="Per-call speed-up of the ONNX models on the A100 (CPU ms ÷ CUDA ms)", xlabel="×", fmt="{:.0f}×", figsize=(7.2, 2.6),
                 note="onnx_bench example, one fixed 640×360 frame, 20 iterations, machine under load. A fixed shape hides the cuDNN search cost the pipeline paid.")

    # Gemini comparison charts (separate)
    if gem:
        q2, qr = qa2_sub or qa2, qro_sub or qro
        sys_labels = ["VideoIndex agent", "VideoIndex retrieval-only", f"{model_label(gem['model'])} agentic video"]
        accs = [100 * q2["accuracy"], 100 * qr["accuracy"], 100 * gem["accuracy"]]
        costs = [q2["cost_usd_mean"], qr["cost_usd_mean"], gem["cost_mean"]]
        toks = [q2["tokens_mean"], qr["tokens_mean"], gem["tokens_mean"]]
        lats = [q2["p50_s"], qr["p50_s"], gem["p50_s"]]
        cite_t = [100 * q2["cite_time_ok"], 100 * qr["cite_time_ok"], 100 * gem["cite_time_ok"]]
        if gem_static:
            sys_labels.append(f"{model_label(gem_static['model'])} static (whole video)")
            accs.append(100 * gem_static["accuracy"]); costs.append(gem_static["cost_mean"]); toks.append(gem_static["tokens_mean"]); lats.append(gem_static["p50_s"]); cite_t.append(100 * gem_static["cite_time_ok"])
        charts.hbars(ASSETS / "gemini-accuracy.svg", sys_labels, accs, title=f"Same {gem['n']} questions, same video files: accuracy", xlabel="accuracy (%)",
                     fmt="{:.1f}%", xlim=(0, 108), figsize=(7.2, 2.6), colors=[charts.SERIES[0], charts.SERIES[0], charts.SERIES[1]] + ([charts.SERIES[1]] if gem_static else []))
        charts.hbars(ASSETS / "gemini-cost.svg", sys_labels, costs, title="Cost per question (list prices, USD)", xlabel="USD",
                     fmt="${:.3f}", figsize=(7.2, 2.6), colors=[charts.SERIES[0], charts.SERIES[0], charts.SERIES[1]] + ([charts.SERIES[1]] if gem_static else []),
                     note="VideoIndex: Claude Sonnet 5 tokens across all agent calls. Gemini: prompt + loaded media at $0.75/M, cached at $0.075/M, output + thinking at $3.75/M.")
        charts.hbars(ASSETS / "gemini-tokens.svg", sys_labels, [t / 1000 for t in toks], title="Tokens per question (thousands)", xlabel="k tokens",
                     fmt="{:.1f}k", figsize=(7.2, 2.6), colors=[charts.SERIES[0], charts.SERIES[0], charts.SERIES[1]] + ([charts.SERIES[1]] if gem_static else []))
        charts.hbars(ASSETS / "gemini-latency.svg", sys_labels, lats, title="Median latency per question (seconds)", xlabel="s",
                     fmt="{:.1f} s", figsize=(7.2, 2.6), colors=[charts.SERIES[0], charts.SERIES[0], charts.SERIES[1]] + ([charts.SERIES[1]] if gem_static else []))
        charts.hbars(ASSETS / "gemini-citations.svg", sys_labels, cite_t, title="Citations within 90 s of the ground-truth moment", xlabel="% of questions",
                     fmt="{:.1f}%", xlim=(0, 108), figsize=(7.2, 2.6), colors=[charts.SERIES[0], charts.SERIES[0], charts.SERIES[1]] + ([charts.SERIES[1]] if gem_static else []))
    # Published relative claims vs our measured agent-vs-baseline deltas
    rel_labels = ["Accuracy", "Cost per question", "Tokens per question"]
    ours = [100 * (qa2["accuracy"] / qro["accuracy"] - 1), 100 * (qa2["cost_usd_mean"] / qro["cost_usd_mean"] - 1), 100 * (qa2["tokens_mean"] / qro["tokens_mean"] - 1)]
    theirs = [GEMINI_CLAIMS["accuracy"], GEMINI_CLAIMS["cost"], GEMINI_CLAIMS["tokens"]]
    _diverging_bars(ASSETS / "gemini-relative-claims.svg", rel_labels, ours, theirs)

    # -------------------------------------------------------------- markdown
    md = GEN / "benchmark.md"
    md.write_text(render_markdown(R1, R2, r2, qa1, qa2, qro, gem, gem_static, ds_table, ds_rows, tv, names, status, qa2_sub, qro_sub, r_fine, qa_fine))
    spec = {
        "title": "VideoIndex benchmark report",
        "subtitle": "Retrieval and question answering over 36.6 hours of lectures and workshops, indexing throughput, and a comparison with Gemini agentic video",
        "org": "VideoIndex",
        "authors": ["Generated from the evaluation artefacts by scripts/report/benchmark_report.py"],
        "date": dt.date.today().isoformat(),
        "version": subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True, cwd=ROOT).stdout.strip(),
        "abstract": ("What was measured, how, and what the numbers mean: the 30-video development dataset, the two dev sets and their "
                     "scoring rules, retrieval quality before and after the full-text query fix, question answering with the agent and "
                     "a retrieval-only baseline, indexing throughput on CPU and GPU, and a like-for-like run of Gemini's agentic video "
                     "understanding on the same questions and files."),
        "out": a.out,
        "chapters": [{"file": str(md)}],
    }
    if a.html:
        spec["html_out"] = a.html
    print(build_pdf(spec))


def _diverging_bars(path, labels, ours, theirs):
    import matplotlib.pyplot as plt
    from report.charts import SERIES, SURFACE, INK2, GRID, MUTED, _rounded_bar, _style, save
    fig, ax = plt.subplots(figsize=(7.2, 2.8))
    ys = list(range(len(labels)))[::-1]
    h = 0.32
    lim = max(abs(v) for v in ours + theirs) * 1.25
    for y, o, t in zip(ys, ours, theirs):
        for val, off, color in ((o, +h / 2 + 0.02, SERIES[0]), (t, -h / 2 - 0.02, SERIES[1])):
            x0 = min(0, val)
            _rounded_bar(ax, x0, y + off - h / 2, abs(val), h, color, horizontal=True, radius=0.02 * lim)
            ax.text(val + (lim * 0.015 if val >= 0 else -lim * 0.015), y + off, f"{val:+.0f}%", va="center",
                    ha="left" if val >= 0 else "right", fontsize=8, color=INK2)
    ax.axvline(0, color=MUTED, linewidth=1)
    ax.set_yticks(ys); ax.set_yticklabels(labels)
    ax.set_xlim(-lim, lim); ax.set_ylim(-0.7, len(labels) - 0.3)
    ax.set_xlabel("change relative to each system's own baseline (%)")
    ax.set_title("Agentic mode relative to its baseline: our measurement and Google's published claim", pad=10)
    ax.bar([0], [0], color=SERIES[0], label="VideoIndex agent vs retrieval-only (measured, 52 questions)")
    ax.bar([0], [0], color=SERIES[1], label="Gemini agentic vs static (Google's 'up to' claims, LongVideoBench)")
    ax.legend(loc="upper center", bbox_to_anchor=(0.5, -0.3), ncol=1)
    _style(ax, horizontal=True)
    ax.spines["bottom"].set_visible(False)
    save(fig, Path(path))


def _miss_table(misses, id_key="id"):
    if not misses:
        return "_none_"
    lines = ["| Question | Accepted | Answer given (truncated) |", "|---|---|---|"]
    for r in misses:
        lines.append(f"| {r[id_key]}: {r['question'][:90].replace('|', '/')} | {', '.join(r['accept'])[:40]} | {r.get('answer', '')[:140].replace('|', '/').replace(chr(10), ' ')} |")
    return "\n".join(lines)


def render_markdown(R1, R2, r2, qa1, qa2, qro, gem, gem_static, ds_table, ds_rows, tv, names, status, qa2_sub=None, qro_sub=None, r_fine=None, qa_fine=None) -> str:
    tot_dur = sum(r[2] for r in ds_rows) if ds_rows else 0
    n_videos = len(ds_rows)
    wall = sum(v["elapsed"] for v in tv)
    dur = sum(v["duration"] for v in tv)
    misses_r = [r for r in r2["results"] if r["status"] == "ok" and (r.get("rank") is None or r["rank"] > 5)]
    unres = [r for r in r2["results"] if r["status"] != "ok"]
    idx_bytes = status["dir_bytes"] / 2**30 if status else 0

    gem_section = _gemini_section(qa2, qro, gem, gem_static, qa2_sub, qro_sub)

    return f"""# VideoIndex benchmark report

## Summary

<div class="kpi-row">
<div class="kpi"><div class="label">Retrieval hit@5</div><div class="value">{R2['overall']['hit@5']:.2f}</div><div class="sub">72 questions, k = 5, 30 s tolerance</div></div>
<div class="kpi"><div class="label">Retrieval MRR</div><div class="value">{R2['overall']['mrr']:.2f}</div><div class="sub">was {R1['overall']['mrr']:.2f} before the query fix</div></div>
<div class="kpi"><div class="label">QA accuracy, agent</div><div class="value">{pct(qa2['accuracy'])}</div><div class="sub">52 questions, {pct(qa2['cited'], 0)} cited, ${qa2['cost_usd_mean']:.3f} each</div></div>
<div class="kpi"><div class="label">QA accuracy, retrieval-only</div><div class="value">{pct(qro['accuracy'])}</div><div class="sub">one search then answer, ${qro['cost_usd_mean']:.3f} each</div></div>
</div>

This report documents the measurements made on the VideoIndex development dataset on 2026-09-11 and 2026-09-12. It is generated
from the evaluation artefacts (`/data/videoindex/eval/*.json`), the indexing logs and the index itself, so every number can be
regenerated by re-running the scripts named in each section. The dataset is {n_videos} lecture and workshop recordings totalling
{hms(tot_dur)} of video; the index built from it holds {idx_bytes:.2f} GiB.

Three results stand out.

1. **Retrieval quality was gated by one bug, not by the models.** The full-text query required every word of a question, so
   natural-language questions got no BM25 candidates at all and the fusion ran on the vector lists alone. OR-ing the terms took
   hit@5 from {R1['overall']['hit@5']:.2f} to {R2['overall']['hit@5']:.2f} and MRR from {R1['overall']['mrr']:.2f} to {R2['overall']['mrr']:.2f}.
2. **The agent earns its cost.** Against a retrieval-only baseline that answers from one search ({pct(qro['accuracy'])}),
   the tool-using agent reaches {pct(qa2['accuracy'])} at {qa2['cost_usd_mean'] / qro['cost_usd_mean']:.1f}× the cost per question
   (${qa2['cost_usd_mean']:.3f} against ${qro['cost_usd_mean']:.3f}) and about {qa2['tool_calls_mean']:.1f} tool calls per question.
3. **Indexing is OCR-bound, not decode-bound, and the GPU fixes that** once cuDNN's exhaustive per-shape algorithm search is
   turned off: the visual pass on a 47-minute lecture runs in 50 s on the CUDA build against 112 s on the CPU build, with decode
   alone at 39.5 s.

{gem_section['summary_line']}

## Dataset

The development dataset is the two YouTube playlists in `dataset/videolist.md`: the AI Engineer conference workshop playlist and
the UC Berkeley Agentic AI MOOC (CS294-196, Fall 2025). They were chosen because they are long, lecture-style and slide-heavy,
which stresses the parts of the system that matter for hour-scale video: transcript search over an hour of speech, on-screen text,
chapter segmentation and the agent's ability to find a moment rather than summarise a clip. The videos were downloaded at 720p with
subtitles, chapters and `.info.json` metadata, and indexed with the `coarse_only` policy (subtitle import, VAD, Whisper large-v3,
1 fps sampling, pHash, thumbnails, shot boundaries, SigLIP frame embeddings, RapidOCR on changed frames, bge-small text embeddings).

{ds_table}

Every video is a VP9 or AV1 stream at 1280×720. Transcript spans come from Whisper large-v3 (fp16, CTranslate2) through the
local OpenAI-compatible server; when a video shipped human-authored captions the `subtitle_import` operator imported them as a
subtitle track as well. OCR lines are RapidOCR (PP-OCRv4 detector, English PP-OCRv3 recogniser) read on frames whose pixels changed
by more than the gate; consecutive identical lines are collapsed. Frames are the 1 fps samples; shots are true camera cuts from
an HSV-histogram plus edge-change detector, which on slide lectures is a small number.

## Methodology

### Two dev sets

Two question sets were written by reading each video's caption digest and contact sheets (`scripts/devset_digest.py`), so
questions are grounded in what is actually said or shown. Both live under `dataset/` and are versioned with the code.

**Retrieval dev set** (`dataset/devset.jsonl`, 72 questions over all 30 videos: 65 transcript, 3 OCR, 4 visual). Each question is
a paraphrase of a moment, not the caption text, so lexical search alone cannot answer it. Transcript and OCR questions carry an
`anchor` phrase; the evaluation resolves the anchor to a time from the video's captions (searching a window of three consecutive
cues, since captions break sentences mid-phrase) or from the index's OCR spans, so ground truth follows the media rather than
hand-typed seconds. Visual questions carry a `t0`/`t1` range read off the contact sheets. Ground truth is the anchor time minus
5 s to plus 30 s.

**QA dev set** (`dataset/devset_qa.jsonl`, 52 questions over 29 videos). Each question names the speaker or session, asks for a
fact with a short answer (a number, a name, a term, a claim) and carries a list of accepted strings and an anchor phrase for the
moment that answers it. Accepted strings include common variants (`2,000`, `2000`, `two thousand`).

### Retrieval scoring

`scripts/devset_eval.py` runs `vidx search` for every question with k = 5 and hybrid retrieval (BM25 per span kind, bge-small text
vectors with the query instruction, SigLIP text-to-frame vectors, reciprocal-rank fusion with k = 60, grouping into shots or
60-second pieces). A result is a hit when its video is the question's video and its time range, widened by 30 s, overlaps the
ground-truth range. Reported metrics:

| Metric | Definition |
|---|---|
| video@1 | the top result is in the right video |
| hit@1, hit@5 | a hit at rank 1; a hit anywhere in the top 5 |
| MRR | mean of 1 / rank of the first hit (0 when none in the top 5) |
| p50 latency | median wall-clock of the one-shot `vidx search` process, including model load |

### Question-answering scoring

`scripts/devset_qa_eval.py` runs `vidx ask --json` for every question with a budget of $0.30 and 6 tool calls, `claude-sonnet-5`
as the agent model. The answer is **correct** when it contains any accepted string (case-insensitive substring). Citations are the
`[[cite:VIDEO:T0-T1]]` markers the agent emits; **cited** means at least one, **citation video correct** means one names the
question's video, and **citation in time** means one lies within 90 s of the anchor. Cost is Anthropic list price over every LLM
call in the `ask`; tokens are input plus output across those calls; latency is wall-clock per `ask`.

Two agent configurations and one baseline were run:

- **Agent, first run**: the LLM chooses tools (`search`, `get_transcript`, `get_ocr`, `timeline`, `view`, …) with the original
  AND-ed full-text query behind `search`.
- **Agent, fixed retrieval**: the same, after OR-ing full-text terms and adding a retry when the model returns an empty final
  turn after exhausting its tool budget.
- **Retrieval-only baseline**: the `RetrievalOnlyPolicy` issues exactly one `search` with the question (k = 8) and the LLM writes
  the answer from those hits. This is what the index alone gives before any agentic looking.

### Known limitations of the scoring

- Substring matching on accepted strings is generous for very short strings (a one-word accept can match inside a longer,
  wrong answer) and strict for paraphrases (a correct answer phrased differently is a miss). Every system compared below is
  scored identically, so the comparison is fair even where an individual verdict is debatable; the miss tables show the
  answers so a reader can judge.
- Anchors mark one moment that answers the question; a speaker who restates the point later produces a "right video, wrong
  minute" miss in retrieval and a citation-in-time miss in QA.
- The dev sets were written by the developers of the system while it was being built. They are a regression gate and a smoke
  test for hour-scale retrieval, not a public benchmark; LVBench, Minerva and 1H-VideoQA runs are planned for M4.
- `vidx search` latency includes process start and loading the SigLIP text tower and bge-small (about 2 s); the search itself is
  tens of milliseconds and the Python binding pays the load once.

### Hardware and software

All runs used one GCP `a2-ultragpu-1g` (12 vCPUs, 167 GiB RAM, one NVIDIA A100-SXM4-80GB, driver 595.91.07, CUDA 13.1 toolkit,
cuDNN 9), Ubuntu 26.04, ffmpeg 8.0.1, rustc 1.98.1, ONNX Runtime 1.28 through `ort` 2.0.0-rc.13, faster-whisper 1.2.1 on
CTranslate2 4.8.2. The dataset indexing run used the CPU ONNX build while builds and tests competed for the machine; the GPU
timings in the indexing section were taken afterwards on the idle machine.

## Results: retrieval

![](../assets/retrieval-before-after.svg)

| | n | video@1 | hit@1 | hit@5 | MRR |
|---|---|---|---|---|---|
| First run (FTS ANDs every word) | {R1['overall']['n']} scored, {72 - R1['overall']['n']} anchors unresolved | {R1['overall']['video@1']:.3f} | {R1['overall']['hit@1']:.3f} | {R1['overall']['hit@5']:.3f} | {R1['overall']['mrr']:.3f} |
| **After OR-ing terms with stopword removal** | {R2['overall']['n']} | **{R2['overall']['video@1']:.3f}** | **{R2['overall']['hit@1']:.3f}** | **{R2['overall']['hit@5']:.3f}** | **{R2['overall']['mrr']:.3f}** |

The first run's failure was structural. FTS5 treats a bare list of terms as a conjunction; the question "language models failing
to count the Rs in strawberry" became `"language" "models" "failing" "to" "count" "the" "Rs" "in" "strawberry"*` and matched nothing,
so every miss shows `lists=['text_vec', 'image_vec']`: the two BM25 lists were empty and only the two vector lists voted. After the
fix the terms are OR-ed (stopwords dropped unless the query is all stopwords, the last term prefix-matched) and BM25 ranks rows that
match more terms higher; every hit in the second run carries all four lists.

![](../assets/retrieval-by-kind.svg)

| Question type | n | video@1 | hit@1 | hit@5 | MRR |
|---|---|---|---|---|---|
| transcript | {R2['transcript']['n']} | {R2['transcript']['video@1']:.3f} | {R2['transcript']['hit@1']:.3f} | {R2['transcript']['hit@5']:.3f} | {R2['transcript']['mrr']:.3f} |
| OCR (on-screen text) | {R2['ocr']['n']} | {R2['ocr']['video@1']:.3f} | {R2['ocr']['hit@1']:.3f} | {R2['ocr']['hit@5']:.3f} | {R2['ocr']['mrr']:.3f} |
| visual (SigLIP text-to-frame) | {R2['visual']['n']} | {R2['visual']['video@1']:.3f} | {R2['visual']['hit@1']:.3f} | {R2['visual']['hit@5']:.3f} | {R2['visual']['mrr']:.3f} |

The OCR and visual groups are small (3 and 4 questions) and their numbers are indicative only. Visual retrieval uses the
SigLIP base patch-16 224 px model on 1 fps frames deduplicated by pHash; a larger SigLIP (so400m, 384 px) is the obvious upgrade
when the visual questions grow in number.

### Remaining misses

{len(misses_r)} of {R2['overall']['n']} questions have no hit in the top 5:

| Question | Type | What happened |
|---|---|---|
""" + "\n".join(
        f"| {r['id']}: {r['question'][:80].replace('|', '/')} | {r['type']} | video@{r.get('video_rank')} ; top result {r['top'][0][1]}–{r['top'][0][2]} s in {r['top'][0][0]} |"
        for r in misses_r) + f"""

Two are right-video-wrong-minute (the phrasing matches a later restatement), one visual question's frame ranks fourth, and one is a
speech-to-text paraphrase gap ("collision rate" is spoken as "collisions") that neither BM25 nor bge-small bridges.
{('Unresolved anchors: ' + ', '.join(r['id'] for r in unres)) if unres else 'Every anchor resolved.'}

## Results: question answering

![](../assets/qa-accuracy.svg)

| Run | accuracy | cited | citation video correct | citation within 90 s | cost / question | tokens / question | tool calls / question | median latency | p90 latency |
|---|---|---|---|---|---|---|---|---|---|
| Agent, first run (AND-ed FTS) | {pct(qa1['accuracy'])} | {pct(qa1['cited'])} | {pct(qa1['cite_video_ok'])} | {pct(qa1['cite_time_ok'])} | ${qa1['cost_usd_mean']:.3f} (${qa1['cost_usd_total']:.2f} total) | {qa1['tokens_mean']:,.0f} | {qa1['tool_calls_mean']:.2f} | {qa1['p50_s']:.1f} s | {qa1['p90_s']:.1f} s |
| **Agent, fixed retrieval** | **{pct(qa2['accuracy'])}** | {pct(qa2['cited'])} | {pct(qa2['cite_video_ok'])} | {pct(qa2['cite_time_ok'])} | ${qa2['cost_usd_mean']:.3f} (${qa2['cost_usd_total']:.2f} total) | {qa2['tokens_mean']:,.0f} | {qa2['tool_calls_mean']:.2f} | {qa2['p50_s']:.1f} s | {qa2['p90_s']:.1f} s |
| Retrieval-only baseline | {pct(qro['accuracy'])} | {pct(qro['cited'])} | {pct(qro['cite_video_ok'])} | {pct(qro['cite_time_ok'])} | ${qro['cost_usd_mean']:.3f} (${qro['cost_usd_total']:.2f} total) | {qro['tokens_mean']:,.0f} | {qro['tool_calls_mean']:.2f} | {qro['p50_s']:.1f} s | {qro['p90_s']:.1f} s |

![](../assets/qa-cost-accuracy.svg)

The agent's extra {qa2['tool_calls_mean'] - qro['tool_calls_mean']:.2f} tool calls per question buy {100 * (qa2['accuracy'] - qro['accuracy']):.1f} accuracy points. The
baseline's misses are mostly questions whose answer sits in a span the first search does not surface; the agent's second search or
a `get_transcript` around the first hit finds it. Three of the first run's seven misses were empty answers: the agent spent its six
tool calls searching (starved by the AND-ed query) and the final no-tools turn came back with no text. The loop now asks once more,
explicitly, before returning a partial answer; with the retrieval fix the agent averages {qa2['tool_calls_mean']:.2f} calls and never hits the cap.

### Agent misses (fixed retrieval)

{_miss_table(qa2['misses'])}

### Retrieval-only misses

{_miss_table(qro['misses'])}

{_fine_section(R2, qa2, r_fine, qa_fine)}

{gem_section['body']}

## Results: indexing throughput

### The dataset run

The 30 videos ({hms(dur)}) were indexed in one `vidx index` process with the `coarse_only` policy in {wall:,.0f} s ({wall / 3600:.1f} h), {dur / wall:.0f}× real
time overall, with zero failures. The machine was not idle: release builds, test suites and a second index were running at times, so
the per-video numbers are an upper bound on the cost of the CPU build.

![](../assets/indexing-speed.svg)

![](../assets/indexing-ocr-vs-speed.svg)

The spread is explained by OCR density. Camera-heavy talks index at 20× real time or better; slide decks that change every few
seconds produce thousands of OCR lines per hour, and on the CPU build RapidOCR's detector and recogniser dominate. Whisper ran on
the GPU throughout at about 29× real time for the speech it was given.

<details><summary>Per-video timings (CPU ONNX build, loaded machine)</summary>

""" + index_log_timings_table(tv, names) + f"""

</details>

### CPU against GPU

![](../assets/gpu-vs-cpu.svg)

| Policy (47-minute 720p VP9 lecture, idle machine) | CPU build | CUDA build, first | CUDA build, fixed |
|---|---|---|---|
""" + "\n".join(
        f"| {p} | {c:.1f} s | {f:.1f} s | {('%.1f s' % x) if x else '—'} |"
        for p, c, f, x in zip(VISUAL_TIMINGS['policies'], VISUAL_TIMINGS['CPU build'], VISUAL_TIMINGS['CUDA build, first'], VISUAL_TIMINGS['CUDA build, fixed'])) + f"""

Decode alone (sampling, pHash, thumbnails) takes 39.5 s for this 47-minute video, 71× real time, on either build. The first CUDA
build lost time in OCR: cuDNN's default exhaustive convolution-algorithm search re-benchmarks every convolution for each new input
shape, and the recogniser's padded batch width changed on almost every call. With the heuristic search and recogniser widths rounded
up to multiples of 32, OCR-only drops from 175.9 s to 48.2 s and the full visual pass to 50.2 s, 10 s above the decode floor. The
per-call benchmark below, taken on a single fixed frame, could not see this: it shows the GPU 10 to 35× faster per call, which is
true once shapes repeat.

![](../assets/per-call-speedup.svg)

| Call | CPU (6 threads) | CUDA | Speed-up |
|---|---|---|---|
""" + "\n".join(f"| {n} | {c:.0f} ms | {g:.1f} ms | {c / g:.0f}× |" for n, c, g in PER_CALL) + """

## Reproducing

```bash
# retrieval
python3 scripts/devset_eval.py /data/videoindex/indexes/dataset.vidx --config config/gcp-a100.toml --json /data/videoindex/eval/retrieval2.json
# question answering, agent and baseline
python3 scripts/devset_qa_eval.py /data/videoindex/indexes/dataset.vidx --config config/gcp-a100.toml --json /data/videoindex/eval/qa-agent2.json
python3 scripts/devset_qa_eval.py /data/videoindex/indexes/dataset.vidx --config config/gcp-a100.toml --policy retrieval-only --json /data/videoindex/eval/qa-retrieval-only.json
# Gemini agentic video on the same questions and files (needs GEMINI_API_KEY)
python3 scripts/gemini_video_eval.py --model gemini-3.8-flash --mode agentic --json /data/videoindex/eval/qa-gemini-agentic.json
# this report
python3 scripts/report/benchmark_report.py
```
"""


def index_log_timings_table(tv, names) -> str:
    cols = ["Video", "Duration", "Wall", "× real time", "asr", "ocr", "image_embed", "shot_boundary", "text_embed"]
    lines = ["| " + " | ".join(cols) + " |", "|" + "---|" * len(cols)]
    for v in sorted(tv, key=lambda v: -v["duration"]):
        name = (names.get(v.get("video_id"), "") or Path(v["uri"]).stem[:12])[:60].replace("|", "/")
        cells = [name, hms(v["duration"]), f"{v['elapsed']:.0f} s", f"{v['duration'] / v['elapsed']:.0f}×"]
        for st in ("asr", "ocr", "image_embed", "shot_boundary", "text_embed"):
            if st in v.get("written", {}):
                cells.append(str(v["written"][st]))
            elif st in v["stages"]:
                cells.append(str(v["stages"][st][0]))
            else:
                cells.append("cached" if st in v["cached"] else "")
        lines.append("| " + " | ".join(cells) + " |")
    return "\n".join(lines)


def _fine_section(R2, qa2, r_fine, qa_fine) -> str:
    if not r_fine or not qa_fine:
        return ""
    F = r_fine["report"]["overall"]
    R2 = R2["overall"] if "overall" in R2 else R2
    return f"""## Results: fine index against coarse index

The fine pass (VLM scene descriptions, chapters, entities and events; 5 h 8 min and $45.96 over the 30 videos with Claude Sonnet 5) was run on the same index and both dev sets re-evaluated.

| | Retrieval hit@5 | Retrieval MRR | video@1 | QA accuracy | citation within anchor | cost / question | tool calls / question |
|---|---|---|---|---|---|---|---|
| Coarse index | {R2['hit@5']:.3f} | {R2['mrr']:.3f} | {R2['video@1']:.3f} | {pct(qa2['accuracy'])} | {pct(qa2['cite_time_ok'])} | ${qa2['cost_usd_mean']:.3f} | {qa2['tool_calls_mean']:.2f} |
| Fine index | {F['hit@5']:.3f} | {F['mrr']:.3f} | {F['video@1']:.3f} | {pct(qa_fine['accuracy'])} | {pct(qa_fine['cite_time_ok'])} | ${qa_fine['cost_usd_mean']:.3f} | {qa_fine['tool_calls_mean']:.2f} |

On this transcript-anchored dev set the fine index scores lower: description rows join the fusion as an equal-weight BM25 list and as text vectors, and their plausible neighbours outrank transcript hits under reciprocal-rank fusion (16 retrieval questions moved down, 10 up; two QA questions flipped). Citation-in-anchor improved, so the rows are useful but over-weighted for this question mix. Per-kind list weights are the first tuning target of the M4 sweep; `coarse_only` stays the default for lecture content.
"""


def _gemini_section(qa2, qro, gem, gem_static, qa2_sub=None, qro_sub=None) -> dict:
    rel = (f"Our own agent-versus-baseline deltas are +{100 * (qa2['accuracy'] / qro['accuracy'] - 1):.0f}% accuracy, "
           f"+{100 * (qa2['cost_usd_mean'] / qro['cost_usd_mean'] - 1):.0f}% cost and +{100 * (qa2['tokens_mean'] / qro['tokens_mean'] - 1):.0f}% tokens.")
    claims = """### What Google published

Google announced agentic video understanding for Gemini 3.7 Flash, 3.6 Flash and 3.5 Flash-Lite on 1 September 2026 (the
developer documentation now lists 3.8 Flash as well). With `processing: "agentic"` on a video input, the model "dynamically
navigates the video timeline, loading only the content it needs based on the prompt" through internal `processing_call` steps
that fetch frames, audio or transcript for a chosen span, instead of the static mode's 1 fps frames at 258 tokens each plus
32 tokens per second of audio (about 300 tokens per second, so a one-hour video is roughly 1.1 million tokens). The claims,
quoted from the announcement: agentic mode can "reduce analysis costs by up to 66% and token consumption by up to 88%, while
improving accuracy by up to 7%" relative to static processing, with Gemini 3.7 Flash "at the accuracy-to-cost pareto frontier
among tested models"; the benchmark named is LongVideoBench, without absolute scores in the post. The feature uses standard
token pricing (Gemini 3.x Flash: $0.75 per million input tokens and $3.75 per million output tokens through 2026-12-31).

The two systems take different routes to the same goal. Gemini's agent works on the raw video at question time and pays for
whatever it loads on every question; VideoIndex indexes once (transcript, on-screen text, embeddings, shots) and answers from the
index, decoding pixels only when the agent calls `view`. The published deltas are relative to Gemini's own static mode, so the
chart below puts them beside our measured agent-versus-retrieval-only deltas; the direction differs because the baselines differ
(a full-video static pass is the expensive end of Gemini's range, while our retrieval-only baseline is the cheap end of ours).

![](../assets/gemini-relative-claims.svg)

""" + rel + "\n"
    if not gem:
        body = """## Comparison with Gemini agentic video

""" + claims + """
### Like-for-like run

The programmatic comparison (`scripts/gemini_video_eval.py`: the same 52 questions, the same video files uploaded through the
Files API, `gemini-3.8-flash` with `processing: "agentic"`, scored with the same rules) had not completed when this report was
generated. Re-run `scripts/report/benchmark_report.py` once `/data/videoindex/eval/qa-gemini-agentic.json` exists.
"""
        return {"summary_line": "", "body": body}

    g = gem
    n = g["n"]
    q2, qr = qa2_sub or qa2, qro_sub or qro
    partial = ("" if n >= 52 else
               f" The run was stopped after {n} questions by decision: this is a development set, and the extensive comparison is "
               f"planned on LVBench and the other public benchmarks in M4. The VideoIndex rows in this section are recomputed over the "
               f"same {n} questions so the comparison is like for like; the full-set VideoIndex numbers are in the previous section.")
    lines = f"""## Comparison with Gemini agentic video

{claims}
### Like-for-like run: same questions, same files

`scripts/gemini_video_eval.py` asked **{model_label(g['model'])}** (`{g['model']}`) with `processing: "agentic"` the QA dev set's questions over the same
video files (each file uploaded once through the Files API; one `interactions.create` call per question; the prompt asks for a
one- or two-sentence answer with `[HH:MM:SS]` citations). Answers were scored with the QA rules above: substring match against
the accepted strings, and a citation counts as in time when a timestamp in the answer lies within 90 s of the anchor. Gemini
cost is computed from the returned usage object at list price: prompt text and the media the agent loaded
(`total_tool_use_tokens`) at $0.75 per million, cached tokens at $0.075 per million, output and thinking tokens at $3.75 per
million.{partial}{('; not scored: ' + ', '.join(f'{i} ({s_})' for i, s_ in g['not_scored'])) if g['not_scored'] else ''}

| System ({n} questions) | accuracy | cited | citation within 90 s | cost / question | tokens / question | median latency | p90 latency |
|---|---|---|---|---|---|---|---|
| VideoIndex agent (Claude Sonnet 5 over the index) | {pct(q2['accuracy'])} | {pct(q2['cited'])} | {pct(q2['cite_time_ok'])} | ${q2['cost_usd_mean']:.3f} | {q2['tokens_mean']:,.0f} | {q2['p50_s']:.1f} s | {q2['p90_s']:.1f} s |
| VideoIndex retrieval-only | {pct(qr['accuracy'])} | {pct(qr['cited'])} | {pct(qr['cite_time_ok'])} | ${qr['cost_usd_mean']:.3f} | {qr['tokens_mean']:,.0f} | {qr['p50_s']:.1f} s | {qr['p90_s']:.1f} s |
| {model_label(g['model'])} agentic video | {pct(g['accuracy'])} | {pct(g['cited'])} | {pct(g['cite_time_ok'])} | ${g['cost_mean']:.3f} | {g['tokens_mean']:,.0f} | {g['p50_s']:.1f} s | {g['p90_s']:.1f} s |
"""
    if gem_static:
        s = gem_static
        lines += f"| {model_label(s['model'])} static (whole video) | {pct(s['accuracy'])} | {pct(s['cited'])} | {pct(s['cite_time_ok'])} | ${s['cost_mean']:.3f} | {s['tokens_mean']:,.0f} | {s['p50_s']:.1f} s | {s['p90_s']:.1f} s |\n"
    lines += f"""
Gemini's agent made {g['processing_calls_mean']:.1f} `processing_call` steps per question on average, loading {g['tool_tokens_mean']:,.0f} tokens of
frames and transcript, and spent {g['thought_tokens_mean']:,.0f} thinking tokens per question; thinking is the largest cost component at
output prices. VideoIndex's token count is Claude's context across all agent calls (index text, never pixels unless `view` is
called).

![](../assets/gemini-accuracy.svg)

![](../assets/gemini-cost.svg)

![](../assets/gemini-tokens.svg)

![](../assets/gemini-latency.svg)

![](../assets/gemini-citations.svg)

### Reading the comparison

- **Different models, different work.** Gemini answers from the raw video with its own model; VideoIndex answers from a prebuilt
  index with Claude Sonnet 5. The comparison is of two products on one task, not of two LLMs.
- **Indexing cost is not in the per-question figures.** VideoIndex spends compute once per video (about {wall_note()}); those
  costs amortise over every question asked of the video. Gemini's per-question cost is the whole cost.
- **Citations are typed differently.** VideoIndex citations are machine-checkable spans tied to index rows; Gemini's are
  timestamps the model writes in prose, parsed from the text. Both are scored by the same 90 s rule.
- **The scoring is the same, generous substring rule for both**, so short accepted strings (a single word) can match inside a
  longer wrong answer for either system.

### Gemini misses

{_miss_table(g['misses'])}
"""
    summary = (f"4. **{model_label(g['model'])} agentic video, asked {n} of the same questions over the same files, scored {pct(g['accuracy'])} "
               f"at ${g['cost_mean']:.3f} and {g['p50_s']:.1f} s median per question** (VideoIndex agent on those {n}: {pct(q2['accuracy'])}, "
               f"${q2['cost_usd_mean']:.3f}, {q2['p50_s']:.1f} s). Both systems answer these developer-written questions well; the "
               f"discriminating comparison waits for the public long-video benchmarks.")
    return {"summary_line": summary, "body": lines}


def wall_note():
    return "2 to 12 minutes of machine time per hour of video on this host depending on the build and the slide density"


if __name__ == "__main__":
    main()
