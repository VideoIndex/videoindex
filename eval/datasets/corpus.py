"""Corpus QA: open-ended questions that span a whole index rather than one
video ("find every talk that mentions X", "show the moments where speakers
discuss Y", "compare how A and B describe Z"). This is the kind of question
the product exists for and none of the public long-video benchmarks ask it,
so the set is our own: 25 questions over the 30-video `dataset` index, kept
in `eval/data/corpus/questions.json`.

Each question carries hand-written fields and derived fields:

- `kind`: `mention` (which videos say X; scored by set precision/recall),
  `topic` (which videos discuss X and what they say; set score plus key
  facts), `synthesis` (a cross-video answer judged mainly on facts) or
  `negative` (nothing in the library matches; scored on not inventing hits).
- `patterns`: case-insensitive regexes run over the transcript and OCR rows
  of the index by `python -m eval.datasets.corpus build`. For `mention`
  questions the videos with spoken hits become `expected`; videos with only
  on-screen hits (or listed in `ambiguous`) become `acceptable`, which neither
  earns nor costs credit; `exclude` names regex false positives (the word
  "notion", a mouse cursor) that stay wrong to claim. For the other kinds
  `expected` is curated and the remaining pattern hits are `acceptable`.
- `anchors`: derived spoken (and on-screen) hit times per video, in seconds,
  used to check that a cited timestamp lands near a real mention (±90 s).
- `facts`: what a complete answer must say; the judge marks each one.

Videos are identified by YouTube id (the `video_key` of the other loaders);
`catalog()` maps them to index ids and titles for the runners.
"""
from __future__ import annotations

import argparse
import json
import re
import sqlite3
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

DATA = Path(__file__).resolve().parents[1] / "data" / "corpus" / "questions.json"
KINDS = ("mention", "topic", "synthesis", "negative", "library")


@dataclass
class CorpusQuestion:
    id: str
    kind: str
    question: str
    expected: list[str] = field(default_factory=list)  # YouTube ids that a complete answer names
    acceptable: list[str] = field(default_factory=list)  # neither rewarded nor penalised
    facts: list[str] = field(default_factory=list)
    anchors: dict[str, list[float]] = field(default_factory=dict)  # video_key -> spoken hit times (s)
    anchors_ocr: dict[str, list[float]] = field(default_factory=dict)
    patterns: list[str] = field(default_factory=list)
    notes: str = ""
    extra: dict = field(default_factory=dict)

    @property
    def task_types(self) -> list[str]:  # keeps `eval.metrics`-style grouping working
        return [self.kind]

    def prompt(self) -> str:
        """The question as both systems receive it: the same wording, the same
        answer format, so the judge sees comparable output."""
        return (
            f"{self.question}\n\n"
            "Answer for the whole video library, not a single video. Format: one bullet per relevant video with the "
            "video's exact title, then the timestamp(s) [HH:MM:SS] of the relevant moment(s), then a one-sentence quote "
            "or paraphrase of what is said or shown there. List only videos where you actually found the evidence; if "
            "nothing in the library matches, say so plainly. End with a line 'Videos: N' giving how many videos you listed."
        )


def load(path: str | Path = DATA) -> list[CorpusQuestion]:
    rows = json.loads(Path(path).read_text())["questions"]
    out = []
    for r in rows:
        if r["kind"] not in KINDS:
            raise SystemExit(f"{r['id']}: unknown kind {r['kind']}")
        out.append(CorpusQuestion(
            id=r["id"], kind=r["kind"], question=r["question"], expected=r.get("expected", []),
            acceptable=r.get("acceptable", []), facts=r.get("facts", []), anchors=r.get("anchors", {}),
            anchors_ocr=r.get("anchors_ocr", {}), patterns=r.get("patterns", []), notes=r.get("notes", ""),
            extra={k: v for k, v in r.items() if k not in ("id", "kind", "question", "expected", "acceptable", "facts", "anchors", "anchors_ocr", "patterns", "notes")},
        ))
    return out


# ---------------------------------------------------------------- the index

def _connect(index: str | Path) -> sqlite3.Connection:
    return sqlite3.connect(f"file:{Path(index) / 'meta.sqlite'}?mode=ro", uri=True)


def youtube_key(uri: str) -> str:
    m = re.search(r"(?:v=|youtu\.be/|/shorts/)([A-Za-z0-9_-]{11})", uri or "")
    return m.group(1) if m else uri


def catalog(index: str | Path) -> list[dict]:
    """Every video of the index: {key, id, title, channel, duration_s, path}
    in a stable order (by index id)."""
    c = _connect(index)
    rows = c.execute("select id, source_uri, content_hash, title, channel, duration_num, duration_den from videos order by id").fetchall()
    return [{"key": youtube_key(uri), "id": vid, "title": title or "", "channel": channel or "", "content_hash": h,
             "duration_s": num / den if den else 0.0} for vid, uri, h, title, channel, num, den in rows]


def hits(index: str | Path, patterns: list[str]) -> tuple[dict[str, list[float]], dict[str, list[float]]]:
    """Spoken and on-screen match times per video key for any of the patterns."""
    if not patterns:
        return {}, {}
    rx = re.compile("|".join(f"(?:{p})" for p in patterns), re.I)
    c = _connect(index)
    key_of = {vid: youtube_key(uri) for vid, uri in c.execute("select id, source_uri from videos")}
    tx: dict[str, list[float]] = defaultdict(list)
    for vid, t0, text in c.execute("select t.video_id, s.t0_secs, s.text from transcript_spans s join tracks t on s.track_id = t.id"):
        if rx.search(text):
            tx[key_of[vid]].append(round(t0, 1))
    ocr: dict[str, list[float]] = defaultdict(list)
    for vid, t, text in c.execute(
        "select t.video_id, o.t_secs, o.text from ocr_spans o join frame_samples f on o.frame_sample_id = f.id join tracks t on f.track_id = t.id"
    ):
        if rx.search(text):
            ocr[key_of[vid]].append(round(t, 1))
    return {k: sorted(set(v)) for k, v in tx.items()}, {k: sorted(set(v)) for k, v in ocr.items()}


def build(path: Path, index: Path) -> None:
    """Fill the derived fields (expected for `mention`, acceptable, anchors)
    from the index, keeping every hand-written field; idempotent."""
    doc = json.loads(path.read_text())
    keys = {v["key"] for v in catalog(index)}
    for q in doc["questions"]:
        tx, ocr = hits(index, q.get("patterns", []))
        exclude = set(q.get("exclude", []))
        ambiguous = set(q.get("ambiguous", []))
        for k in exclude | ambiguous | set(q.get("expected_curated", [])):
            if k not in keys:
                raise SystemExit(f"{q['id']}: unknown video key {k}")
        if q["kind"] == "mention":
            expected = sorted((set(tx) - exclude - ambiguous) | set(q.get("expected_curated", [])))
        elif q["kind"] == "negative":
            expected = []
        elif q["kind"] == "library":
            expected = sorted(set(q.get("expected_curated", [])))
        else:
            expected = sorted(set(q["expected_curated"]))
        hit_videos = set(tx) | set(ocr)
        acceptable = sorted(((hit_videos - set(expected) - exclude) | ambiguous | set(q.get("acceptable_curated", []))) - set(expected))
        if q["kind"] == "library":  # any video may be cited in a whole-library answer
            acceptable = sorted(keys - set(expected))
        q["expected"] = expected
        q["acceptable"] = acceptable
        q["anchors"] = {k: v for k, v in sorted(tx.items()) if k not in exclude}
        q["anchors_ocr"] = {k: v for k, v in sorted(ocr.items()) if k not in exclude}
    doc["built_from"] = {"index": str(index), "videos": len(keys)}
    path.write_text(json.dumps(doc, indent=1, ensure_ascii=False) + "\n")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build", help="derive expected/acceptable/anchors from an index")
    b.add_argument("--index", default="/data/videoindex/indexes/dataset.vidx")
    b.add_argument("--file", default=str(DATA))
    s = sub.add_parser("show", help="print the set with its ground truth")
    s.add_argument("--file", default=str(DATA))
    s.add_argument("--index", default="/data/videoindex/indexes/dataset.vidx")
    a = ap.parse_args()
    if a.cmd == "build":
        build(Path(a.file), Path(a.index))
        print(a.file)
    else:
        titles = {v["key"]: v["title"] for v in catalog(a.index)}
        for q in load(a.file):
            print(f"\n{q.id} [{q.kind}] {q.question}")
            for k in q.expected:
                print(f"   + {k}  {titles.get(k, '?')[:80]}  ({len(q.anchors.get(k, []))} spoken, {len(q.anchors_ocr.get(k, []))} on-screen)")
            for k in q.acceptable:
                print(f"   ~ {k}  {titles.get(k, '?')[:80]}")
            for f in q.facts:
                print(f"   fact: {f}")


if __name__ == "__main__":
    main()
