# LVBench: 25% stratified sample (seed 1)

Generated 2026-09-13 by `eval/report.py` from 4 run file(s). Accuracy is exact match on the option letter; intervals are 95% Wilson. Costs are provider list prices per question; indexing cost is not included for the index-based configurations.

![](assets/lvbench-f0.25-2026-09-13-pareto.svg)

| Configuration | n | accuracy | 95% CI | unparsed | cost / q | tokens / q | tool calls / q | latency p50 | latency p95 | no-decode | citation in range |
|---|---|---|---|---|---|---|---|---|---|---|---|
| agent | 340 | **65.0%** | 59.8–69.9 | 7 | $0.100 | 28,673 | 4.11 | 14.3 s | 44.2 s | 39% | 73% (n=335) |
| retrieval-only | 340 | **50.0%** | 44.7–55.3 | 3 | $0.026 | 6,689 | 1.01 | 8.4 s | 15.2 s | 100% | 57% (n=336) |
| uniform-32 (claude-sonnet-5) | 340 | **52.1%** | 46.8–57.3 | 14 | $0.037 | 11,718 | 0.00 | 11.8 s | 22.3 s | 100% | — |
| gemini-3.8-flash agentic video | 211 | **79.1%** | 73.2–84.1 | 11 | $0.065 | 44,765 | 3.70 | 14.8 s | 86.2 s | 100% | — |

## Where this stands against Gemini agentic video

The `gemini-…` row is Google's Gemini 3.8 Flash with `processing: "agentic"` asked the same questions over the same video files through the Interactions API (run once and cached; it is not re-run with every matrix). Google's own announcement ("Introducing agentic video in Gemini", 2026-09-01) reports agentic mode against static whole-video processing on LongVideoBench as up to 88% fewer tokens, up to 66% lower cost and up to 7% higher accuracy, without absolute scores; the numbers here are absolute, on LVBench, and directly comparable across rows because every row answered the same questions under the same scoring. VideoIndex's per-question cost excludes indexing (done once per video); Gemini's per-question cost is the whole cost. Both systems are scored with the same letter parser.


## Accuracy by task type

| Task type | agent | retrieval-only | uniform-32 (claude-sonnet-5) | gemini-3.8-flash agentic video |
|---|---|---|---|---|
| entity recognition | 64% (n=139) | 44% (n=139) | 50% (n=139) | 75% (n=85) |
| event understanding | 63% (n=135) | 50% (n=135) | 57% (n=135) | 78% (n=82) |
| key information retrieval | 74% (n=74) | 57% (n=74) | 45% (n=74) | 86% (n=43) |
| reasoning | 61% (n=38) | 45% (n=38) | 55% (n=38) | 73% (n=26) |
| summarization | 29% (n=14) | 50% (n=14) | 43% (n=14) | 100% (n=7) |
| temporal grounding | 78% (n=36) | 56% (n=36) | 53% (n=36) | 95% (n=22) |

## Tool-call profiles

**agent**: `search > search > view` ×44; `search > search` ×30; `search` ×21; `view` ×16; `search > view` ×10; `search > search > get_transcript` ×8; `search > search > search > search > search > search` ×7; `search > search > view > view > view > view` ×6

**retrieval-only**: `search` ×340

**uniform-32 (claude-sonnet-5)**: `(none)` ×340

**gemini-3.8-flash agentic video**: `processing_call×2` ×58; `processing_call×1` ×47; `processing_call×3` ×33; `processing_call×4` ×16; `processing_call×5` ×15; `processing_call×6` ×12; `processing_call×7` ×8; `processing_call×8` ×7

## Runs

- **agent**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-agent.json` — {"benchmark": "lvbench", "policy": "agent", "budget_usd": 0.5, "budget_tokens": 120000, "max_tool_calls": 6, "sample": 387, "fraction": 0.25, "seed": 1, "skipped_not_indexed": 47, "vi_version": "vi 0.1.0", "started": "2026-09-13T23:30:59Z"}
- **retrieval-only**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-retrieval-only.json` — {"benchmark": "lvbench", "policy": "retrieval-only", "budget_usd": 0.5, "budget_tokens": 120000, "max_tool_calls": 6, "sample": 387, "fraction": 0.25, "seed": 1, "skipped_not_indexed": 47, "vi_version": "vi 0.1.0", "started": "2026-09-13T23:40:26Z"}
- **uniform-32 (claude-sonnet-5)**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-uniform-32.json` — {"benchmark": "lvbench", "policy": "uniform-32", "model": "claude-sonnet-5", "frames": 32, "transcript": true, "sample": 387, "fraction": 0.25, "seed": 1, "started": "2026-09-13T23:44:46Z"}
- **gemini-3.8-flash agentic video**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-gemini-3.8-flash-agentic.json` — {"benchmark": "lvbench", "policy": "gemini-agentic", "model": "gemini-3.8-flash", "mode": "agentic", "thinking": null, "sample": 387, "fraction": 0.25, "seed": 1, "started": "2026-09-13T19:30:04Z"}

Questions per run: up to 340. Benchmark videos are public YouTube content and may be in model training data; read per-configuration deltas rather than absolute scores.
