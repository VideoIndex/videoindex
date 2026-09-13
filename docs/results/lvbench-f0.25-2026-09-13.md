# LVBench: 25% stratified sample (seed 1) over the videos available

Generated 2026-09-13 by `eval/report.py` from 4 run file(s). Accuracy is exact match on the option letter; intervals are 95% Wilson. Costs are provider list prices per question; indexing cost is not included for the index-based configurations.

![](assets/lvbench-f0.25-2026-09-13-pareto.svg)

| Configuration | n | accuracy | 95% CI | unparsed | cost / q | tokens / q | tool calls / q | latency p50 | latency p95 | no-decode | citation in range |
|---|---|---|---|---|---|---|---|---|---|---|---|
| agent | 223 | **64.1%** | 57.6–70.1 | 3 | $0.102 | 29,186 | 4.17 | 14.9 s | 42.4 s | 36% | 71% (n=221) |
| retrieval-only | 223 | **50.7%** | 44.2–57.2 | 1 | $0.026 | 6,694 | 1.00 | 8.5 s | 14.7 s | 100% | 54% (n=221) |
| uniform-32 (claude-sonnet-5) | 223 | **52.9%** | 46.4–59.4 | 6 | $0.036 | 11,358 | 0.00 | 11.0 s | 18.4 s | 100% | — |
| gemini-3.8-flash agentic video | 211 | **79.1%** | 73.2–84.1 | 11 | $0.065 | 44,765 | 3.70 | 14.8 s | 86.2 s | 100% | — |

## Where this stands against Gemini agentic video

The `gemini-…` row is Google's Gemini 3.8 Flash with `processing: "agentic"` asked the same questions over the same video files through the Interactions API (run once and cached; it is not re-run with every matrix). Google's own announcement ("Introducing agentic video in Gemini", 2026-09-01) reports agentic mode against static whole-video processing on LongVideoBench as up to 88% fewer tokens, up to 66% lower cost and up to 7% higher accuracy, without absolute scores; the numbers here are absolute, on LVBench, and directly comparable across rows because every row answered the same questions under the same scoring. VideoIndex's per-question cost excludes indexing (done once per video); Gemini's per-question cost is the whole cost. Both systems are scored with the same letter parser.


## Accuracy by task type

| Task type | agent | retrieval-only | uniform-32 (claude-sonnet-5) | gemini-3.8-flash agentic video |
|---|---|---|---|---|
| entity recognition | 62% (n=92) | 46% (n=92) | 52% (n=92) | 75% (n=85) |
| event understanding | 64% (n=86) | 52% (n=86) | 58% (n=86) | 78% (n=82) |
| key information retrieval | 73% (n=45) | 58% (n=45) | 47% (n=45) | 86% (n=43) |
| reasoning | 63% (n=27) | 48% (n=27) | 59% (n=27) | 73% (n=26) |
| summarization | 29% (n=7) | 57% (n=7) | 57% (n=7) | 100% (n=7) |
| temporal grounding | 78% (n=23) | 61% (n=23) | 48% (n=23) | 95% (n=22) |

## Tool-call profiles

**agent**: `search > search > view` ×32; `search > search` ×19; `view` ×11; `search` ×9; `search > search > view > search > search > view` ×5; `search > search > get_transcript` ×5; `search > view` ×5; `(none)` ×4

**retrieval-only**: `search` ×223

**uniform-32 (claude-sonnet-5)**: `(none)` ×223

**gemini-3.8-flash agentic video**: `processing_call×2` ×58; `processing_call×1` ×47; `processing_call×3` ×33; `processing_call×4` ×16; `processing_call×5` ×15; `processing_call×6` ×12; `processing_call×7` ×8; `processing_call×8` ×7

## Runs

- **agent**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-agent.json` — {"benchmark": "lvbench", "policy": "agent", "budget_usd": 0.5, "budget_tokens": 120000, "max_tool_calls": 6, "sample": 387, "fraction": 0.25, "seed": 1, "skipped_not_indexed": 164, "vi_version": "vi 0.1.0", "started": "2026-09-13T19:25:28Z"}
- **retrieval-only**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-retrieval-only.json` — {"benchmark": "lvbench", "policy": "retrieval-only", "budget_usd": 0.5, "budget_tokens": 120000, "max_tool_calls": 6, "sample": 387, "fraction": 0.25, "seed": 1, "skipped_not_indexed": 164, "vi_version": "vi 0.1.0", "started": "2026-09-13T19:27:37Z"}
- **uniform-32 (claude-sonnet-5)**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-uniform-32.json` — {"benchmark": "lvbench", "policy": "uniform-32", "model": "claude-sonnet-5", "frames": 32, "transcript": true, "sample": 387, "fraction": 0.25, "seed": 1, "started": "2026-09-13T19:28:54Z"}
- **gemini-3.8-flash agentic video**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-gemini-3.8-flash-agentic.json` — {"benchmark": "lvbench", "policy": "gemini-agentic", "model": "gemini-3.8-flash", "mode": "agentic", "thinking": null, "sample": 387, "fraction": 0.25, "seed": 1, "started": "2026-09-13T19:30:04Z"}

Questions per run: up to 223. Benchmark videos are public YouTube content and may be in model training data; read per-configuration deltas rather than absolute scores.
