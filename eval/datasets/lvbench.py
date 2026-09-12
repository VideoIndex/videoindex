"""LVBench (zai-org, ICCV 2025): `video_info.meta.jsonl` from
https://huggingface.co/datasets/THUDM/LVBench. One line per video with `key`
(YouTube id), `type`, `video_info` and a `qa` list whose `question` embeds the
options as "(A) ...\n(B) ..." lines; `answer` is the letter; `question_type`
is a list of the six capabilities; `time_reference` is "MM:SS-MM:SS"."""
from __future__ import annotations

import json
import re
import urllib.request
from pathlib import Path

from . import Question

META_URL = "https://huggingface.co/datasets/THUDM/LVBench/resolve/main/video_info.meta.jsonl"
OPTION_RE = re.compile(r"^\(([A-H])\)\s*(.*)$")


def ensure_meta(root: Path) -> Path:
    path = root / "video_info.meta.jsonl"
    if not path.is_file():
        root.mkdir(parents=True, exist_ok=True)
        urllib.request.urlretrieve(META_URL, path)
    return path


def split_question(text: str) -> tuple[str, list[str]]:
    stem, options = [], []
    for line in text.split("\n"):
        m = OPTION_RE.match(line.strip())
        if m:
            options.append(m.group(2).strip())
        elif options:
            options[-1] += " " + line.strip()
        else:
            stem.append(line)
    return "\n".join(stem).strip(), options


def load(root: str | Path) -> list[Question]:
    root = Path(root)
    out = []
    for line in ensure_meta(root).read_text().splitlines():
        if not line.strip():
            continue
        v = json.loads(line)
        for qa in v["qa"]:
            stem, options = split_question(qa["question"])
            out.append(
                Question(
                    id=f"lvbench-{v['key']}-{qa.get('uid', len(out))}",
                    benchmark="lvbench",
                    video_key=v["key"],
                    question=stem,
                    options=options,
                    answer=qa["answer"].strip().upper(),
                    task_types=list(qa.get("question_type", [])),
                    time_reference=qa.get("time_reference"),
                    video_type=v.get("type"),
                    extra={"duration_minutes": v.get("video_info", {}).get("duration_minutes")},
                )
            )
    return out


def video_keys(root: str | Path) -> list[str]:
    return [json.loads(l)["key"] for l in ensure_meta(Path(root)).read_text().splitlines() if l.strip()]
