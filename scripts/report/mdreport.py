#!/usr/bin/env python3
"""Markdown documents to a styled technical report (PDF).

    python3 scripts/report/mdreport.py spec.json
    python3 scripts/report/mdreport.py --title "..." --out out.pdf a.md b.md ...

The spec is JSON:

    {"title": "...", "subtitle": "...", "org": "...", "authors": ["..."],
     "date": "2026-09-12", "version": "...", "out": "docs/reports/x.pdf",
     "abstract": "one paragraph shown on the cover",
     "chapters": [{"file": "docs/01-overview.md", "title": "Overview"}, ...],
     "html_out": "optional path to also write the HTML"}

Each chapter becomes a page-broken section. Markdown is rendered with
python-markdown (tables, fenced code with Pygments highlighting, footnotes,
definition lists, attribute lists); ```mermaid fences are drawn as SVG
through Graphviz / matplotlib (see diagrams.py); images and links between
the chapter files are resolved; a table of contents with page numbers, the
running header and page numbers come from WeasyPrint's paged CSS.
"""
from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import re
import sys
from pathlib import Path

import markdown
from markdown.extensions.toc import slugify_unicode
from pymdownx.superfences import fence_code_format  # noqa: F401  (registers pymdownx)

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from report.diagrams import UnsupportedDiagram, render_mermaid  # noqa: E402

HERE = Path(__file__).resolve().parent
CSS = (HERE / "report.css").read_text()


# ---------------------------------------------------------------- rendering

def mermaid_fence(source, language, css_class, options, md, **kwargs):
    try:
        svg = render_mermaid(source)
        return f'<figure class="diagram">{svg}</figure>'
    except (UnsupportedDiagram, Exception) as e:  # noqa: BLE001 - fall back to source
        return (
            f'<div class="diagram-fallback"><div class="caption">Diagram source (not rendered: {html.escape(str(e))})</div>'
            f"<pre><code>{html.escape(source)}</code></pre></div>"
        )


def make_md(prefix: str) -> markdown.Markdown:
    def slug(value, sep):
        return f"{prefix}--{slugify_unicode(value, sep)}"

    return markdown.Markdown(
        extensions=[
            "tables",
            "footnotes",
            "def_list",
            "attr_list",
            "md_in_html",
            "sane_lists",
            "toc",
            "pymdownx.superfences",
            "pymdownx.highlight",
            "pymdownx.tilde",
            "pymdownx.betterem",
        ],
        extension_configs={
            "toc": {"slugify": slug, "permalink": False, "toc_depth": "1-3"},
            "pymdownx.highlight": {"css_class": "highlight", "guess_lang": False, "noclasses": False},
            "pymdownx.superfences": {
                "custom_fences": [
                    {"name": "mermaid", "class": "mermaid", "format": mermaid_fence}
                ]
            },
        },
        output_format="html5",
    )


def chapter_prefix(path: Path) -> str:
    return re.sub(r"[^a-z0-9]+", "-", path.stem.lower()).strip("-")


def rewrite_links(body: str, src: Path, known: dict[str, str]) -> str:
    """Make image paths absolute and turn links to sibling .md files into anchors."""
    base = src.parent.resolve()

    def img(m):
        url = m.group(1)
        if re.match(r"^[a-z]+://", url) or url.startswith("data:"):
            return m.group(0)
        return f'src="{(base / url).resolve().as_uri()}"'

    body = re.sub(r'src="([^"]+)"', img, body)

    def link(m):
        url = html.unescape(m.group(1))
        if re.match(r"^[a-z]+://", url) or url.startswith("#") or url.startswith("mailto:"):
            return m.group(0)
        target, _, frag = url.partition("#")
        name = Path(target).name
        if name in known:
            prefix = known[name]
            return f'href="#{prefix}--{frag}"' if frag else f'href="#chapter-{prefix}"'
        # A path outside the report: keep the text, drop the dead link.
        return 'href="#"'

    body = re.sub(r'href="([^"]+)"', link, body)
    return body


def flatten_details(text: str) -> str:
    """`<details><summary>X</summary>…</details>` blocks (collapsible on the
    web) become a level-4 heading plus their content, so the Markdown inside
    them (tables, lists) still renders in print."""
    text = re.sub(r"<details[^>]*>\s*<summary>(.*?)</summary>\s*", lambda m: f"\n\n#### {m.group(1).strip()}\n\n", text, flags=re.S)
    return re.sub(r"\s*</details>\s*", "\n\n", text)


def render_chapter(path: Path, title: str | None, known: dict[str, str]) -> tuple[str, str, list]:
    text = flatten_details(path.read_text(encoding="utf-8"))
    prefix = known[path.name]
    md = make_md(prefix)
    body = md.convert(text)
    body = rewrite_links(body, path, known)
    # Chapter title: the first H1 if present, else the given title.
    m = re.search(r"<h1[^>]*>(.*?)</h1>", body, re.S)
    if m:
        h1 = re.sub(r"<[^>]+>", "", m.group(1))
        body = body.replace(m.group(0), "", 1)
    else:
        h1 = title or path.stem
    if title:
        h1 = title
    # Headings for the table of contents (h2 and h3 with ids).
    heads = [(int(t), i, re.sub(r"<[^>]+>", "", txt))
             for t, i, txt in re.findall(r'<h([23]) id="([^"]+)">(.*?)</h[23]>', body, re.S)]
    section = (
        f'<section class="chapter" id="chapter-{prefix}">'
        f'<h1 class="chapter-title" data-chapter="{html.escape(h1)}">{html.escape(h1)}</h1>'
        f"{body}</section>"
    )
    return section, h1, heads


def build_html(spec: dict) -> str:
    chapters = spec["chapters"]
    known = {Path(c["file"]).name: chapter_prefix(Path(c["file"])) for c in chapters}
    sections, toc = [], []
    for c in chapters:
        path = Path(c["file"])
        section, h1, heads = render_chapter(path, c.get("title"), known)
        sections.append(section)
        toc.append((known[path.name], h1, heads))
    date = spec.get("date") or dt.date.today().isoformat()
    authors = ", ".join(spec.get("authors", []))
    toc_html = ['<nav class="toc"><h1>Contents</h1><ol class="toc-l1">']
    for prefix, h1, heads in toc:
        toc_html.append(f'<li><a href="#chapter-{prefix}">{html.escape(h1)}</a>')
        h2s = [(lvl, i, t) for lvl, i, t in heads if lvl == 2]
        if h2s:
            toc_html.append('<ol class="toc-l2">')
            for _, i, t in h2s:
                toc_html.append(f'<li><a href="#{i}">{html.escape(t)}</a></li>')
            toc_html.append("</ol>")
        toc_html.append("</li>")
    toc_html.append("</ol></nav>")
    abstract = spec.get("abstract", "")
    cover = f"""
<section class="cover">
  <div class="cover-org">{html.escape(spec.get("org", ""))}</div>
  <h1 class="cover-title">{html.escape(spec["title"])}</h1>
  <div class="cover-subtitle">{html.escape(spec.get("subtitle", ""))}</div>
  <div class="cover-abstract">{markdown.markdown(abstract)}</div>
  <div class="cover-meta">
    <div>{html.escape(authors)}</div>
    <div>{html.escape(date)}{(" · " + html.escape(spec["version"])) if spec.get("version") else ""}</div>
  </div>
</section>"""
    return f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<title>{html.escape(spec["title"])}</title>
<style>{CSS}</style>
<style>{pygments_css()}</style>
</head>
<body data-title="{html.escape(spec["title"])}">
{cover}
{"".join(toc_html)}
{"".join(sections)}
</body></html>"""


def pygments_css() -> str:
    from pygments.formatters import HtmlFormatter

    return HtmlFormatter(style="friendly").get_style_defs(".highlight")


def build_pdf(spec: dict) -> Path:
    from weasyprint import HTML

    html_text = build_html(spec)
    if spec.get("html_out"):
        Path(spec["html_out"]).write_text(html_text, encoding="utf-8")
    out = Path(spec["out"])
    out.parent.mkdir(parents=True, exist_ok=True)
    HTML(string=html_text, base_url=str(Path.cwd())).write_pdf(str(out))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("spec", nargs="?", help="JSON spec file")
    ap.add_argument("files", nargs="*", help="Markdown files (when no spec)")
    ap.add_argument("--title")
    ap.add_argument("--subtitle", default="")
    ap.add_argument("--org", default="")
    ap.add_argument("--author", action="append", default=[])
    ap.add_argument("--date")
    ap.add_argument("--out")
    ap.add_argument("--html")
    a = ap.parse_args()
    if a.spec and a.spec.endswith(".json"):
        spec = json.loads(Path(a.spec).read_text())
    else:
        files = ([a.spec] if a.spec else []) + a.files
        if not files or not a.title or not a.out:
            ap.error("give a JSON spec, or --title, --out and markdown files")
        spec = {"title": a.title, "subtitle": a.subtitle, "org": a.org, "authors": a.author,
                "date": a.date, "out": a.out, "chapters": [{"file": f} for f in files]}
    if a.html:
        spec["html_out"] = a.html
    out = build_pdf(spec)
    print(out)


if __name__ == "__main__":
    main()
