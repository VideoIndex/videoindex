#!/usr/bin/env python3
"""Score corpus run files. Every system's answer is free text, so a judge
model (Claude) first turns it into structure: which catalog videos it names,
which timestamps it gives for each, and which of the question's key facts it
states. Everything after that is deterministic and identical for every
system:

- set precision / recall / F1 over the videos named against `expected`;
  videos in `acceptable` count neither way; for `negative` questions the
  score is 1 when no video outside `acceptable` is claimed, else 0.
- anchor hit rate: the share of cited (video, timestamp) pairs that fall
  within ±90 s of a spoken or on-screen mention found in the index.
- fact coverage: the share of `facts` the answer states.
- quality (0–1): mention → F1; negative → 0/1; topic → mean of F1 and fact
  coverage; synthesis → 0.3·F1 + 0.7·facts; library → facts.

    export ANTHROPIC_API_KEY=...
    python3 -m eval.judge /data/videoindex/eval/runs/corpus-*.json [--model claude-opus-5] [--force]

Results are written back into each run file (`judge` and `scores` per
result). Answers already judged with the same text and judge model are skipped.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

from .datasets.corpus import CorpusQuestion, load

ANCHOR_WINDOW_S = 90.0

SCHEMA = {
    "type": "object",
    "properties": {
        "claims": {"type": "array", "items": {
            "type": "object",
            "properties": {
                "video_key": {"type": "string", "description": "the catalog key of the video the answer names, or 'unknown' if no catalog video matches"},
                "timestamps_s": {"type": "array", "items": {"type": "number"}, "description": "every timestamp the answer gives for this video, in seconds"},
                "evidence": {"type": "string", "description": "the quote or paraphrase the answer attaches to this video (short)"},
            },
            "required": ["video_key", "timestamps_s", "evidence"], "additionalProperties": False}},
        "says_none": {"type": "boolean", "description": "true when the answer states that no video in the library matches"},
        "facts_covered": {"type": "array", "items": {"type": "boolean"}, "description": "one entry per listed fact: true only when the answer states that fact's substance"},
        "assessment": {"type": "string", "description": "two sentences on the answer's quality: hallucinated videos, vague or missing timestamps, wrong attributions"},
    },
    "required": ["claims", "says_none", "facts_covered", "assessment"], "additionalProperties": False,
}


def judge_prompt(q: CorpusQuestion, catalog: list[dict], text: str) -> str:
    cat = "\n".join(f"- {v['key']} | {v['channel']} | {v['title']}" for v in catalog)
    facts = "\n".join(f"{i + 1}. {f}" for i, f in enumerate(q.facts)) or "(none)"
    return (
        "You are grading an answer produced by a video question-answering system over a library of talks. Do not answer the "
        "question yourself; extract what the answer claims so it can be scored against ground truth.\n\n"
        f"## Library catalog (key | channel | title)\n{cat}\n\n"
        f"## Question\n{q.question}\n\n"
        f"## Key facts a complete answer states\n{facts}\n\n"
        f"## The system's answer\n<answer>\n{text}\n</answer>\n\n"
        "Instructions:\n"
        "- `claims`: one entry per distinct library video the answer names as relevant (by title, speaker or unmistakable description). "
        "Map it to the catalog key; use 'unknown' when it matches no catalog entry (an invented video). Do not include videos the answer "
        "explicitly says are irrelevant or that it only mentions as absent.\n"
        "- `timestamps_s`: convert every timestamp the answer gives for that video ([HH:MM:SS], MM:SS, '1h02m', 'around minute 12') to seconds. "
        "Ranges contribute both ends.\n"
        "- `says_none`: true only if the answer says nothing in the library matches.\n"
        "- `facts_covered`: exactly one boolean per key fact above, in order; true only when the answer conveys that fact's substance "
        "(names, mechanism, conclusion), even in other words. When the fact list is '(none)', return an empty array.\n"
        "- `assessment`: two sentences; mention invented videos, videos named without timestamps, and misattributed content."
    )


def call_judge(client, model: str, prompt: str, attempts: int = 3) -> dict:
    """One judgement. The structured output is normally valid JSON; a reply cut
    at max_tokens or an empty text block is retried, so one bad reply does not
    lose a whole run file's judgements (seen 2026-09-26)."""
    last = None
    for attempt in range(attempts):
        resp = client.messages.create(
            model=model, max_tokens=8000, messages=[{"role": "user", "content": prompt}],
            output_config={"format": {"type": "json_schema", "schema": SCHEMA}},
        )
        if resp.stop_reason == "refusal":
            raise RuntimeError(f"judge refused: {getattr(resp, 'stop_details', None)}")
        text = next((b.text for b in resp.content if b.type == "text"), "")
        try:
            out = json.loads(text)
        except json.JSONDecodeError as e:
            last = f"attempt {attempt + 1}: {e} (stop_reason={resp.stop_reason}, {len(text)} chars: {text[:120]!r})"
            print(f"  judge reply was not JSON; {last}", file=sys.stderr)
            continue
        out["usage"] = {"tokens_in": resp.usage.input_tokens, "tokens_out": resp.usage.output_tokens}
        return out
    raise RuntimeError(f"judge returned no valid JSON after {attempts} attempts; {last}")


# ---------------------------------------------------------------- scoring

def score_result(q: CorpusQuestion, j: dict) -> dict:
    claimed = {c["video_key"] for c in j.get("claims", [])}
    unknown = sum(c["video_key"] == "unknown" for c in j.get("claims", []))
    expected, acceptable = set(q.expected), set(q.acceptable)
    scored_claims = claimed - acceptable  # acceptable claims are neither right nor wrong
    tp = len(claimed & expected)
    precision = tp / len(scored_claims) if scored_claims else (1.0 if not expected else 0.0)
    recall = tp / len(expected) if expected else 1.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    wrong = sorted(scored_claims - expected)
    missed = sorted(expected - claimed)
    # anchors: does each cited timestamp land near a real mention of the video?
    hits = total = 0
    for c in j.get("claims", []):
        anchors = q.anchors.get(c["video_key"], []) + q.anchors_ocr.get(c["video_key"], [])
        if not anchors or not c.get("timestamps_s"):
            continue
        for t in c["timestamps_s"]:
            total += 1
            hits += any(abs(t - a) <= ANCHOR_WINDOW_S for a in anchors)
    anchor_hit = hits / total if total else None
    covered = j.get("facts_covered", [])
    facts = (sum(bool(x) for x in covered[: len(q.facts)]) / len(q.facts)) if q.facts else None
    if q.kind == "negative":
        quality = 1.0 if not wrong else 0.0
    elif q.kind == "mention":
        quality = f1
    elif q.kind == "topic":
        quality = (f1 + facts) / 2 if facts is not None else f1
    elif q.kind == "synthesis":
        quality = 0.3 * f1 + 0.7 * facts if facts is not None else f1
    else:  # library
        quality = facts if facts is not None else 0.0
    return {"precision": round(precision, 4), "recall": round(recall, 4), "f1": round(f1, 4), "claimed": len(claimed), "unknown_videos": unknown,
            "wrong": wrong, "missed": missed, "acceptable_claimed": sorted(claimed & acceptable), "anchor_hit": anchor_hit,
            "anchors_checked": total, "fact_coverage": facts, "quality": round(quality, 4), "cited_with_timestamps": sum(bool(c.get("timestamps_s")) for c in j.get("claims", []))}


def text_hash(text: str, model: str) -> str:
    return hashlib.sha1((model + "\n" + text).encode()).hexdigest()[:16]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("runs", nargs="+")
    ap.add_argument("--model", default="claude-opus-5")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--force", action="store_true", help="re-judge every answer")
    ap.add_argument("--rescore-only", action="store_true", help="recompute the deterministic scores from stored judgements (no API calls)")
    a = ap.parse_args()
    qs = {q.id: q for q in load()}
    client = None
    if not a.rescore_only:
        import anthropic  # noqa: PLC0415

        client = anthropic.Anthropic()
    for path in a.runs:
        run = json.loads(Path(path).read_text())
        cat = run.get("catalog", [])
        todo = []
        for r in run["results"]:
            if r.get("status") not in ("ok", "partial-failure") or not r.get("text") or r["id"] not in qs:
                continue
            h = text_hash(r["text"], a.model)
            if a.rescore_only or (not a.force and r.get("judge", {}).get("hash") == h):
                if r.get("judge"):
                    r["scores"] = score_result(qs[r["id"]], r["judge"])
                continue
            todo.append((r, h))
        print(f"{Path(path).name}: {len(todo)} answers to judge", file=sys.stderr)
        if todo:
            def work(item):
                r, h = item
                j = call_judge(client, a.model, judge_prompt(qs[r["id"]], cat, r["text"]))
                j["hash"], j["model"] = h, a.model
                return r, j

            with ThreadPoolExecutor(max_workers=a.jobs) as ex:
                futs = {ex.submit(work, it): it[0] for it in todo}
                for fut in as_completed(futs):
                    try:
                        r, j = fut.result()
                    except Exception as e:  # noqa: BLE001 - one failed judgement must not lose the file
                        failed = futs[fut]
                        print(f"  {failed['id']} judge failed: {e}", file=sys.stderr)
                        failed.pop("judge", None)
                        failed.pop("scores", None)
                        continue
                    r["judge"] = j
                    r["scores"] = score_result(qs[r["id"]], j)
                    s = r["scores"]
                    print(f"  {r['id']} q={s['quality']:.2f} P={s['precision']:.2f} R={s['recall']:.2f} facts={s['fact_coverage']} anchors={s['anchor_hit']} wrong={s['wrong']} missed={s['missed']}", flush=True)
        Path(path).write_text(json.dumps(run, indent=1, ensure_ascii=False))
        scored = [r["scores"] for r in run["results"] if r.get("scores")]
        if scored:
            print(f"{Path(path).name}: n={len(scored)} quality={sum(s['quality'] for s in scored) / len(scored):.3f} "
                  f"F1={sum(s['f1'] for s in scored) / len(scored):.3f}", file=sys.stderr)


if __name__ == "__main__":
    main()
