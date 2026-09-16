"""Loader and parser tests (no network, no models)."""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from eval.datasets.lvbench import split_question  # noqa: E402
from eval.metrics import parse_ref, score, wilson  # noqa: E402
from eval.runners.answer import parse_letter  # noqa: E402
from eval.datasets import stratified_sample, sample_size  # noqa: E402
from eval.datasets import Question  # noqa: E402


def test_lvbench_question_split():
    stem, opts = split_question("What year appears?\n(A) 1636\n(B) 1366\n(C) 1363\n(D) 1633")
    assert stem == "What year appears?"
    assert opts == ["1636", "1366", "1363", "1633"]


def test_letter_parsing():
    L = ["A", "B", "C", "D"]
    assert parse_letter("The caption shows 1633.\n\nAnswer: D", L) == "D"
    assert parse_letter("**Answer: (B)**", L) == "B"
    assert parse_letter("I think it is (C).", L) == "C"
    assert parse_letter("Reasoning only, nothing decided.", L) is None
    assert parse_letter("Answer: Z", L) is None


def test_stratified_sample_keeps_every_type():
    qs = [Question(id=str(i), benchmark="b", video_key="v", question="q", options=["a", "b"], answer="A", task_types=[t])
          for i, t in enumerate(["x"] * 50 + ["y"] * 10 + ["z"] * 2)]
    s = stratified_sample(qs, 12, 1)
    assert len(s) == 12
    assert {q.task_types[0] for q in s} == {"x", "y", "z"}


def test_metrics():
    lo, hi = wilson(96, 100)
    assert 0.89 < lo < 0.96 < hi < 0.99
    assert parse_ref("00:15-00:19") == (15, 19)
    assert parse_ref("1:02:03-1:02:10") == (3723, 3730)
    run = {"results": [
        {"status": "ok", "correct": True, "parsed": True, "task_types": ["t"], "usage": {"cost_usd": 0.1, "tokens_in": 100, "tokens_out": 10, "tool_calls": 2}, "ms": 1000, "tools": ["search"], "citations": [{"t0": 10, "t1": 20}], "time_reference": "00:15-00:19"},
        {"status": "ok", "correct": False, "parsed": False, "task_types": ["t"], "usage": {"cost_usd": 0.3, "tokens_in": 300, "tokens_out": 30, "tool_calls": 4}, "ms": 3000, "tools": ["search", "view"], "citations": [], "time_reference": None},
        {"status": "ask-failed"},
    ]}
    s = score(run)
    assert s["n"] == 2 and s["accuracy"] == 0.5 and s["failed"] == 1 and s["unparsed"] == 1
    assert s["citation_in_range"] == 1.0 and s["no_decode_fraction"] == 0.5
    assert abs(s["cost_mean"] - 0.2) < 1e-9


def test_sample_size_and_determinism():
    assert sample_size(1549, None, 0.25) == 387
    assert sample_size(100, 30, 0.25) == 30
    assert sample_size(100, None, None) is None
    qs = [Question(id=str(i), benchmark="b", video_key="v", question="q", options=["a", "b"], answer="A", task_types=[t])
          for i, t in enumerate(["x"] * 50 + ["y"] * 10)]
    a = [q.id for q in stratified_sample(qs, 12, 1)]
    b = [q.id for q in stratified_sample(list(reversed(qs)), 12, 1)]
    assert a == b, "the sample depends on the seed, not on the pool order"


def test_option_text_fallback():
    letters = ["A", "B", "C"]
    opts = ["#4 in white dribbles the ball down court then passes", "#0 in white inbounds the ball to #4", "The referee stops play"]
    assert parse_letter("The play was: #4 in white dribbles the ball down court then passes to #10.", letters, opts) == "A"
    # Two options restated -> ambiguous -> unparsed
    assert parse_letter("#4 in white dribbles the ball down court then passes; then #0 in white inbounds the ball to #4.", letters, opts) is None
    assert parse_letter("Something else entirely.", letters, opts) is None
    assert parse_letter("Answer: C", letters, opts) == "C"


# ---------------------------------------------------------------- corpus set

def test_corpus_set_is_consistent():
    from eval.datasets.corpus import KINDS, load as load_corpus  # noqa: PLC0415

    qs = load_corpus()
    assert len(qs) >= 20
    assert len({q.id for q in qs}) == len(qs)
    for q in qs:
        assert q.kind in KINDS
        assert not (set(q.expected) & set(q.acceptable)), q.id
        if q.kind == "negative":
            assert not q.expected
        if q.kind in ("synthesis", "library"):
            assert q.facts, q.id
        for k in q.anchors:
            assert k in q.expected or k in q.acceptable, (q.id, k)
    assert "Videos: N" in qs[0].prompt()


def test_corpus_scoring():
    from eval.datasets.corpus import CorpusQuestion  # noqa: PLC0415
    from eval.judge import score_result  # noqa: PLC0415

    q = CorpusQuestion(id="t", kind="mention", question="?", expected=["a", "b", "c"], acceptable=["d"],
                       anchors={"a": [100.0], "b": [50.0]}, anchors_ocr={"c": [900.0]})
    j = {"claims": [{"video_key": "a", "timestamps_s": [130]}, {"video_key": "b", "timestamps_s": [400]},
                    {"video_key": "d", "timestamps_s": []}, {"video_key": "unknown", "timestamps_s": [5]}], "facts_covered": []}
    s = score_result(q, j)
    assert s["precision"] == round(2 / 3, 4)  # a, b right; unknown wrong; d ignored
    assert s["recall"] == round(2 / 3, 4)
    assert s["wrong"] == ["unknown"] and s["missed"] == ["c"] and s["acceptable_claimed"] == ["d"]
    assert s["anchor_hit"] == 0.5 and s["anchors_checked"] == 2
    assert s["quality"] == s["f1"]
    neg = CorpusQuestion(id="n", kind="negative", question="?", acceptable=["x"])
    assert score_result(neg, {"claims": [{"video_key": "x", "timestamps_s": []}], "facts_covered": []})["quality"] == 1.0
    assert score_result(neg, {"claims": [{"video_key": "y", "timestamps_s": []}], "facts_covered": []})["quality"] == 0.0
    syn = CorpusQuestion(id="s", kind="synthesis", question="?", expected=["a", "b"], facts=["f1", "f2", "f3", "f4"])
    s = score_result(syn, {"claims": [{"video_key": "a", "timestamps_s": []}, {"video_key": "b", "timestamps_s": []}], "facts_covered": [True, True, False, False]})
    assert s["f1"] == 1.0 and s["fact_coverage"] == 0.5 and abs(s["quality"] - (0.3 + 0.35)) < 1e-6
    lib = CorpusQuestion(id="l", kind="library", question="?", facts=["f1", "f2"])
    assert score_result(lib, {"claims": [], "facts_covered": [True, False]})["quality"] == 0.5
