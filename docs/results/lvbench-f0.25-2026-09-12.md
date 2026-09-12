# lvbench: 25% stratified sample, seed 1

**Interim:** 140 of the 387 sampled questions had indexed videos at this pass (47 of 103 LVBench videos downloaded); the page is regenerated when acquisition completes.

Generated 2026-09-12 by `eval/report.py` from 3 run file(s). Accuracy is exact match on the option letter; intervals are 95% Wilson. Costs are provider list prices per question; indexing cost is not included for the index-based configurations.

![](assets/lvbench-f0.25-2026-09-12-pareto.svg)

| Configuration | n | accuracy | 95% CI | unparsed | cost / q | tokens / q | tool calls / q | latency p50 | latency p95 | no-decode | citation in range |
|---|---|---|---|---|---|---|---|---|---|---|---|
| agent | 140 | **65.0%** | 56.8–72.4 | 2 | $0.105 | 30,105 | 4.11 | 14.8 s | 44.0 s | 36% | 71% (n=138) |
| retrieval-only | 140 | **52.1%** | 43.9–60.2 | 0 | $0.025 | 6,735 | 1.00 | 9.0 s | 15.2 s | 100% | 53% (n=139) |
| uniform-32 (claude-sonnet-5) | 140 | **57.1%** | 48.9–65.0 | 5 | $0.036 | 11,486 | 0.00 | 12.3 s | 21.4 s | 100% | — |

## Accuracy by task type

| Task type | agent | retrieval-only | uniform-32 (claude-sonnet-5) |
|---|---|---|---|
| entity recognition | 63% (n=54) | 50% (n=54) | 54% (n=54) |
| event understanding | 65% (n=55) | 51% (n=55) | 62% (n=55) |
| key information retrieval | 76% (n=33) | 58% (n=33) | 55% (n=33) |
| reasoning | 56% (n=16) | 44% (n=16) | 62% (n=16) |
| summarization | 29% (n=7) | 57% (n=7) | 57% (n=7) |
| temporal grounding | 83% (n=12) | 67% (n=12) | 58% (n=12) |

## Tool-call profiles

**agent**: `search > search > view` ×17; `search > search` ×10; `view` ×9; `search` ×7; `search > search > get_transcript` ×4; `(none)` ×3; `search > search > view > search > search > view` ×3; `search > search > get_transcript > search > search > get_transcript` ×3

**retrieval-only**: `search` ×140

**uniform-32 (claude-sonnet-5)**: `(none)` ×140

## Runs

- **agent**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-agent.json` — {"benchmark": "lvbench", "policy": "agent", "budget_usd": 0.5, "budget_tokens": 120000, "max_tool_calls": 6, "sample": 387, "fraction": 0.25, "seed": 1, "skipped_not_indexed": 247, "vi_version": "vi 0.1.0", "started": "2026-09-12T13:55:18Z"}
- **retrieval-only**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-retrieval-only.json` — {"benchmark": "lvbench", "policy": "retrieval-only", "budget_usd": 0.5, "budget_tokens": 120000, "max_tool_calls": 6, "sample": 387, "fraction": 0.25, "seed": 1, "skipped_not_indexed": 247, "vi_version": "vi 0.1.0", "started": "2026-09-12T14:07:06Z"}
- **uniform-32 (claude-sonnet-5)**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-uniform-32.json` — {"benchmark": "lvbench", "policy": "uniform-32", "model": "claude-sonnet-5", "frames": 32, "transcript": true, "sample": 387, "fraction": 0.25, "seed": 1, "started": "2026-09-12T14:13:00Z"}

Questions per run: up to 140. Benchmark videos are public YouTube content and may be in model training data; read per-configuration deltas rather than absolute scores.
