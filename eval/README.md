# Evaluation harness

Runs the long-video QA benchmarks named in `docs/08-evaluation.md` end to end over
VideoIndex indexes, across policy and provider configurations, and reports accuracy
against cost. Python; uses the `vi` CLI (and the HTTP API when a server is up). The
SDK knows nothing about benchmarks.

```
eval/
  datasets/    loaders: lvbench.py, minerva.py, onehour_videoqa.py; acquire.py (yt-dlp + vi index)
  runners/     answer.py (ask per question, parse the option letter), baselines.py (uniform frames)
  metrics.py   accuracy overall and per task type, tokens, cost, latency, tool-call histogram, Wilson CIs
  report.py    markdown tables + pareto plot into docs/results/
  configs/     run matrices (policy × budget × provider)
```

## Flow

```bash
# 1. annotations + videos (YouTube; works from this host with yt-dlp + deno)
python3 -m eval.datasets.acquire lvbench --root /data/videoindex/eval/lvbench
# 2. index (coarse pass on the GPU build)
python3 -m eval.datasets.acquire lvbench --root /data/videoindex/eval/lvbench --index /data/videoindex/indexes/eval-lvbench.vidx --config config/gcp-a100.toml
# 3. answer with a configuration, on a stratified sample or the full set
python3 -m eval.runners.answer lvbench --root /data/videoindex/eval/lvbench --index /data/videoindex/indexes/eval-lvbench.vidx \
    --config config/gcp-a100.toml --policy agent --sample 300 --out /data/videoindex/eval/runs/lvbench-agent.json
python3 -m eval.runners.answer lvbench ... --policy retrieval-only --out .../lvbench-retrieval.json
python3 -m eval.runners.baselines lvbench ... --frames 32 --out .../lvbench-uniform32.json
# 4. report
python3 -m eval.report lvbench /data/videoindex/eval/runs/lvbench-*.json --out docs/results/lvbench-2026-09-12.md
```

Every run file records the question set, the exact configuration (policy, budget, model
ids, prompt hashes when the CLI reports them), per-question answers, parsed choices,
usage and timing, so runs are comparable and re-scorable.

## Multiple-choice protocol

The question and its lettered options go to `vi ask` restricted to the question's video
(`--video`) with a fixed budget. The prompt asks for reasoning then a final line
`Answer: X`. The parser takes the last `Answer: X`, else a lone `(X)`/`X.` at the end, else
the first option letter that appears in the last line; unparseable answers count as wrong
and are logged. Accuracy is exact match on the letter. LVBench's `time_reference` is kept
for a citation-in-range metric when the answer carries citations.

Licensing: LVBench is CC BY-NC-SA 4.0 (research only); Minerva CC BY 4.0; 1H-VideoQA via
Kaggle terms. Benchmark videos are public YouTube content and may be in model training
data; the report says so and prefers per-configuration deltas over absolute scores.
