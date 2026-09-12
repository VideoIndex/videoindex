"""Scores for a run file: accuracy overall and per task type with Wilson
intervals, tokens, cost, latency (mean, p50, p95), tool-call histogram, the
fraction answered without decoding, and citation-in-range when the benchmark
gives time references."""
from __future__ import annotations

import math
import re
from collections import Counter


def wilson(k: int, n: int, z: float = 1.96) -> tuple[float, float]:
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    denom = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / denom
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / denom
    return (max(0.0, centre - half), min(1.0, centre + half))


def pct(xs: list[float], q: float) -> float:
    if not xs:
        return 0.0
    s = sorted(xs)
    return s[min(len(s) - 1, int(q * (len(s) - 1)))]


def parse_ref(ref: str | None) -> tuple[float, float] | None:
    if not ref:
        return None
    m = re.match(r"^\s*(\d+):(\d+)(?::(\d+))?\s*-\s*(\d+):(\d+)(?::(\d+))?\s*$", ref)
    if not m:
        return None

    def secs(a, b, c):
        return (int(a) * 3600 + int(b) * 60 + int(c)) if c else (int(a) * 60 + int(b))

    return secs(m.group(1), m.group(2), m.group(3)), secs(m.group(4), m.group(5), m.group(6))


def score(run: dict) -> dict:
    res = [r for r in run["results"] if r.get("status") == "ok"]
    n = len(res)
    correct = sum(r["correct"] for r in res)
    lo, hi = wilson(correct, n)
    by_type: dict[str, list[dict]] = {}
    for r in res:
        for t in r.get("task_types") or ["unknown"]:
            by_type.setdefault(t, []).append(r)
    per_type = {}
    for t, rs in sorted(by_type.items()):
        k = sum(x["correct"] for x in rs)
        l2, h2 = wilson(k, len(rs))
        per_type[t] = {"n": len(rs), "accuracy": k / len(rs), "ci": (l2, h2)}
    costs = [r["usage"].get("cost_usd", 0) for r in res]
    toks = [r["usage"].get("tokens_in", 0) + r["usage"].get("tokens_out", 0) for r in res]
    lat = [r["ms"] / 1000 for r in res]
    tool_hist = Counter()
    for r in res:
        tool_hist[" > ".join(r.get("tools") or []) or "(none)"] += 1
    in_range = total_ref = 0
    for r in res:
        ref = parse_ref(r.get("time_reference"))
        if ref and r.get("citations"):
            total_ref += 1
            if any(c["t0"] - 60 <= ref[1] and c["t1"] + 60 >= ref[0] for c in r["citations"]):
                in_range += 1
    return {
        "n": n, "accuracy": correct / n if n else 0.0, "ci": (lo, hi), "unparsed": sum(not r.get("parsed", True) for r in res),
        "failed": sum(r.get("status") != "ok" for r in run["results"]),
        "per_type": per_type,
        "cost_mean": sum(costs) / n if n else 0.0, "cost_total": sum(costs), "cost_p95": pct(costs, 0.95),
        "tokens_mean": sum(toks) / n if n else 0.0, "tokens_p95": pct(toks, 0.95),
        "latency_mean": sum(lat) / n if n else 0.0, "latency_p50": pct(lat, 0.5), "latency_p95": pct(lat, 0.95),
        "tool_calls_mean": sum(r["usage"].get("tool_calls", 0) for r in res) / n if n else 0.0,
        "no_decode_fraction": sum("view" not in (r.get("tools") or []) and "describe" not in (r.get("tools") or []) for r in res) / n if n else 0.0,
        "tool_histogram": tool_hist.most_common(8),
        "citation_in_range": (in_range / total_ref) if total_ref else None,
        "citation_in_range_n": total_ref,
    }
