"""1H-VideoQA (Google DeepMind): 101 five-way questions over 21 YouTube videos
(40 to 90 minutes), distributed through the Kaggle benchmark
https://www.kaggle.com/benchmarks/deepmind/video-qa. The Kaggle export is a
CSV with `Video URL`, `Question`, `Capability` (Recall / Reasoning) and a
`Prompt` that embeds the options as `Options: (A) ... (B) ...` and asks for
`Final Answer: (X)`. **It carries no answers**: scoring happens on the Kaggle
leaderboard, so runs over this set produce predictions rather than an accuracy;
the score comes from a kaggle-benchmarks task that asks the hosted VideoIndex
API the same questions (`eval.report --submission` exports the local predictions).

Place the CSV (or the zip's extracted folder) under `<root>/`; older exports
with explicit `option_a..e` / `answer` columns are also accepted.
"""
from __future__ import annotations

import csv
import json
import re
from pathlib import Path

from . import Question
from .minerva import youtube_key

OPT_RE = re.compile(r"\(([A-H])\)\s*(.*?)(?=\s*\([A-H]\)\s|$)", re.S)


def _options_from_prompt(prompt: str) -> list[str]:
    m = re.search(r"Options:\s*(.*)$", prompt, re.S)
    if not m:
        return []
    return [t.strip() for _, t in OPT_RE.findall(m.group(1))]


def load(root: str | Path) -> list[Question]:
    root = Path(root)
    rows: list[dict] = []
    for p in sorted(root.rglob("*.csv")):
        with open(p, newline="") as f:
            rows.extend(csv.DictReader(f))
    for p in sorted(root.rglob("*.json")):
        try:
            d = json.loads(p.read_text())
        except json.JSONDecodeError:
            continue
        rows.extend(d if isinstance(d, list) else d.get("data", []))
    if not rows:
        raise SystemExit(f"no 1H-VideoQA data under {root}; download the CSV from the Kaggle benchmark's Data tab")
    out = []
    for i, r in enumerate(rows):
        url = r.get("Video URL") or r.get("video_id") or r.get("video_url") or ""
        question = r.get("Question") or r.get("question") or ""
        opts = r.get("options")
        if isinstance(opts, str):
            opts = [o.strip() for o in opts.split("|")]
        if not opts:
            opts = [r[k] for k in ("option_a", "option_b", "option_c", "option_d", "option_e") if r.get(k)]
        if not opts and r.get("Prompt"):
            opts = _options_from_prompt(r["Prompt"])
        ans = str(r.get("answer", "")).strip().upper()[:1] or None
        cap = r.get("Capability") or r.get("capability")
        out.append(
            Question(
                id=f"onehour-{r.get('', i) or i}",
                benchmark="onehour",
                video_key=youtube_key(str(url)),
                question=question,
                options=list(opts),
                answer=ans or "?",
                task_types=[cap] if cap else [],
                extra={"kaggle_row": r.get("", i), "answer_known": bool(ans)},
            )
        )
    return out
