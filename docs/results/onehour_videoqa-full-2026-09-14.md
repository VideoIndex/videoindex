# onehour_videoqa: all questions

Generated 2026-09-14 by `eval/report.py` from 1 run file(s). Accuracy is exact match on the option letter; intervals are 95% Wilson. Costs are provider list prices per question; indexing cost is not included for the index-based configurations.

| Configuration | n | accuracy | 95% CI | unparsed | cost / q | tokens / q | tool calls / q | latency p50 | latency p95 | no-decode | citation in range |
|---|---|---|---|---|---|---|---|---|---|---|---|
| agent | 101 | predictions only (no public answers) | — | 1 | $0.126 | 36,667 | 4.90 | 21.1 s | 45.6 s | 18% | — |

## Predicted letters

No public answers exist for this set, so there is no accuracy here; scoring happens on Kaggle, where a kaggle-benchmarks task asks the hosted VideoIndex API the same questions. The letter distribution is a sanity check for position bias (five-way questions: about 20% each if the answers are spread evenly).

**agent**: A 20, B 23, C 21, D 16, E 20, — 1

## Questions by task type

| Task type | agent |
|---|---|
| Reasoning | n=27 |
| Recall | n=74 |

## Tool-call profiles

**agent**: `search > search > view` ×16; `search > search` ×6; `search > search > view > view` ×5; `search > search > view > search > search > view` ×5; `search > search > view > view > view > describe` ×4; `search > search > search > search > view > view` ×4; `search > search > view > search > search > search > search` ×3; `search > search > view > view > view` ×3

## Runs

- **agent**: `/data/videoindex/eval/runs/onehour_videoqa-full-s1-agent.json` — {"benchmark": "onehour_videoqa", "policy": "agent", "budget_usd": 0.5, "budget_tokens": 120000, "max_tool_calls": 6, "sample": null, "fraction": null, "seed": 1, "skipped_not_indexed": 0, "vi_version": "vi 0.1.0", "started": "2026-09-14T21:42:03Z"}

Questions per run: up to 101. Benchmark videos are public YouTube content and may be in model training data; read per-configuration deltas rather than absolute scores.
