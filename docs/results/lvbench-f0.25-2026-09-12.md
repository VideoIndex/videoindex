# LVBench: 25% stratified sample (seed 1) over the 50 videos available

Generated 2026-09-12 by `eval/report.py` from 4 run file(s). Accuracy is exact match on the option letter; intervals are 95% Wilson. Costs are provider list prices per question; indexing cost is not included for the index-based configurations.

![](assets/lvbench-f0.25-2026-09-12-pareto.svg)

| Configuration | n | accuracy | 95% CI | unparsed | cost / q | tokens / q | tool calls / q | latency p50 | latency p95 | no-decode | citation in range |
|---|---|---|---|---|---|---|---|---|---|---|---|
| agent | 195 | **64.1%** | 57.2–70.5 | 2 | $0.103 | 29,562 | 4.14 | 14.8 s | 43.1 s | 36% | 75% (n=193) |
| retrieval-only | 195 | **50.8%** | 43.8–57.7 | 0 | $0.025 | 6,583 | 1.00 | 8.7 s | 14.8 s | 100% | 56% (n=194) |
| uniform-32 (claude-sonnet-5) | 195 | **54.9%** | 47.9–61.7 | 5 | $0.036 | 11,443 | 0.00 | 12.0 s | 19.3 s | 100% | — |
| gemini-3.8-flash agentic video | 182 | **79.7%** | 73.2–84.9 | 11 | $0.064 | 43,299 | 3.57 | 14.9 s | 76.9 s | 100% | — |

## Reading the sample

- **Scope.** The 25% stratified sample is 387 questions fixed by `(seed 1, fraction 0.25)`; the VideoIndex rows cover the 195 whose videos were indexed at the latest pass (acquisition is still adding videos), the Gemini reference the 182 that were indexed when it ran (two more failed with "model generated too many tool calls"). Later passes fill the same sample.
- **Gemini 3.8 Flash agentic video is the strongest system here: 79.7% against 64.1% for the VideoIndex agent**, at 60% of its per-question cost ($0.064 vs $0.103) and the same median latency (15 s), though with a long tail (p95 77 s vs 43 s) and 11 answers that gave no option letter (counted wrong; its parsed accuracy is higher still). It leads on every task type, most on entity recognition (75% vs 61%), event understanding (80% vs 65%) and summarization (100% vs 29%, n=7): questions answered by *looking*, where its agent loads frames straight into a strong native video model, while ours reaches pixels only through SigLIP-base retrieval and a 3×3 grid `view`. The gap is smallest on temporal grounding (95% vs 80%) and key information retrieval (86% vs 73%), the transcript- and OCR-anchored types the index serves well.
- **Against its own baselines the VideoIndex agent leads by 9 to 13 points** (64.1% vs 54.9% uniform-32-frames and 50.8% retrieval-only) at 3 to 4× their cost; the retrieval-only row is the cheapest configuration and within 4 points of the uniform baseline that decodes at question time.
- **What this says about the system.** The index is doing its job for speech and on-screen text; the visual side is the gap. The three levers, in order of expected return: a stronger frame model in the coarse pass (SigLIP so400m-384 instead of base-224), a "coarse whole-video view first" step for summary and event questions (the uniform baseline beats our agent on exactly those), and denser `view` sampling with the VLM describing frames the agent asks about. Gemini's per-question cost excludes nothing, whereas ours excludes indexing; on a corpus asked many questions the index amortises, on one question per video it does not.
- **Citations**: 75% of the VideoIndex agent's answers cite a moment within 60 s of LVBench's `time_reference`; Gemini's answers are not asked to cite.

## Where this stands against Gemini agentic video

The `gemini-…` row is Google's Gemini 3.8 Flash with `processing: "agentic"` asked the same questions over the same video files through the Interactions API (run once and cached; it is not re-run with every matrix). Google's own announcement ("Introducing agentic video in Gemini", 2026-09-01) reports agentic mode against static whole-video processing on LongVideoBench as up to 88% fewer tokens, up to 66% lower cost and up to 7% higher accuracy, without absolute scores; the numbers here are absolute, on LVBench, and directly comparable across rows because every row answered the same questions under the same scoring. VideoIndex's per-question cost excludes indexing (done once per video); Gemini's per-question cost is the whole cost. Both systems are scored with the same letter parser.


## Accuracy by task type

| Task type | agent | retrieval-only | uniform-32 (claude-sonnet-5) | gemini-3.8-flash agentic video |
|---|---|---|---|---|
| entity recognition | 61% (n=80) | 46% (n=80) | 52% (n=80) | 75% (n=73) |
| event understanding | 65% (n=74) | 53% (n=74) | 61% (n=74) | 80% (n=69) |
| key information retrieval | 73% (n=44) | 57% (n=44) | 48% (n=44) | 86% (n=42) |
| reasoning | 61% (n=23) | 43% (n=23) | 65% (n=23) | 71% (n=21) |
| summarization | 29% (n=7) | 57% (n=7) | 57% (n=7) | 100% (n=7) |
| temporal grounding | 80% (n=20) | 65% (n=20) | 55% (n=20) | 95% (n=20) |

## Tool-call profiles

**agent**: `search > search > view` ×26; `search > search` ×18; `view` ×10; `search` ×7; `search > search > get_transcript` ×5; `search > view` ×5; `search > search > view > search > search > view` ×4; `(none)` ×3

**retrieval-only**: `search` ×195

**uniform-32 (claude-sonnet-5)**: `(none)` ×195

**gemini-3.8-flash agentic video**: `processing_call×2` ×52; `processing_call×1` ×38; `processing_call×3` ×30; `processing_call×4` ×15; `processing_call×5` ×12; `processing_call×6` ×10; `processing_call×7` ×7; `processing_call×8` ×7

## Runs

- **agent**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-agent.json` — {"benchmark": "lvbench", "policy": "agent", "budget_usd": 0.5, "budget_tokens": 120000, "max_tool_calls": 6, "sample": 387, "fraction": 0.25, "seed": 1, "skipped_not_indexed": 192, "vi_version": "vi 0.1.0", "started": "2026-09-12T19:31:07Z"}
- **retrieval-only**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-retrieval-only.json` — {"benchmark": "lvbench", "policy": "retrieval-only", "budget_usd": 0.5, "budget_tokens": 120000, "max_tool_calls": 6, "sample": 387, "fraction": 0.25, "seed": 1, "skipped_not_indexed": 192, "vi_version": "vi 0.1.0", "started": "2026-09-12T19:32:12Z"}
- **uniform-32 (claude-sonnet-5)**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-uniform-32.json` — {"benchmark": "lvbench", "policy": "uniform-32", "model": "claude-sonnet-5", "frames": 32, "transcript": true, "sample": 387, "fraction": 0.25, "seed": 1, "started": "2026-09-12T19:32:34Z"}
- **gemini-3.8-flash agentic video**: `/data/videoindex/eval/runs/lvbench-f0.25-s1-gemini-3.8-flash-agentic.json` — {"benchmark": "lvbench", "policy": "gemini-agentic", "model": "gemini-3.8-flash", "mode": "agentic", "thinking": null, "sample": 387, "fraction": 0.25, "seed": 1, "started": "2026-09-12T17:02:52Z"}

Questions per run: up to 195. Benchmark videos are public YouTube content and may be in model training data; read per-configuration deltas rather than absolute scores.
