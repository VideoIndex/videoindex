"""Render the mermaid diagrams used in docs/ without a JavaScript runtime.

Flowcharts, state diagrams and entity-relationship diagrams become Graphviz
DOT (rendered with `dot -Tsvg`); sequence diagrams are drawn directly with
matplotlib. Only the mermaid subset the docs use is supported; anything else
falls back to a preformatted block of the source.
"""
from __future__ import annotations

import html
import re
import shutil
import subprocess
import textwrap
from dataclasses import dataclass, field

FONT = "Inter"
INK = "#0b0b0b"
INK2 = "#52514e"
MUTED = "#898781"
BORDER = "#c3c2b7"
FILL = "#f3f2ee"
CLUSTER_FILL = "#fcfcfb"
ACCENT = "#2a78d6"
ACCENT_FILL = "#e3eefb"


class UnsupportedDiagram(Exception):
    pass


def dot_available() -> bool:
    return shutil.which("dot") is not None


def render_dot(dot: str) -> str:
    """DOT source to an SVG string (without the XML prologue)."""
    out = subprocess.run(
        ["dot", "-Tsvg"], input=dot.encode(), capture_output=True, check=True
    ).stdout.decode()
    # Drop the XML/DOCTYPE header so the SVG can be inlined in HTML.
    i = out.find("<svg")
    svg = out[i:]
    # Let CSS size it: remove fixed width/height, keep viewBox.
    svg = re.sub(r'<svg([^>]*?) width="[^"]*"', r"<svg\1", svg, count=1)
    svg = re.sub(r'<svg([^>]*?) height="[^"]*"', r"<svg\1", svg, count=1)
    return svg


def _esc(s: str) -> str:
    return s.replace("\\", "\\\\").replace('"', '\\"')


def _wrap(label: str, width: int = 26) -> str:
    """Wrap long node labels so boxes stay narrow; DOT takes \\n line breaks."""
    if len(label) <= width:
        return label
    return "\\n".join(textwrap.wrap(label, width, break_long_words=False, break_on_hyphens=False))  # DOT newline


def _strip_quotes(s: str) -> str:
    s = s.strip()
    if len(s) >= 2 and s[0] == s[-1] and s[0] in "\"'":
        s = s[1:-1]
    return html.unescape(s)


# ----------------------------------------------------------------- flowchart

NODE_RE = re.compile(
    r"""^\s*([A-Za-z0-9_]+)\s*(?:(\(\(|\[\[|\[|\(|\{)\s*(.*?)\s*(\)\)|\]\]|\]|\)|\}))?\s*$"""
)


@dataclass
class Flow:
    direction: str = "TB"
    nodes: dict = field(default_factory=dict)  # id -> (label, shape)
    edges: list = field(default_factory=list)  # (src, dst, label)
    clusters: list = field(default_factory=list)  # (id, label, [node ids])
    node_cluster: dict = field(default_factory=dict)


def _split_amp(seg: str) -> list[str]:
    """Split `A & B` fan-outs on `&` outside brackets and quotes."""
    parts, depth, quote, cur = [], 0, None, []
    for ch in seg:
        if quote:
            cur.append(ch)
            if ch == quote:
                quote = None
            continue
        if ch in "\"'":
            quote = ch
            cur.append(ch)
        elif ch in "[({":
            depth += 1
            cur.append(ch)
        elif ch in "])}":
            depth -= 1
            cur.append(ch)
        elif ch == "&" and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    parts.append("".join(cur))
    return [p.strip() for p in parts if p.strip()]


def _split_edges(line: str) -> list[str]:
    """Split a chain on arrows outside brackets and quotes, keeping the arrows."""
    out, depth, quote, cur, i = [], 0, None, [], 0
    arrows = ("-.->", "-->", "---", "==>")
    while i < len(line):
        ch = line[i]
        if quote:
            cur.append(ch)
            if ch == quote:
                quote = None
            i += 1
            continue
        if ch in "\"'":
            quote = ch
        elif ch in "[({":
            depth += 1
        elif ch in "])}":
            depth -= 1
        if depth == 0 and quote is None:
            hit = next((a for a in arrows if line.startswith(a, i)), None)
            if hit:
                out.append("".join(cur).strip())
                out.append(hit)
                cur = []
                i += len(hit)
                continue
        cur.append(ch)
        i += 1
    out.append("".join(cur).strip())
    return out


def _parse_node(token: str, flow: Flow, cluster: str | None) -> str:
    m = NODE_RE.match(token)
    if not m:
        raise UnsupportedDiagram(f"cannot parse node '{token}'")
    nid, open_, label, close = m.groups()
    shape = "box"
    if open_ in ("(", "(("):
        shape = "ellipse" if open_ == "((" else "box"
    if open_ == "{":
        shape = "diamond"
    if label is not None:
        flow.nodes[nid] = (_strip_quotes(label), shape)
    elif nid not in flow.nodes:
        flow.nodes[nid] = (nid, shape)
    if cluster and nid not in flow.node_cluster:
        flow.node_cluster[nid] = cluster
        for cid, _, members in flow.clusters:
            if cid == cluster:
                members.append(nid)
    return nid


def parse_flowchart(src: str) -> Flow:
    lines = [l.rstrip() for l in src.strip().splitlines()]
    head = lines[0].split()
    flow = Flow(direction=head[1] if len(head) > 1 else "TB")
    cluster: str | None = None
    for raw in lines[1:]:
        line = raw.strip()
        if not line or line.startswith("%%"):
            continue
        m = re.match(r'^subgraph\s+([A-Za-z0-9_]+)\s*(?:\[\s*"?(.*?)"?\s*\])?$', line)
        if m:
            cid, label = m.group(1), m.group(2) or m.group(1)
            flow.clusters.append((cid, html.unescape(label), []))
            cluster = cid
            continue
        if line == "end":
            cluster = None
            continue
        # Split a chain: A --> B -->|"x"| C & D
        parts = _split_edges(line)
        if len(parts) == 1:
            for tok in _split_amp(parts[0]):
                _parse_node(tok, flow, cluster)
            continue
        prev_group: list[str] | None = None
        i = 0
        while i < len(parts):
            seg = parts[i]
            label = ""
            if i > 0 and parts[i - 1] in ("-->", "---", "-.->", "==>"):
                lm = re.match(r'^\|\s*"?(.*?)"?\s*\|\s*(.*)$', seg)
                if lm:
                    label, seg = html.unescape(lm.group(1)), lm.group(2)
            group = [_parse_node(t, flow, cluster) for t in _split_amp(seg)]
            if prev_group is not None:
                for a in prev_group:
                    for b in group:
                        flow.edges.append((a, b, label))
            prev_group = group
            i += 2
    return flow


def flowchart_to_dot(flow: Flow) -> str:
    rankdir = {"TB": "TB", "TD": "TB", "LR": "LR", "BT": "BT", "RL": "RL"}[flow.direction]
    out = [
        "digraph G {",
        f'  rankdir={rankdir}; bgcolor="transparent"; pad=0.1; nodesep=0.3; ranksep=0.45;',
        f'  node [shape=box style="rounded,filled" fillcolor="{FILL}" color="{BORDER}" fontname="{FONT}" fontsize=10 fontcolor="{INK}" margin="0.18,0.08" penwidth=1];',
        f'  edge [color="{MUTED}" arrowsize=0.7 penwidth=1 fontname="{FONT}" fontsize=8.5 fontcolor="{INK2}"];',
    ]
    cluster_ids = {cid for cid, _, _ in flow.clusters}
    for cid, label, members in flow.clusters:
        out.append(f'  subgraph cluster_{cid} {{ label="{_esc(label)}"; labeljust=l; fontname="{FONT}"; fontsize=9.5; fontcolor="{INK2}"; style="rounded,filled"; fillcolor="{CLUSTER_FILL}"; color="{BORDER}"; margin=10;')
        for nid in members:
            label_n, shape = flow.nodes[nid]
            out.append(f'    "{nid}" [label="{_wrap(_esc(label_n))}" shape={shape}];')
        # Stack a wide cluster's nodes into columns of three with invisible
        # edges so a layer with eight members is not eight boxes in a row.
        if len(members) > 3:
            cols = [members[i:i + 3] for i in range(0, len(members), 3)]
            for col in cols:
                for a, b in zip(col, col[1:]):
                    out.append(f'    "{a}" -> "{b}" [style=invis weight=20];')
        out.append("  }")
    for nid, (label, shape) in flow.nodes.items():
        if nid in flow.node_cluster or nid in cluster_ids:
            continue
        out.append(f'  "{nid}" [label="{_wrap(_esc(label))}" shape={shape}];')
    for a, b, label in flow.edges:
        attrs = []
        if label:
            attrs.append(f'label="{_esc(label)}"')
        # Edges between clusters: point at a member and clip to the cluster.
        a_is_cluster, b_is_cluster = a in cluster_ids, b in cluster_ids
        src = a if not a_is_cluster else _cluster_anchor(flow, a)
        dst = b if not b_is_cluster else _cluster_anchor(flow, b)
        if a_is_cluster:
            attrs.append(f'ltail="cluster_{a}"')
        if b_is_cluster:
            attrs.append(f'lhead="cluster_{b}"')
        if src is None or dst is None:
            continue
        out.append(f'  "{src}" -> "{dst}" [{" ".join(attrs)}];')
    if any(a in cluster_ids or b in cluster_ids for a, b, _ in flow.edges):
        out.insert(1, "  compound=true;")
    out.append("}")
    return "\n".join(out)


def _cluster_anchor(flow: Flow, cid: str) -> str | None:
    for c, _, members in flow.clusters:
        if c == cid and members:
            return members[0]
    return None


# --------------------------------------------------------------- state chart

def state_to_dot(src: str) -> str:
    out = [
        "digraph S {",
        f'  rankdir=TB; bgcolor="transparent"; nodesep=0.35; ranksep=0.4;',
        f'  node [shape=box style="rounded,filled" fillcolor="{FILL}" color="{BORDER}" fontname="{FONT}" fontsize=10 fontcolor="{INK}" margin="0.18,0.08"];',
        f'  edge [color="{MUTED}" arrowsize=0.7 fontname="{FONT}" fontsize=8.5 fontcolor="{INK2}"];',
        f'  start [shape=circle label="" width=0.18 style=filled fillcolor="{INK}" color="{INK}"];',
        f'  finish [shape=doublecircle label="" width=0.16 style=filled fillcolor="{INK}" color="{INK}"];',
    ]
    for raw in src.strip().splitlines()[1:]:
        line = raw.strip()
        m = re.match(r"^(\[\*\]|[A-Za-z0-9_]+)\s*-->\s*(\[\*\]|[A-Za-z0-9_]+)\s*(?::\s*(.*))?$", line)
        if not m:
            continue
        a, b, label = m.groups()
        a = "start" if a == "[*]" else a
        b = "finish" if b == "[*]" else b
        attrs = f' [label="{_esc(label.strip())}"]' if label else ""
        out.append(f'  "{a}" -> "{b}"{attrs};')
    out.append("}")
    return "\n".join(out)


# ------------------------------------------------------------------ ER chart

def er_to_dot(src: str) -> str:
    out = [
        "digraph E {",
        f'  rankdir=LR; bgcolor="transparent"; nodesep=0.25; ranksep=0.7; splines=true;',
        f'  node [shape=box style="rounded,filled" fillcolor="{ACCENT_FILL}" color="{ACCENT}" fontname="{FONT}" fontsize=9.5 fontcolor="{INK}" margin="0.14,0.06"];',
        f'  edge [color="{MUTED}" arrowsize=0.6 fontname="{FONT}" fontsize=8 fontcolor="{INK2}" arrowhead=crow];',
    ]
    for raw in src.strip().splitlines()[1:]:
        line = raw.strip()
        m = re.match(r"^([A-Z_]+)\s+(\|\||\|o|\}o|\}\|)--(\|\||o\||o\{|\|\{)\s+([A-Z_]+)\s*:\s*(.*)$", line)
        if not m:
            continue
        a, _l, _r, b, label = m.groups()
        out.append(f'  "{a}" -> "{b}" [label="{_esc(label.strip())}"];')
    out.append("}")
    return "\n".join(out)


# ---------------------------------------------------------- sequence diagram

def sequence_to_svg(src: str) -> str:
    """Draw a sequence diagram with matplotlib and return inline SVG."""
    import io

    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.patches import FancyBboxPatch

    participants: list[tuple[str, str]] = []
    messages: list[tuple[str, str, str, bool]] = []  # (from, to, text, dashed)
    blocks: list[tuple[str, int, int]] = []  # (label, first msg idx, last msg idx)
    open_blocks: list[tuple[str, int]] = []
    for raw in src.strip().splitlines()[1:]:
        line = raw.strip()
        if not line:
            continue
        m = re.match(r"^participant\s+([A-Za-z0-9_]+)(?:\s+as\s+(.*))?$", line)
        if m:
            participants.append((m.group(1), m.group(2) or m.group(1)))
            continue
        m = re.match(r"^(alt|opt|loop)\s+(.*)$", line)
        if m:
            open_blocks.append((f"{m.group(1)} {m.group(2)}", len(messages)))
            continue
        if line == "end" and open_blocks:
            label, start = open_blocks.pop()
            blocks.append((label, start, len(messages) - 1))
            continue
        m = re.match(r"^([A-Za-z0-9_]+)\s*(-->>|->>|-->|->)\s*([A-Za-z0-9_]+)\s*:\s*(.*)$", line)
        if m:
            a, arrow, b, text = m.groups()
            text = re.sub(r"\s*//.*$", "", text)  # drop trailing comments
            messages.append((a, b, text, arrow.startswith("--")))
    if not participants or not messages:
        raise UnsupportedDiagram("empty sequence diagram")

    ids = [p for p, _ in participants]
    x = {pid: i for i, pid in enumerate(ids)}
    n = len(ids)
    row_h = 0.46
    height = 1.1 + row_h * (len(messages) + 1)
    width = max(6.0, 1.35 * n)
    fig, ax = plt.subplots(figsize=(width, height), dpi=100)
    ax.set_xlim(-0.6, n - 0.4)
    ax.set_ylim(-(len(messages) + 1) * row_h - 0.3, 0.9)
    ax.axis("off")
    for pid, label in participants:
        xi = x[pid]
        ax.add_patch(FancyBboxPatch((xi - 0.42, 0.3), 0.84, 0.5, boxstyle="round,pad=0.02,rounding_size=0.06",
                                    fc=FILL, ec=BORDER, lw=1))
        ax.text(xi, 0.55, label, ha="center", va="center", fontsize=10, color=INK, fontname=FONT, wrap=True)
        ax.plot([xi, xi], [0.3, -(len(messages) + 1) * row_h - 0.1], color=BORDER, lw=0.8, zorder=0)
    for label, s, e in blocks:
        y_top = -(s + 0.4) * row_h
        y_bot = -(e + 1.55) * row_h
        ax.add_patch(FancyBboxPatch((-0.5, y_bot), n, y_top - y_bot, boxstyle="square,pad=0", fc="#f7f7f5", ec=BORDER, lw=0.8, ls="-", zorder=0))
        ax.text(-0.46, y_top - 0.02, label, ha="left", va="top", fontsize=9, color=INK2, fontname=FONT)
    for i, (a, b, text, dashed) in enumerate(messages):
        y = -(i + 1) * row_h
        xa, xb = x[a], x[b]
        ax.annotate("", xy=(xb, y), xytext=(xa, y),
                    arrowprops=dict(arrowstyle="-|>", color=ACCENT if not dashed else MUTED,
                                    lw=1.2, linestyle="--" if dashed else "-", shrinkA=0, shrinkB=2))
        ax.text((xa + xb) / 2, y + 0.06, text, ha="center", va="bottom", fontsize=9.2, color=INK, fontname=FONT)
    buf = io.StringIO()
    fig.savefig(buf, format="svg", bbox_inches="tight", transparent=True)
    plt.close(fig)
    svg = buf.getvalue()
    i = svg.find("<svg")
    svg = svg[i:]
    svg = re.sub(r'<svg([^>]*?) width="[^"]*"', r"<svg\1", svg, count=1)
    svg = re.sub(r'<svg([^>]*?) height="[^"]*"', r"<svg\1", svg, count=1)
    return svg


# ----------------------------------------------------------------- dispatch

def render_mermaid(src: str) -> str:
    """Mermaid source to inline SVG; raises UnsupportedDiagram when it cannot."""
    first = src.strip().splitlines()[0].strip() if src.strip() else ""
    kind = first.split()[0] if first else ""
    if kind in ("flowchart", "graph"):
        if not dot_available():
            raise UnsupportedDiagram("graphviz not installed")
        flow = parse_flowchart(src)
        # Render in the declared direction and in the other one; keep the
        # orientation that needs the least shrinking to fit a portrait page
        # (about 650 x 620 pt of usable area). Long chains and wide layered
        # graphs otherwise scale down to unreadable.
        def fit(svg_text: str) -> float:
            m = re.search(r'viewBox="[\d.]+ [\d.]+ ([\d.]+) ([\d.]+)"', svg_text)
            if not m:
                return 1.0
            w, h = float(m.group(1)), float(m.group(2))
            return max(w / 650.0, h / 620.0, 1.0)

        first = render_dot(flowchart_to_dot(flow))
        other_dir = "LR" if flow.direction in ("TB", "TD", "BT") else "TB"
        alt_flow = parse_flowchart(src)
        alt_flow.direction = other_dir
        alt = render_dot(flowchart_to_dot(alt_flow))
        svg = first if fit(first) <= fit(alt) * 1.05 else alt
        return svg
    if kind.startswith("stateDiagram"):
        if not dot_available():
            raise UnsupportedDiagram("graphviz not installed")
        return render_dot(state_to_dot(src))
    if kind == "erDiagram":
        if not dot_available():
            raise UnsupportedDiagram("graphviz not installed")
        return render_dot(er_to_dot(src))
    if kind == "sequenceDiagram":
        return sequence_to_svg(src)
    raise UnsupportedDiagram(f"unsupported diagram type '{kind}'")
