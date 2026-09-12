"""1H-VideoQA (Google DeepMind): 101 five-way questions over 21 YouTube videos,
distributed through Kaggle (https://www.kaggle.com/benchmarks/deepmind/video-qa),
which needs an account. Place the exported CSV/JSON under `<root>/` with columns
or keys `video_id`/`video_url`, `question`, `option_a..e` (or `options`), `answer`."""
from __future__ import annotations

import csv
import json
from pathlib import Path

from . import Question
from .minerva import youtube_key


def load(root: str | Path) -> list[Question]:
    root = Path(root)
    rows: list[dict] = []
    for p in sorted(root.glob("*.json")):
        d = json.loads(p.read_text())
        rows.extend(d if isinstance(d, list) else d.get("data", []))
    for p in sorted(root.glob("*.csv")):
        with open(p, newline="") as f:
            rows.extend(csv.DictReader(f))
    if not rows:
        raise SystemExit(f"no 1H-VideoQA data under {root}; export it from the Kaggle benchmark")
    out = []
    for i, r in enumerate(rows):
        opts = r.get("options")
        if isinstance(opts, str):
            opts = [o.strip() for o in opts.split("|")]
        if not opts:
            opts = [r[k] for k in ("option_a", "option_b", "option_c", "option_d", "option_e") if k in r and r[k]]
        ans = str(r.get("answer", "A")).strip().upper()[:1]
        out.append(
            Question(
                id=f"onehour-{i}",
                benchmark="onehour",
                video_key=youtube_key(str(r.get("video_id") or r.get("video_url") or "")),
                question=str(r["question"]),
                options=list(opts),
                answer=ans,
            )
        )
    return out
