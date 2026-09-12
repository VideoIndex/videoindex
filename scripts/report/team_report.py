#!/usr/bin/env python3
"""Build the team catch-up report: architecture, milestones and the work
performed so far, as one PDF.

    python3 scripts/report/team_report.py [--out docs/reports/videoindex-team-report-YYYY-MM-DD.pdf]

The design documents are included as chapters; a "Work performed" chapter is
generated from the git history and the source tree at build time.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from report.mdreport import build_pdf  # noqa: E402

GEN = ROOT / "docs" / "reports" / "generated"

TYPES = {"feat": "Features", "fix": "Fixes", "perf": "Performance", "docs": "Documentation",
         "ci": "CI", "chore": "Chores", "test": "Tests", "refactor": "Refactors"}


def sh(*cmd) -> str:
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, check=True).stdout


def rust_loc() -> list[tuple[str, int, int, int]]:
    """(crate, lines of Rust in src, lines in tests/examples, number of files)."""
    rows = []
    for crate in sorted((ROOT / "crates").iterdir()):
        if not (crate / "Cargo.toml").is_file():
            continue
        src = sum(len(p.read_text(errors="replace").splitlines()) for p in (crate / "src").rglob("*.rs"))
        other = 0
        for sub in ("tests", "examples", "benches"):
            if (crate / sub).is_dir():
                other += sum(len(p.read_text(errors="replace").splitlines()) for p in (crate / sub).rglob("*.rs"))
        files = len(list(crate.rglob("*.rs")))
        rows.append((crate.name, src, other, files))
    py = ROOT / "bindings" / "python"
    if py.is_dir():
        src = sum(len(p.read_text(errors="replace").splitlines()) for p in (py / "src").rglob("*.rs"))
        pyl = sum(len(p.read_text(errors="replace").splitlines()) for p in (py / "python").rglob("*.py")) \
            + sum(len(p.read_text(errors="replace").splitlines()) for p in (py / "tests").rglob("*.py"))
        rows.append(("bindings/python", src, pyl, len(list(py.rglob("*.rs"))) + len(list(py.rglob("*.py")))))
    return rows


def test_counts() -> dict[str, int]:
    out: dict[str, int] = {}
    for p in ROOT.rglob("*.rs"):
        if "target" in p.parts:
            continue
        n = len(re.findall(r"#\[(?:tokio::)?test", p.read_text(errors="replace")))
        if n:
            crate = p.relative_to(ROOT).parts[1] if p.relative_to(ROOT).parts[0] == "crates" else p.relative_to(ROOT).parts[0]
            out[crate] = out.get(crate, 0) + n
    for p in (ROOT / "bindings" / "python" / "tests").glob("test_*.py"):
        out["bindings/python"] = out.get("bindings/python", 0) + len(re.findall(r"^def test_", p.read_text(), re.M))
    return out


def work_log_md() -> str:
    log = sh("git", "log", "--reverse", "--format=%h%x09%ad%x09%s", "--date=short")
    commits = [l.split("\t") for l in log.splitlines() if l.strip()]
    by_day: dict[str, list] = defaultdict(list)
    types = Counter()
    crates = Counter()
    for h, day, subj in commits:
        m = re.match(r"^(\w+)(?:\(([^)]*)\))?!?:\s*(.*)$", subj)
        typ, scope, msg = (m.group(1), m.group(2), m.group(3)) if m else ("other", None, subj)
        types[typ] += 1
        for s in (scope or "").split(","):
            s = s.strip()
            if s:
                crates[s] += 1
        by_day[day].append((h, typ, scope, msg))
    first, last = commits[0][1], commits[-1][1]
    loc = rust_loc()
    tests = test_counts()
    total_src = sum(r[1] for r in loc)
    total_other = sum(r[2] for r in loc)
    lines = [
        "# Work performed",
        "",
        f"This chapter is generated from the repository at build time ({dt.date.today().isoformat()}). "
        f"The project has {len(commits)} commits between {first} and {last}; the Rust workspace holds "
        f"{total_src:,} lines of library and binary source plus {total_other:,} lines of tests, examples and Python, "
        f"with {sum(tests.values())} automated tests.",
        "",
        "## Commits by type",
        "",
        "| Type | Commits | Meaning |",
        "|---|---|---|",
    ]
    for t, n in types.most_common():
        lines.append(f"| `{t}` | {n} | {TYPES.get(t, '')} |")
    lines += ["", "## Code by crate", "", "| Crate | Source lines | Test / example / Python lines | Files | Tests |", "|---|---|---|---|---|"]
    for name, src, other, files in loc:
        lines.append(f"| `{name}` | {src:,} | {other:,} | {files} | {tests.get(name, 0)} |")
    lines.append(f"| **total** | **{total_src:,}** | **{total_other:,}** | | **{sum(tests.values())}** |")
    lines += ["", "## Commit log by day", "",
              "Conventional-commit subjects, oldest first. Scopes name the crates a change touched."]
    for day in sorted(by_day):
        lines += ["", f"### {day}", ""]
        for h, typ, scope, msg in by_day[day]:
            sc = f" `{scope}`" if scope else ""
            lines.append(f"- **{typ}**{sc}: {msg} (`{h}`)")
    return "\n".join(lines) + "\n"


EXEC_SUMMARY = """# Executive summary

VideoIndex turns long videos into a queryable knowledge base: an SDK and infrastructure
framework whose core is Rust, with a Python binding, a CLI and (planned) a server with HTTP,
SSE and MCP. Applications such as a video question-answering chat sit on top of the SDK.

## Where the project stands

- **M0 (skeleton) and M1 (coarse index) are complete.** Every video that enters the system is
  decoded in a sandboxed worker, sampled at 1 fps, hashed, thumbnailed, cut into shots, transcribed
  (Silero VAD + Whisper large-v3 on the GPU), read for on-screen text (RapidOCR), and embedded
  (SigLIP for frames, bge-small for text). Hybrid search fuses BM25, text vectors and image vectors.
- **M2 is largely delivered.** Provider adapters for OpenAI-compatible servers, Anthropic and Gemini;
  an agent loop with tools, budgets, sessions and timestamp citations (`vi ask`); the fine-pass
  operators (scenes, chapters, VLM descriptions, entities and events); a Python binding with
  operators and agent policies written in Python.
- **Measured on a 30-video, 36.6-hour dataset** (AI Engineer conference workshops and the Berkeley
  Agentic AI MOOC): retrieval hit@5 0.94 and MRR 0.72 on 72 questions; question answering 96.2%
  with citations on 52 questions at $0.05 per question, against a 73.1% retrieval-only baseline.
  Coarse indexing runs at 56× real time on the GPU build.

## What this document contains

Part I reproduces the design documents that define the system (overview, architecture, the Rust
boundary, data model, indexing pipeline, query and agents, model providers, evaluation, SDK
surfaces, deployment). Part II covers the milestones and the M1/M2 report with measurements.
Part III is generated from the repository: the work log by day, code and test counts, the dated
decisions made where the design was silent, and the facts about the development machine.

## Next steps

1. Re-index the dataset with the CUDA build to measure the GPU end to end (about two hours).
2. A measured fine pass over the dataset (roughly $60 at $1.60 per hour of video with Claude Sonnet 5).
3. Programmatic comparison against Gemini's agentic video understanding on the same questions.
4. M3: Node binding, `vi-server` (HTTP, SSE, MCP) and the chat application at videoindex.app.
"""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=f"docs/reports/videoindex-team-report-{dt.date.today().isoformat()}.pdf")
    ap.add_argument("--html")
    a = ap.parse_args()
    GEN.mkdir(parents=True, exist_ok=True)
    (GEN / "00-executive-summary.md").write_text(EXEC_SUMMARY)
    (GEN / "90-work-performed.md").write_text(work_log_md())
    docs = ROOT / "docs"
    chapters = [
        {"file": str(GEN / "00-executive-summary.md")},
        {"file": str(docs / "01-overview.md")},
        {"file": str(docs / "02-architecture.md")},
        {"file": str(docs / "03-rust-boundary.md")},
        {"file": str(docs / "04-data-model.md")},
        {"file": str(docs / "05-indexing-pipeline.md")},
        {"file": str(docs / "06-query-and-agents.md")},
        {"file": str(docs / "07-model-providers.md")},
        {"file": str(docs / "08-evaluation.md")},
        {"file": str(docs / "09-sdk-and-apis.md")},
        {"file": str(docs / "11-deployment.md")},
        {"file": str(docs / "10-roadmap.md"), "title": "Milestones and roadmap"},
        {"file": str(docs / "M1-REPORT.md"), "title": "M1 report: what was built and measured"},
        {"file": str(GEN / "90-work-performed.md")},
        {"file": str(docs / "DECISIONS.md"), "title": "Decisions"},
        {"file": str(docs / "MACHINE.md"), "title": "Development machine"},
    ]
    spec = {
        "title": "VideoIndex: architecture, milestones and work performed",
        "subtitle": "Team catch-up report",
        "org": "VideoIndex",
        "authors": ["Generated from the repository by scripts/report/team_report.py"],
        "date": dt.date.today().isoformat(),
        "version": sh("git", "rev-parse", "--short", "HEAD").strip(),
        "abstract": "A single document for new team members: the design as written (architecture, data model, "
                    "pipeline, query layer, providers, SDK surfaces), the milestone plan and where it stands, "
                    "the measured results so far, and a work log generated from the git history.",
        "out": a.out,
        "chapters": chapters,
    }
    if a.html:
        spec["html_out"] = a.html
    print(build_pdf(spec))


if __name__ == "__main__":
    main()
