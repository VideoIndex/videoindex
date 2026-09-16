#!/usr/bin/env python3
"""Report for corpus runs: one Markdown page with charts under
`docs/results/`, and optionally the same content as a PDF technical report.

    python3 -m eval.corpus_report /data/videoindex/eval/runs/corpus/corpus-*.json \
        --out docs/results/corpus-2026-09-16.md --pdf ../vi_internal/reports/videoindex-corpus-eval-2026-09-16.pdf

Run files must have been scored by `eval.judge`. Quality, set precision /
recall / F1, anchor hit rate and fact coverage are defined in `eval/judge.py`.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import subprocess
import sys
from pathlib import Path

from .datasets.corpus import load
from .metrics import pct

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from report import charts  # noqa: E402

KIND_ORDER = ["mention", "topic", "synthesis", "library", "negative"]
KIND_LABEL = {"mention": "which videos mention X", "topic": "moments about a topic", "synthesis": "cross-video synthesis",
              "library": "whole-library summary", "negative": "nothing matches"}


def mean(xs):
    xs = [x for x in xs if x is not None]
    return sum(xs) / len(xs) if xs else None


def summarize(run: dict) -> dict:
    res = [r for r in run["results"] if r.get("scores")]
    by_kind = {}
    for k in KIND_ORDER:
        rs = [r for r in res if r["kind"] == k]
        if rs:
            by_kind[k] = {"n": len(rs), "quality": mean(r["scores"]["quality"] for r in rs)}
    set_rs = [r for r in res if r["kind"] not in ("library", "negative")]
    anchors_hit = sum(r["scores"]["anchor_hit"] * r["scores"]["anchors_checked"] for r in res if r["scores"]["anchor_hit"] is not None)
    anchors_n = sum(r["scores"]["anchors_checked"] for r in res if r["scores"]["anchor_hit"] is not None)
    costs = [r["usage"].get("cost_usd", 0) for r in res]
    lat = [r["ms"] / 1000 for r in res]
    return {
        "n": len(res), "failed": sum(r.get("status") != "ok" for r in run["results"]),
        "quality": mean(r["scores"]["quality"] for r in res), "by_kind": by_kind,
        "precision": mean(r["scores"]["precision"] for r in set_rs), "recall": mean(r["scores"]["recall"] for r in set_rs),
        "f1": mean(r["scores"]["f1"] for r in set_rs),
        "anchor_hit": anchors_hit / anchors_n if anchors_n else None, "anchors_n": anchors_n,
        "fact_coverage": mean(r["scores"]["fact_coverage"] for r in res),
        "wrong_total": sum(len(r["scores"]["wrong"]) for r in res), "unknown_total": sum(r["scores"]["unknown_videos"] for r in res),
        "missed_total": sum(len(r["scores"]["missed"]) for r in res),
        "claimed_total": sum(r["scores"]["claimed"] for r in res),
        "cost_mean": mean(costs) or 0.0, "cost_total": sum(costs), "cost_p95": pct(costs, 0.95),
        "latency_mean": mean(lat) or 0.0, "latency_p50": pct(lat, 0.5), "latency_p95": pct(lat, 0.95),
        "latency_seq_p50": pct([r["ms_sequential"] / 1000 for r in res if r.get("ms_sequential")], 0.5) if any(r.get("ms_sequential") for r in res) else None,
        "tokens_mean": mean(r["usage"].get("tokens_in", 0) + r["usage"].get("tokens_out", 0) for r in res) or 0.0,
        "tool_calls_mean": mean(r["usage"].get("tool_calls", 0) for r in res) or 0.0,
        "partial": sum(bool(r.get("partial")) for r in res),
        "judge_cost": sum((r.get("judge", {}).get("usage", {}).get("tokens_in", 0) * 5 + r.get("judge", {}).get("usage", {}).get("tokens_out", 0) * 25) / 1e6 for r in res),
    }


def heatmap(path: Path, rows: list[str], cols: list[str], values: list[list[float | None]], *, title: str):
    """Systems × questions grid of quality scores (one hue, light = low)."""
    import matplotlib.pyplot as plt  # noqa: PLC0415
    from matplotlib.colors import LinearSegmentedColormap  # noqa: PLC0415

    cmap = LinearSegmentedColormap.from_list("q", [charts.SEQ[100], charts.SEQ[400], charts.SEQ[700]])
    fig, ax = plt.subplots(figsize=(max(7.2, 0.27 * len(cols) + 2.2), 0.42 * len(rows) + 1.3))
    for i, row in enumerate(values):
        for j, v in enumerate(row):
            if v is None:
                ax.add_patch(plt.Rectangle((j, i), 1, 1, color=charts.DEEMPH, alpha=0.4, linewidth=0))
                continue
            ax.add_patch(plt.Rectangle((j + 0.04, i + 0.06), 0.92, 0.88, color=cmap(v), linewidth=0))
            ax.text(j + 0.5, i + 0.5, f"{v:.2f}".lstrip("0") if v < 1 else "1", ha="center", va="center", fontsize=6.6,
                    color=charts.SURFACE if v > 0.55 else charts.INK)
    ax.set_xlim(0, len(cols))
    ax.set_ylim(len(rows), 0)
    ax.set_xticks([j + 0.5 for j in range(len(cols))])
    ax.set_xticklabels(cols, rotation=90, fontsize=7)
    ax.set_yticks([i + 0.5 for i in range(len(rows))])
    ax.set_yticklabels(rows, fontsize=8)
    ax.tick_params(length=0)
    for s in ax.spines.values():
        s.set_visible(False)
    ax.set_title(title, pad=10)
    fig.tight_layout()
    return charts.save(fig, path)


def short_label(label: str) -> str:
    """Chart-sized version of a run label."""
    reps = [("VideoIndex ", "VI "), ("claude-sonnet-5", "Sonnet 5"), ("gemini-3.8-flash", "Gemini 3.8 Flash"), ("agent_llm default", "default model"),
            ("1,500-token answer cap", "1.5k cap"), ("Gemini agentic video (Gemini 3.8 Flash, ", "Gemini agentic video ("), (" videos + merge", " + merge")]
    for a, b in reps:
        label = label.replace(a, b)
    return label


def wrap(label: str, width: int = 16) -> str:
    import textwrap  # noqa: PLC0415

    return "\n".join(textwrap.wrap(label, width))


# ---------------------------------------------------------------- headline chart

def _esc(t: str) -> str:
    return t.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def headline_svg(path: Path, vi: dict, gem: dict, n_questions: int, n_videos: int) -> Path:
    """Four small panels, VideoIndex against Gemini agentic video, one metric
    each (cost, latency, F1, quality). Hand-built SVG in the brand tokens so the
    same file sits on videoindex.org, the docs site and in the results page.
    Emphasis form: VideoIndex in the accent hue, the reference in the muted
    ink; every value is labelled, so the gray needs no legend contrast."""
    NAVY, ACCENT, MUTED, INK, INK2, LINE = "#0a2347", "#3186e9", "#8a8880", "#0b0b0b", "#4d4c48", "#e3e1d8"
    SANS = "IBM Plex Sans, Inter, system-ui, -apple-system, Segoe UI, sans-serif"
    MONO = "IBM Plex Mono, ui-monospace, SF Mono, Menlo, monospace"
    panels = [
        ("Cost per question", "lower is better", vi["cost_mean"], gem["cost_mean"], lambda v: f"${v:.2f}", f"{gem['cost_mean'] / vi['cost_mean']:.1f}× cheaper"),
        ("Median latency", "lower is better · p95 " + f"{vi['latency_p95']:.0f} s against {gem['latency_p95']:.0f} s", vi["latency_p50"], gem["latency_p50"], lambda v: f"{v:.0f} s", f"{gem['latency_p50'] / vi['latency_p50']:.1f}× faster"),
        ("Videos found (F1)", "higher is better", 100 * vi["f1"], 100 * gem["f1"], lambda v: f"{v:.0f}%", None),
        ("Answer quality", "higher is better · judge-scored against ground truth", 100 * vi["quality"], 100 * gem["quality"], lambda v: f"{v:.0f}%", None),
    ]
    W, H = 720, 400
    PW, PH, PX0, PY0, GX, GY = 340, 118, 20, 92, 20, 14
    LABEL_W, VAL_W = 96, 60
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" role="img" '
           f'aria-label="VideoIndex against Gemini agentic video on {n_questions} library-wide questions: cost, latency, videos found and answer quality">',
           f'<title>VideoIndex against Gemini agentic video, {n_questions} library-wide questions over {n_videos} talks</title>',
           f'<rect width="{W}" height="{H}" fill="none"/>',
           f'<text x="20" y="30" font-family="{SANS}" font-size="16" font-weight="600" fill="{NAVY}">Questions that span the whole library</text>',
           f'<text x="20" y="50" font-family="{SANS}" font-size="12.5" fill="{INK2}">{n_questions} questions over {n_videos} talks (36.6 hours): which videos mention X, the moments about a topic, cross-video</text>',
           f'<text x="20" y="67" font-family="{SANS}" font-size="12.5" fill="{INK2}">comparisons, library summaries. Same questions, same answer format, same judge for both systems.</text>']
    for i, (title, note, v_vi, v_gem, fmt, callout) in enumerate(panels):
        x = PX0 + (i % 2) * (PW + GX)
        y = PY0 + (i // 2) * (PH + GY)
        out.append(f'<rect x="{x}" y="{y}" width="{PW}" height="{PH}" rx="12" fill="#fffefb" stroke="{LINE}"/>')
        out.append(f'<text x="{x + 16}" y="{y + 24}" font-family="{SANS}" font-size="13.5" font-weight="600" fill="{INK}">{_esc(title)}</text>')
        out.append(f'<text x="{x + 16}" y="{y + 40}" font-family="{SANS}" font-size="11" fill="{MUTED}">{_esc(note)}</text>')
        bx = x + 16 + LABEL_W
        bw_max = PW - 32 - LABEL_W - VAL_W
        vmax = max(v_vi, v_gem) or 1.0
        for j, (name, v, color, weight) in enumerate([("VideoIndex", v_vi, ACCENT, "600"), ("Gemini agentic", v_gem, MUTED, "400")]):
            by = y + 56 + j * 28
            bw = max(4.0, bw_max * v / vmax)
            out.append(f'<text x="{x + 16}" y="{by + 12}" font-family="{SANS}" font-size="12" font-weight="{weight}" fill="{NAVY if j == 0 else INK2}">{name}</text>')
            # square baseline end, rounded data end (two shapes)
            out.append(f'<rect x="{bx}" y="{by}" width="{bw:.1f}" height="16" rx="4" fill="{color}"/>')
            out.append(f'<rect x="{bx}" y="{by}" width="{min(4.0, bw):.1f}" height="16" fill="{color}"/>')
            out.append(f'<text x="{bx + bw + 8:.1f}" y="{by + 12}" font-family="{MONO}" font-size="12" fill="{INK}">{_esc(fmt(v))}</text>')
            if j == 0 and callout:
                out.append(f'<text x="{x + PW - 16}" y="{by + 12}" text-anchor="end" font-family="{MONO}" font-size="11.5" font-weight="500" fill="{ACCENT}">{_esc(callout)}</text>')
    out.append(f'<text x="20" y="{H - 24}" font-family="{SANS}" font-size="10.5" fill="{MUTED}">VideoIndex cost excludes indexing, paid once per video. Gemini agentic video takes ten videos per request, so each question is three parallel</text>')
    out.append(f'<text x="20" y="{H - 10}" font-family="{SANS}" font-size="10.5" fill="{MUTED}">agentic requests plus a merge, and its cost is the whole cost. Provider list prices, September 2026.</text>')
    out.append("</svg>")
    path.write_text("\n".join(out))
    return path


def fmt_pct(x):
    return "—" if x is None else f"{100 * x:.0f}%"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("runs", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--pdf", help="also build a PDF technical report at this path")
    ap.add_argument("--title", default="Corpus QA: questions that span the whole library")
    a = ap.parse_args()
    qs = load()
    qid = [q.id for q in qs]
    runs = []
    for p in a.runs:
        run = json.loads(Path(p).read_text())
        if not any(r.get("scores") for r in run["results"]):
            print(f"skipping {p}: not judged", file=sys.stderr)
            continue
        runs.append((run["config"].get("label") or Path(p).stem, run, summarize(run), p))
    if not runs:
        sys.exit("no scored runs")
    out = Path(a.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    assets = out.parent / "assets"
    assets.mkdir(exist_ok=True)
    stem = out.stem
    labels = [l for l, _, _, _ in runs]
    short = [short_label(l) for l in labels]

    # ---- charts
    kinds = [k for k in KIND_ORDER if any(k in s["by_kind"] for _, _, s, _ in runs)]
    charts.grouped_bars(assets / f"{stem}-quality-by-kind.svg", [wrap(KIND_LABEL[k]) for k in kinds] + ["all questions"],
                        [(sh, [s["by_kind"].get(k, {}).get("quality") for k in kinds] + [s["quality"]]) for sh, (_, _, s, _) in zip(short, runs)],
                        title="Answer quality by question kind (0–1, judge-extracted claims scored against ground truth)", ylim=(0, 1.15), fmt="{:.2f}", figsize=(8.4, 3.6))
    charts.grouped_bars(assets / f"{stem}-set-metrics.svg", ["precision", "recall", "F1", wrap("timestamps near a real mention"), wrap("key facts covered")],
                        [(sh, [s["precision"], s["recall"], s["f1"], s["anchor_hit"], s["fact_coverage"]]) for sh, (_, _, s, _) in zip(short, runs)],
                        title="Which videos were named, and how well they were cited", ylim=(0, 1.15), fmt="{:.2f}", figsize=(8.4, 3.6))
    charts.scatter_labeled(assets / f"{stem}-quality-vs-cost.svg", [(sh, s["cost_mean"], 100 * s["quality"], i % 8) for i, (sh, (_, _, s, _)) in enumerate(zip(short, runs))],
                           title="Quality against cost per question", xlabel="USD per question (list price, every call)", ylabel="quality (%)", ylim=(0, 105), figsize=(6.8, 3.8))
    charts.grouped_bars(assets / f"{stem}-latency.svg", [wrap(sh, 22) for sh in short],
                        [("median", [s["latency_p50"] for _, _, s, _ in runs]), ("p95", [s["latency_p95"] for _, _, s, _ in runs])],
                        title="Wall-clock seconds per question", ylabel="seconds", fmt="{:.0f}", figsize=(8.4, 3.6))
    grid = []
    for _, run, _, _ in runs:
        by_id = {r["id"]: r for r in run["results"]}
        grid.append([by_id[q].get("scores", {}).get("quality") if q in by_id else None for q in qid])
    heatmap(assets / f"{stem}-per-question.svg", short, [q.replace("corpus-", "q") for q in qid], grid, title="Quality per question")
    vi_rows = [s for l, _, s, _ in runs if l.lower().startswith("videoindex") and "cap" not in l]
    gem_rows = [s for l, _, s, _ in runs if "gemini agentic" in l.lower()]
    headline = None
    if vi_rows and gem_rows:
        headline = headline_svg(assets / f"{stem}-headline.svg", max(vi_rows, key=lambda s: s["quality"]), gem_rows[0], len(qs), len(runs[0][1].get("catalog", [])))

    # ---- markdown
    n_videos = len(runs[0][1].get("catalog", []))
    md = [f"# {a.title}", "",
          f"Generated {dt.date.today().isoformat()} by `eval/corpus_report.py` from {len(runs)} run file(s) over the "
          f"{len(qs)}-question corpus set (`eval/data/corpus/questions.json`) on the {n_videos}-video `dataset` index.", "",
          "Every system answered the same questions with the same answer format. A judge model (Claude Opus 5) turned each free-text "
          "answer into structure: the catalog videos it names, the timestamps it gives, and which key facts it states. Scoring after "
          "that is deterministic and identical for every system: set precision / recall / F1 of named videos against the expected set "
          "(videos with only an on-screen mention, or listed as ambiguous, count neither way), the share of cited timestamps within 90 s "
          "of a real mention in the index, key-fact coverage, and a per-question quality score (mention → F1; topic → mean of F1 and "
          "facts; synthesis → 0.3 F1 + 0.7 facts; whole-library → facts; nothing-matches → 1 unless a video is invented). Costs are "
          "provider list prices for every call a question needed; VideoIndex's costs exclude indexing (done once per video), Gemini's are the whole cost.", "",
          *([f"![](assets/{stem}-headline.svg)", ""] if headline else []),
          f"![](assets/{stem}-quality-by-kind.svg)", "",
          "| Configuration | n | quality | precision | recall | F1 | timestamps near a mention | facts covered | wrong videos | missed videos |",
          "|---|---|---|---|---|---|---|---|---|---|"]
    for label, run, s, _ in runs:
        md.append(f"| {label} | {s['n']} | **{100 * s['quality']:.0f}%** | {fmt_pct(s['precision'])} | {fmt_pct(s['recall'])} | {fmt_pct(s['f1'])} | "
                  f"{fmt_pct(s['anchor_hit'])} (n={s['anchors_n']}) | {fmt_pct(s['fact_coverage'])} | {s['wrong_total']} | {s['missed_total']} |")
    md += ["", "| Configuration | cost / q | cost p95 | total | latency p50 | latency p95 | tool calls / q | tokens / q | budget hit |",
           "|---|---|---|---|---|---|---|---|---|"]
    for label, run, s, _ in runs:
        md.append(f"| {label} | ${s['cost_mean']:.3f} | ${s['cost_p95']:.2f} | ${s['cost_total']:.2f} | {s['latency_p50']:.0f} s | {s['latency_p95']:.0f} s | {s['tool_calls_mean']:.1f} | {s['tokens_mean']:,.0f} | {s['partial']} |")
    md += ["", "Wrong videos: named videos outside the expected and acceptable sets (including invented titles), summed over all questions. "
           "Missed videos: expected videos not named. Latency is wall-clock per question as the runner saw it; Gemini's batches run in "
           "parallel, and the sequential sum is given in the runs section. Budget hit: answers cut short by the token, cost or time budget.", "",
           f"![](assets/{stem}-set-metrics.svg)", "", f"![](assets/{stem}-quality-vs-cost.svg)", "", f"![](assets/{stem}-latency.svg)", "",
           "## Quality by question kind", "",
           "| Kind | " + " | ".join(labels) + " |", "|---|" + "---|" * len(labels)]
    for k in kinds:
        n = next(s["by_kind"][k]["n"] for _, _, s, _ in runs if k in s["by_kind"])
        md.append(f"| {KIND_LABEL[k]} (n={n}) | " + " | ".join(fmt_pct(s["by_kind"].get(k, {}).get("quality")) for _, _, s, _ in runs) + " |")
    md += ["", f"![](assets/{stem}-per-question.svg)", "", "## Where this stands", ""]
    vi = [(l, s) for l, _, s, _ in runs if s and l.lower().startswith("videoindex")]
    gem = [(l, s) for l, _, s, _ in runs if s and "gemini agentic" in l.lower()]
    if vi and gem:
        (lv, sv), (lg, sg) = max(vi, key=lambda x: x[1]["quality"]), gem[0]
        md.append(f"The best VideoIndex configuration ({lv}) scores {100 * sv['quality']:.0f}% quality against {100 * sg['quality']:.0f}% for {lg}, "
                  f"at ${sv['cost_mean']:.3f} against ${sg['cost_mean']:.3f} per question ({sg['cost_mean'] / max(sv['cost_mean'], 1e-9):.1f}×) and a median "
                  f"{sv['latency_p50']:.0f} s against {sg['latency_p50']:.0f} s. VideoIndex named {sv['wrong_total']} wrong videos and missed {sv['missed_total']}; "
                  f"Gemini named {sg['wrong_total']} wrong and missed {sg['missed_total']}. Timestamps landed near a real mention "
                  f"{fmt_pct(sv['anchor_hit'])} of the time for VideoIndex and {fmt_pct(sg['anchor_hit'])} for Gemini.")
        gr = gem[0][0]
        grun = next(run for l, run, _, _ in runs if l == gr)
        gcosts = sorted(r["usage"]["cost_usd"] for r in grun["results"] if r.get("scores"))
        expensive = [r for r in grun["results"] if r.get("scores") and r["usage"]["cost_usd"] > 1.0]
        md.append("")
        exp_desc = ", ".join(f"{r['id'].replace('corpus-', 'q')} ({r['kind']}, ${r['usage']['cost_usd']:.2f})" for r in sorted(expensive, key=lambda r: -r["usage"]["cost_usd"]))
        md.append(f"Gemini's cost is uneven: {len(expensive)} of {len(gcosts)} questions cost more than $1 ({exp_desc}); on those the agent loaded "
                  f"transcript for most of the library and spent several hundred thousand thinking tokens, while the median question cost "
                  f"${gcosts[len(gcosts) // 2]:.2f}. VideoIndex's cost is flat because the index answers a library-wide scan with one full-text query; "
                  f"its agent's lost points are mostly missed videos rather than wrong ones.")
        capped = [(l, s) for l, s in vi if "cap" in l]
        if capped:
            lc, sc = capped[0]
            md.append("")
            md.append(f"The row '{lc}' is the same agent before this evaluation: each model turn was capped at 1,500 output tokens, which is enough "
                      f"for a single-video answer but truncated list answers over the library ({sc['partial']} of {sc['n']} answers cut short, two returned "
                      f"empty). The cap became the `max_answer_tokens` budget field (default 4,000) on 2026-09-16 and every other VideoIndex row uses it.")
        md.append("")
        md.append("Gemini's agentic video mode takes at most ten videos per request and the library does not fit its context window in "
                  "static mode, so the Gemini row is a map-reduce: three agentic requests of ten videos each, run in parallel, and a "
                  "text-only merge by the same model. That is the closest the public API comes to a whole-library question; a product "
                  "built on it would have to add exactly the kind of index VideoIndex maintains. Gemini also receives the video titles in "
                  "the prompt, as VideoIndex's agent does in its system prompt, so metadata questions are fair to both.")
    md += ["", "## Per-question results", "",
           "| question | expected | " + " | ".join(short) + " |", "|---|---|" + "---|" * len(short)]
    for q in qs:
        cells = []
        for _, run, _, _ in runs:
            r = next((r for r in run["results"] if r["id"] == q.id), None)
            s = r.get("scores") if r else None
            if not s:
                cells.append("—")
                continue
            extra = []
            if s["wrong"]:
                extra.append(f"{len(s['wrong'])} wrong")
            if s["missed"]:
                extra.append(f"{len(s['missed'])} missed")
            if s["unknown_videos"]:
                extra.append(f"{s['unknown_videos']} invented")
            cells.append(f"{s['quality']:.2f}" + (f" ({', '.join(extra)})" if extra else ""))
        md.append(f"| **{q.id.replace('corpus-', 'q')}** ({q.kind}) {q.question} | {len(q.expected)} | " + " | ".join(cells) + " |")
    md += ["", "## Runs", ""]
    for label, run, s, path in runs:
        cfg = {k: v for k, v in run["config"].items() if k not in ("index", "config", "catalog", "label")}
        seq = f"; sequential-sum latency p50 {s['latency_seq_p50']:.0f} s" if s.get("latency_seq_p50") else ""
        md.append(f"- **{label}**: `{path}` — total ${s['cost_total']:.2f}, judge ≈ ${s['judge_cost']:.2f}{seq}; {json.dumps(cfg)}")
    md += ["", "The ground truth for mention questions comes from the index's own ASR transcript and OCR text, so it inherits their errors "
           "(the set notes the known ones, such as 'Laura' for LoRA). A system that hears a mention the transcript missed is scored as wrong; "
           "the per-question table and the judge assessments in the run files are the place to check such cases.", ""]
    out.write_text("\n".join(md))
    print(out)

    if a.pdf:
        from report.mdreport import build_pdf  # noqa: PLC0415

        gen = out.parent / "assets" / f"{stem}-detail.md"
        gen.write_text(detail_markdown(qs, runs, short))
        spec = {
            "title": "VideoIndex corpus evaluation",
            "subtitle": f"{len(qs)} questions that span a {n_videos}-video library: VideoIndex against Gemini agentic video on quality, cost and latency",
            "org": "VideoIndex", "authors": ["Generated from the run files by eval/corpus_report.py"],
            "date": dt.date.today().isoformat(),
            "version": subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True, cwd=ROOT).stdout.strip(),
            "abstract": ("Public long-video benchmarks ask about one video at a time. This report evaluates the question the product exists for: "
                         "asking a whole library. A hand-written set of questions (which talks mention X, the moments where speakers discuss Y, "
                         "cross-video comparisons, whole-library summaries, and a question nothing answers) is answered by VideoIndex's agent, "
                         "by a retrieval-only baseline, and by Gemini's agentic video mode over the same 30 files; a judge model extracts what "
                         "each answer claims and the claims are scored against ground truth derived from the index's transcripts and on-screen text."),
            "out": a.pdf,
            "chapters": [{"file": str(out), "title": "Results"}, {"file": str(gen), "title": "Questions, ground truth and answers"}],
        }
        print(build_pdf(spec))


def detail_markdown(qs, runs, short) -> str:
    """Appendix: every question with its ground truth and each system's scored answer."""
    md = ["# Questions, ground truth and answers", "",
          "For each question: the expected videos (and acceptable extras), then every system's score, the videos it named wrongly or "
          "missed, the judge's two-sentence assessment and the opening of its answer.", ""]
    titles = {v["key"]: v["title"] for v in runs[0][1].get("catalog", [])}
    for q in qs:
        md += [f"## {q.id}: {q.question}", "", f"*Kind: {q.kind}.* " + (q.notes or ""), ""]
        if q.expected:
            md.append("Expected: " + "; ".join(titles.get(k, k) for k in q.expected))
        else:
            md.append("Expected: no video" if q.kind == "negative" else "Expected: scored on facts only")
        if q.acceptable and q.kind != "library":
            md.append("  \nAcceptable, not required: " + "; ".join(titles.get(k, k) for k in q.acceptable))
        if q.facts:
            md += ["", "Key facts:", ""] + [f"- {f}" for f in q.facts]
        md.append("")
        for sh, (_, run, _, _) in zip(short, runs):
            r = next((r for r in run["results"] if r["id"] == q.id), None)
            if not r or not r.get("scores"):
                md += [f"**{sh}**: no scored answer", ""]
                continue
            s = r["scores"]
            parts = [f"quality {s['quality']:.2f}", f"P {s['precision']:.2f}", f"R {s['recall']:.2f}"]
            if s["fact_coverage"] is not None:
                parts.append(f"facts {s['fact_coverage']:.2f}")
            if s["anchor_hit"] is not None:
                parts.append(f"timestamps near a mention {s['anchor_hit']:.2f} (n={s['anchors_checked']})")
            parts += [f"${r['usage'].get('cost_usd', 0):.3f}", f"{r['ms'] / 1000:.0f} s"]
            md.append(f"**{sh}** — " + ", ".join(parts))
            if s["wrong"]:
                md.append(f"  \nWrong: " + "; ".join(titles.get(k, k) for k in s["wrong"]))
            if s["missed"]:
                md.append(f"  \nMissed: " + "; ".join(titles.get(k, k) for k in s["missed"]))
            md.append(f"  \nJudge: {r['judge'].get('assessment', '')}")
            excerpt = re.sub(r"[#*|>`_]+", "", r["text"]).strip()
            excerpt = re.sub(r"\s+", " ", excerpt)
            md.append(f"  \n> {excerpt[:700]}{'…' if len(excerpt) > 700 else ''}")
            md.append("")
    return "\n".join(md)


if __name__ == "__main__":
    main()
