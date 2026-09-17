#!/usr/bin/env python3
"""Brand headline charts for the single-video benchmarks: VideoIndex's best
agent against the Gemini agentic-video reference on LVBench and MINERVA.

    python3 -m eval.benchmark_headline \
        --lvbench /data/videoindex/eval/runs/lvbench-f0.25-s1-agent-gemini.json \
                  /data/videoindex/eval/runs/lvbench-f0.25-s1-gemini-3.8-flash-agentic.json \
        --minerva /data/videoindex/eval/runs/minerva-f0.25-s1-agent-gemini.json \
                  /data/videoindex/eval/runs/minerva-f0.25-s1-gemini-3.8-flash-agentic.json \
        --out-dir ../vi_internal/reports/assets --stem benchmarks-2026-09-17

Writes `<stem>-headline.svg` (accuracy, cost per question and median latency,
one row per benchmark) and `<stem>-by-type.svg` (accuracy per task type, two
facets), plus PNGs at 2x when `rsvg-convert` is available. Hand-built SVG in
the brand tokens, the same emphasis form as `corpus_report.headline_svg`:
VideoIndex in the accent hue, the reference in the muted ink; every value is
labelled and a legend names both series, so the muted gray carries no
identity on its own.
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import textwrap
from pathlib import Path

from eval.metrics import score

NAVY, ACCENT, MUTED, INK, INK2, LINE, CARD = "#0a2347", "#3186e9", "#8a8880", "#0b0b0b", "#4d4c48", "#e3e1d8", "#fffefb"
SANS = "IBM Plex Sans, Inter, system-ui, -apple-system, Segoe UI, sans-serif"
MONO = "IBM Plex Mono, ui-monospace, SF Mono, Menlo, monospace"
BENCH_LABEL = {"lvbench": "LVBench", "minerva": "MINERVA"}
BENCH_NOTE = {"lvbench": "hour-long videos, 6 task types", "minerva": "long videos, 13 task types"}


def _esc(t: str) -> str:
    return t.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def _text(x, y, s, size, fill, weight="400", family=SANS, anchor="start"):
    return (f'<text x="{x:.1f}" y="{y:.1f}" font-family="{family}" font-size="{size}" font-weight="{weight}" '
            f'fill="{fill}" text-anchor="{anchor}">{_esc(s)}</text>')


def _bar(x, y, w, h, color):
    # Square at the baseline, 4px rounded data end: two shapes.
    return (f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h}" rx="4" fill="{color}"/>'
            f'<rect x="{x:.1f}" y="{y:.1f}" width="{min(4.0, w):.1f}" height="{h}" fill="{color}"/>')


def _legend(x, y):
    out = [f'<rect x="{x}" y="{y - 9}" width="12" height="12" rx="3" fill="{ACCENT}"/>',
           _text(x + 18, y + 1, "VideoIndex agent (Gemini 3.8 Flash as chat model, over the index)", 11.5, INK2)]
    x2 = x + 18 + 6.4 * 62 + 16
    out += [f'<rect x="{x2}" y="{y - 9}" width="12" height="12" rx="3" fill="{MUTED}"/>',
            _text(x2 + 18, y + 1, "Gemini 3.8 Flash agentic video (reads the video file)", 11.5, INK2)]
    return out


def headline_svg(path: Path, rows: list[tuple[str, dict, dict]]) -> Path:
    """One row per benchmark, three panels: accuracy, cost per question, median latency."""
    panels = [
        ("Accuracy", "higher is better · exact option match", lambda s: 100 * s["accuracy"], lambda v: f"{v:.1f}%", 100.0, None),
        ("Cost per question", "lower is better · provider list prices", lambda s: s["cost_mean"], lambda v: f"${v:.3f}", None,
         lambda vi, gem: f"{gem / vi:.1f}× cheaper" if vi and gem > vi else None),
        ("Median latency", "lower is better · wall clock per question", lambda s: s["latency_p50"], lambda v: f"{v:.0f} s", None,
         lambda vi, gem: f"{gem / vi:.1f}× faster" if vi and gem > vi * 1.05 else None),
    ]
    PW, PH, PX0, GX, GY, ROW_LABEL_W = 300, 128, 20, 16, 18, 118
    LABEL_W, VAL_W = 82, 64
    W = 2 * PX0 + ROW_LABEL_W + len(panels) * PW + (len(panels) - 1) * GX
    n_q = {b: s_vi["n"] for b, s_vi, _ in rows}
    sub = textwrap.wrap("Single-video multiple-choice questions, 25% stratified samples (seed 1): "
                        + ", ".join(f"{BENCH_LABEL[b]} {n_q[b]} questions" for b in n_q)
                        + ". Same questions and videos for both systems; VideoIndex answers from its index of each video, Gemini from the video file.",
                        int((W - 40) / 6.3))
    foot = textwrap.wrap("VideoIndex cost excludes indexing, paid once per video; Gemini's is the whole cost. Both agents were limited to a small tool "
                         "budget per question. Gemini agentic video is Google's Interactions API with processing: agentic, run once and cached. September 2026.",
                         int((W - 40) / 5.3))
    y_legend = 50 + 17 * len(sub) + 6
    PY0 = y_legend + 22
    H = PY0 + len(rows) * PH + (len(rows) - 1) * GY + 24 + 14 * len(foot)
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" role="img" '
           f'aria-label="VideoIndex agent against Gemini agentic video on LVBench and MINERVA: accuracy, cost per question and median latency">',
           f'<title>VideoIndex agent against Gemini agentic video on the single-video benchmarks</title>',
           f'<rect width="{W}" height="{H}" fill="none"/>',
           _text(20, 30, "Single-video benchmarks: the agent against Gemini agentic video", 16, NAVY, "600")]
    for k, line in enumerate(sub):
        out.append(_text(20, 50 + 17 * k, line, 12.5, INK2))
    out += _legend(20, y_legend)
    for r, (bench, s_vi, s_gem) in enumerate(rows):
        y = PY0 + r * (PH + GY)
        out.append(_text(20, y + 26, BENCH_LABEL[bench], 15, NAVY, "600"))
        for k, line in enumerate(textwrap.wrap(BENCH_NOTE[bench], 20)):
            out.append(_text(20, y + 44 + 14 * k, line, 11, MUTED))
        out.append(_text(20, y + 44 + 14 * len(textwrap.wrap(BENCH_NOTE[bench], 20)), f"n = {s_vi['n']}", 11, MUTED, family=MONO))
        for i, (title, note, get, fmt, vmax_fixed, callout) in enumerate(panels):
            x = PX0 + ROW_LABEL_W + i * (PW + GX)
            out.append(f'<rect x="{x}" y="{y}" width="{PW}" height="{PH}" rx="12" fill="{CARD}" stroke="{LINE}"/>')
            out.append(_text(x + 16, y + 24, title, 13.5, INK, "600"))
            out.append(_text(x + 16, y + 40, note, 10.5, MUTED))
            v_vi, v_gem = get(s_vi), get(s_gem)
            bx = x + 16 + LABEL_W
            bw_max = PW - 32 - LABEL_W - VAL_W
            vmax = vmax_fixed or max(v_vi, v_gem) or 1.0
            for j, (name, v, color, weight) in enumerate([("VideoIndex", v_vi, ACCENT, "600"), ("Gemini", v_gem, MUTED, "400")]):
                by = y + 54 + j * 26
                bw = max(4.0, bw_max * v / vmax)
                out.append(_text(x + 16, by + 12, name, 12, NAVY if j == 0 else INK2, weight))
                out.append(_bar(bx, by, bw, 16, color))
                out.append(_text(bx + bw + 8, by + 12, fmt(v), 12, INK, family=MONO))
            if callout:
                c = callout(v_vi, v_gem)
                if c:
                    out.append(_text(x + PW - 16, y + PH - 12, c, 11.5, ACCENT, "500", MONO, "end"))
    for k, line in enumerate(foot):
        out.append(_text(20, H - 10 - 14 * (len(foot) - 1 - k), line, 10.5, MUTED))
    out.append("</svg>")
    path.write_text("\n".join(out))
    return path


def by_type_svg(path: Path, rows: list[tuple[str, dict, dict]], min_n: int = 5) -> Path:
    """Accuracy per task type, one facet per benchmark, paired horizontal bars."""
    FW, GX, PX0 = 430, 24, 20
    LABEL_W, VAL_W, BAR_H, PAIR_H = 176, 46, 12, 40
    W = 2 * PX0 + len(rows) * FW + (len(rows) - 1) * GX
    facets = []
    for bench, s_vi, s_gem in rows:
        types = [(t, p) for t, p in s_vi["per_type"].items() if p["n"] >= min_n and t in s_gem["per_type"]]
        types.sort(key=lambda tp: -tp[1]["n"])
        facets.append((bench, types, s_vi, s_gem))
    rows_max = max(len(t) for _, t, _, _ in facets)
    sub = textwrap.wrap("Accuracy per task type on the same samples. Types with fewer than five questions are left out; n is the number of "
                        "questions of that type in the sample.", int((W - 40) / 6.3))
    y_legend = 50 + 17 * len(sub) + 6
    FY0 = y_legend + 22
    FH = 62 + rows_max * PAIR_H + 12
    H = FY0 + FH + 30
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" role="img" '
           f'aria-label="Accuracy per task type, VideoIndex agent against Gemini agentic video, LVBench and MINERVA">',
           '<title>Accuracy per task type: VideoIndex agent against Gemini agentic video</title>',
           f'<rect width="{W}" height="{H}" fill="none"/>',
           _text(20, 30, "Where each system wins: accuracy by task type", 16, NAVY, "600")]
    for k, line in enumerate(sub):
        out.append(_text(20, 50 + 17 * k, line, 12.5, INK2))
    out += _legend(20, y_legend)
    for f, (bench, types, s_vi, s_gem) in enumerate(facets):
        x = PX0 + f * (FW + GX)
        y = FY0
        out.append(f'<rect x="{x}" y="{y}" width="{FW}" height="{FH}" rx="12" fill="{CARD}" stroke="{LINE}"/>')
        out.append(_text(x + 16, y + 24, f"{BENCH_LABEL[bench]}", 13.5, INK, "600"))
        out.append(_text(x + 16, y + 40, f"overall {100 * s_vi['accuracy']:.1f}% against {100 * s_gem['accuracy']:.1f}%", 11, MUTED, family=MONO))
        bx = x + 16 + LABEL_W
        bw_max = FW - 32 - LABEL_W - VAL_W
        # Hairline grid at 50% and 100%, recessive.
        for g in (0.5, 1.0):
            gx = bx + bw_max * g
            out.append(f'<line x1="{gx:.1f}" y1="{y + 54}" x2="{gx:.1f}" y2="{y + FH - 10}" stroke="{LINE}" stroke-width="1"/>')
            out.append(_text(gx, y + 52, f"{int(100 * g)}%", 9.5, MUTED, family=MONO, anchor="middle"))
        for i, (t, p_vi) in enumerate(types):
            p_gem = s_gem["per_type"][t]
            py = y + 62 + i * PAIR_H
            label = t if len(t) <= 27 else t[:26] + "…"
            out.append(_text(x + 16, py + 12, label, 11.5, INK))
            out.append(_text(x + 16, py + 25, f"n = {p_vi['n']}", 9.5, MUTED, family=MONO))
            for j, (p, color) in enumerate([(p_vi, ACCENT), (p_gem, MUTED)]):
                by = py + 2 + j * (BAR_H + 2)   # 2px surface gap between the pair
                bw = max(4.0, bw_max * p["accuracy"])
                out.append(_bar(bx, by, bw, BAR_H, color))
                out.append(_text(bx + bw + 6, by + 10, f"{100 * p['accuracy']:.0f}%", 10.5, INK, family=MONO))
    out.append(_text(20, H - 10, "Per-type intervals are wide on small n; read the direction, not the decimal. September 2026.", 10.5, MUTED))
    out.append("</svg>")
    path.write_text("\n".join(out))
    return path


def to_png(svg: Path, scale: int = 2) -> Path | None:
    if not shutil.which("rsvg-convert"):
        return None
    png = svg.with_suffix(".png")
    subprocess.run(["rsvg-convert", "-z", str(scale), "-b", "white", str(svg), "-o", str(png)], check=True)
    return png


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--lvbench", nargs=2, metavar=("VI_RUN", "GEMINI_RUN"), required=True)
    ap.add_argument("--minerva", nargs=2, metavar=("VI_RUN", "GEMINI_RUN"), required=True)
    ap.add_argument("--out-dir", required=True)
    ap.add_argument("--stem", default="benchmarks")
    ap.add_argument("--no-png", action="store_true")
    a = ap.parse_args()
    rows = []
    for bench, (vi_path, gem_path) in (("lvbench", a.lvbench), ("minerva", a.minerva)):
        vi = score(json.load(open(vi_path)))
        gem = score(json.load(open(gem_path)))
        rows.append((bench, vi, gem))
    out_dir = Path(a.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    for p in (headline_svg(out_dir / f"{a.stem}-headline.svg", rows), by_type_svg(out_dir / f"{a.stem}-by-type.svg", rows)):
        print(p)
        if not a.no_png:
            png = to_png(p)
            if png:
                print(png)


if __name__ == "__main__":
    main()
