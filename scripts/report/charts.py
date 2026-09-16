"""Charts for the technical reports (matplotlib, SVG output).

Follows the data-viz method the project uses: form by the data's job, color
by role (categorical slots in fixed order, one-hue sequential for magnitude),
thin marks with rounded data ends, hairline recessive grid, direct labels
only where they carry the story, a legend whenever there are two or more
series, text in ink tokens never in series color.
"""
from __future__ import annotations

from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.patches import FancyBboxPatch  # noqa: E402

# Reference palette (validated; see the dataviz skill's palette.md).
SERIES = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#4a3aa7", "#e34948"]
SEQ = {100: "#cde2fb", 250: "#86b6ef", 400: "#3987e5", 450: "#2a78d6", 550: "#1c5cab", 700: "#0d366b"}
DEEMPH = "#c3c2b7"
SURFACE = "#fcfcfb"
INK = "#0b0b0b"
INK2 = "#52514e"
MUTED = "#898781"
GRID = "#e1e0d9"
BASELINE = "#c3c2b7"
FONT = "Inter"

plt.rcParams.update({
    "font.family": FONT,
    "font.size": 9,
    "axes.edgecolor": BASELINE,
    "axes.labelcolor": INK2,
    "xtick.color": MUTED,
    "ytick.color": MUTED,
    "xtick.labelcolor": INK2,
    "ytick.labelcolor": INK2,
    "axes.titlecolor": INK,
    "figure.facecolor": SURFACE,
    "axes.facecolor": SURFACE,
    "savefig.facecolor": SURFACE,
    "legend.frameon": False,
    "legend.fontsize": 8.5,
    "axes.titlesize": 11,
    "axes.titleweight": "semibold",
    "axes.titlelocation": "left",
    "svg.fonttype": "none",
})


def _style(ax, *, horizontal=False, grid=True):
    for side in ("top", "right"):
        ax.spines[side].set_visible(False)
    ax.spines["left"].set_visible(not horizontal)
    ax.spines["bottom"].set_visible(True)
    ax.tick_params(length=0)
    if grid:
        ax.grid(axis="x" if horizontal else "y", color=GRID, linewidth=0.8, zorder=0)
    ax.set_axisbelow(True)


def _rounded_bar(ax, x, y, w, h, color, horizontal=False, radius=0.0, aspect=1.0):
    """A bar with a rounded data end and a square baseline end. `radius` is in
    x data units; `aspect` (y span / x span) keeps the corner round when the
    y axis spans hundreds of units and the x axis a few categories."""
    if h == 0 and not horizontal:
        return
    if horizontal:
        # x = start (baseline), w = value
        r = min(radius, w / 2 if w else 0)
        patch = FancyBboxPatch((x, y), w, h, boxstyle=f"round,pad=0,rounding_size={r}",
                               fc=color, ec="none", mutation_aspect=1)
        ax.add_patch(patch)
        # square the baseline end
        ax.add_patch(plt.Rectangle((x, y), min(r, w), h, fc=color, ec="none"))
    else:
        r = min(radius, w / 2, (h / 2) / aspect if h else 0)
        patch = FancyBboxPatch((x, y), w, h, boxstyle=f"round,pad=0,rounding_size={r}",
                               fc=color, ec="none", mutation_aspect=aspect)
        ax.add_patch(patch)
        ax.add_patch(plt.Rectangle((x, y), w, min(r * aspect, h), fc=color, ec="none"))


def save(fig, path: Path):
    path.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(path, format="svg", bbox_inches="tight", pad_inches=0.12)
    plt.close(fig)
    return path


def grouped_bars(path, categories, series, *, title, ylabel="", ylim=None, fmt="{:.2f}",
                 label_all=True, figsize=(7.2, 3.4), note=None):
    """Grouped vertical bars: `series` = [(name, [values...]), ...] in slot order."""
    fig, ax = plt.subplots(figsize=figsize)
    n = len(series)
    width = min(0.8 / n, 0.24)
    gap = 0.02
    xs = range(len(categories))
    top = ylim[1] if ylim else max(v for _, vals in series for v in vals if v is not None) * 1.18
    for i, (name, values) in enumerate(series):
        offs = (i - (n - 1) / 2) * (width + gap)
        for xi, v in zip(xs, values):
            if v is None:
                continue
            _rounded_bar(ax, xi + offs - width / 2, 0, width, v, SERIES[i], radius=0.3 * width, aspect=top / max(1, len(categories)))
            if label_all:
                ax.text(xi + offs, v + top * 0.015, fmt.format(v), ha="center", va="bottom", fontsize=7.8, color=INK2)
        ax.bar([0], [0], color=SERIES[i], label=name, width=0)  # legend proxy
    ax.set_xticks(list(xs))
    ax.set_xticklabels(categories)
    ax.set_xlim(-0.6, len(categories) - 0.4)
    ax.set_ylim(0, top)
    ax.set_ylabel(ylabel)
    legend_rows = -(-n // 3)
    ax.set_title(title, pad=12 + 14 * legend_rows)
    _style(ax)
    ax.legend(loc="lower left", bbox_to_anchor=(0, 1.0), ncol=min(n, 3), handlelength=1.0, handleheight=1.0, borderaxespad=0)
    if note:
        fig.text(0.01, -0.06, note, fontsize=7.5, color=MUTED, ha="left")
    return save(fig, Path(path))


def hbars(path, labels, values, *, title, xlabel="", fmt="{:.0f}", colors=None, xlim=None,
          figsize=(7.2, 3.2), note=None, highlight=None):
    """Horizontal bars for one measure across items (single hue; emphasis optional)."""
    fig, ax = plt.subplots(figsize=figsize)
    n = len(labels)
    ys = list(range(n))[::-1]
    vmax = xlim[1] if xlim else max(values) * 1.15
    for y, v, lab in zip(ys, values, labels):
        color = SERIES[0]
        if colors:
            color = colors[labels.index(lab)]
        elif highlight is not None:
            color = SERIES[0] if lab == highlight else DEEMPH
        _rounded_bar(ax, 0, y - 0.3, v, 0.6, color, horizontal=True, radius=0.02 * vmax)
        ax.text(v + vmax * 0.012, y, fmt.format(v), va="center", ha="left", fontsize=8, color=INK2)
    ax.set_yticks(ys)
    ax.set_yticklabels(labels)
    ax.set_ylim(-0.7, n - 0.3)
    ax.set_xlim(0, vmax)
    ax.set_xlabel(xlabel)
    ax.set_title(title, pad=10)
    _style(ax, horizontal=True)
    ax.spines["bottom"].set_visible(False)
    ax.tick_params(axis="x", labelsize=7.5)
    if note:
        fig.text(0.01, -0.16, note, fontsize=7.5, color=MUTED, ha="left")
    return save(fig, Path(path))


def scatter_labeled(path, points, *, title, xlabel, ylabel, figsize=(6.4, 3.8), xlog=False,
                    ylim=None, note=None, groups=None):
    """Labeled dots. `points` = [(label, x, y, group_index)], groups = group names."""
    fig, ax = plt.subplots(figsize=figsize)
    for label, x, y, g in points:
        color = SERIES[g]
        ax.scatter([x], [y], s=64, color=color, edgecolors=SURFACE, linewidths=2, zorder=3)
        ax.annotate(label, (x, y), textcoords="offset points", xytext=(8, 6), fontsize=8, color=INK)
    if xlog:
        ax.set_xscale("log")
    if ylim:
        ax.set_ylim(*ylim)
    ax.set_xlabel(xlabel)
    ax.set_ylabel(ylabel)
    ax.set_title(title, pad=10)
    _style(ax)
    ax.grid(axis="x", color=GRID, linewidth=0.8, zorder=0)
    if groups:
        for i, g in enumerate(groups):
            ax.scatter([], [], color=SERIES[i], label=g, s=40)
        ax.legend(loc="lower right")
    if note:
        fig.text(0.01, -0.08, note, fontsize=7.5, color=MUTED, ha="left")
    return save(fig, Path(path))


def dot_strip(path, labels, values, *, title, xlabel, fmt="{:.0f}×", figsize=(7.2, 6.0), median=None):
    """One dot per item on a shared axis (long lists; sorted by the caller)."""
    fig, ax = plt.subplots(figsize=figsize)
    ys = list(range(len(labels)))[::-1]
    ax.hlines(ys, 0, values, color=GRID, linewidth=1, zorder=1)
    ax.scatter(values, ys, s=42, color=SERIES[0], edgecolors=SURFACE, linewidths=2, zorder=3)
    for y, v in zip(ys, values):
        ax.text(v + max(values) * 0.015, y, fmt.format(v), va="center", fontsize=7.5, color=INK2)
    if median is not None:
        ax.axvline(median, color=MUTED, linewidth=1, zorder=2)
        ax.text(median, len(labels) - 0.2, f"median {fmt.format(median)}", fontsize=7.5, color=INK2, ha="left", va="bottom")
    ax.set_yticks(ys)
    ax.set_yticklabels(labels, fontsize=7.5)
    ax.set_xlim(0, max(values) * 1.15)
    ax.set_ylim(-0.8, len(labels) - 0.2 + 0.6)
    ax.set_xlabel(xlabel)
    ax.set_title(title, pad=10)
    _style(ax, horizontal=True)
    ax.spines["bottom"].set_visible(False)
    return save(fig, Path(path))
