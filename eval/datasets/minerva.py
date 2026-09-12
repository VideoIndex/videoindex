"""MINERVA (Google DeepMind, 2025; released in github.com/google-deepmind/neptune).
Expects one or more JSON files under `<root>/*.json` with records carrying
`key`, `video_id` (YouTube URL or id), `question`, `answer`, `answer_choice_{i}`
and `answer_id` (index of the correct choice), `reasoning`, `question_type`.
The loader is schema-tolerant: choices may also arrive as a list."""
from __future__ import annotations

import json
import re
from pathlib import Path

from . import Question

YT_RE = re.compile(r"(?:v=|youtu\.be/|^)([\w-]{11})(?:[&?]|$)")


def youtube_key(s: str) -> str:
    m = YT_RE.search(s)
    return m.group(1) if m else s


def _choices(rec: dict) -> tuple[list[str], str]:
    if isinstance(rec.get("answer_choices"), list):
        choices = [str(c) for c in rec["answer_choices"]]
    else:
        choices = []
        for i in range(0, 12):
            k = f"answer_choice_{i}"
            if k in rec:
                choices.append(str(rec[k]))
        if not choices and "answer" in rec:
            choices = [str(rec["answer"])]
    if "answer_id" in rec:
        idx = int(rec["answer_id"])
    elif rec.get("answer") in choices:
        idx = choices.index(rec["answer"])
    else:
        idx = 0
    return choices, chr(ord("A") + idx)


def load(root: str | Path) -> list[Question]:
    root = Path(root)
    files = sorted(p for p in root.glob("*.json") if "ego" not in p.name.lower())
    if not files:
        raise SystemExit(f"no MINERVA json under {root}; download it from github.com/google-deepmind/neptune")
    out = []
    for f in files:
        data = json.loads(f.read_text())
        records = data if isinstance(data, list) else data.get("data") or data.get("questions") or list(data.values())
        for i, rec in enumerate(records):
            if not isinstance(rec, dict) or "question" not in rec:
                continue
            choices, letter = _choices(rec)
            out.append(
                Question(
                    id=f"minerva-{rec.get('key', i)}",
                    benchmark="minerva",
                    video_key=youtube_key(str(rec.get("video_id", rec.get("video", "")))),
                    question=str(rec["question"]),
                    options=choices,
                    answer=letter,
                    task_types=[str(t) for t in (rec.get("question_type") or [])] if isinstance(rec.get("question_type"), list) else ([str(rec["question_type"])] if rec.get("question_type") else []),
                    extra={"reasoning": rec.get("reasoning")},
                )
            )
    return out
