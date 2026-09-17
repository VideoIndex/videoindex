# Evaluation harness

Runs the long-video QA benchmarks named in `docs/08-evaluation.md` end to end over
VideoIndex indexes, across policy and provider configurations, and reports accuracy
against cost. Python; uses the `vidx` CLI (and the HTTP API when a server is up). The
SDK knows nothing about benchmarks.

```
eval/
  datasets/    loaders: lvbench.py, minerva.py, onehour_videoqa.py; corpus.py (our cross-video set); acquire.py (yt-dlp + vidx index)
  data/corpus/ questions.json: the corpus question set with its derived ground truth
  runners/     answer.py (ask per question, parse the option letter), baselines.py (uniform frames),
               gemini.py (Gemini agentic video, one video), corpus.py (vidx ask over a whole index),
               corpus_gemini.py (Gemini agentic video over a library, map-reduce over batches of 10 files)
  metrics.py   accuracy overall and per task type, tokens, cost, latency, tool-call histogram, Wilson CIs
  judge.py     corpus scoring: a judge model extracts claims, then deterministic set / anchor / fact scores
  report.py    markdown tables + pareto plot into docs/results/ (multiple-choice benchmarks)
  corpus_report.py  markdown + charts (+ PDF) for corpus runs
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

## Sampled runs (while iterating on the core)

```bash
# 25% of the benchmark, stratified by task type, same questions for every configuration; one command runs the matrix and the report
python3 -m eval.run eval/configs/lvbench-first.toml --fraction 0.25 --seed 1 --jobs 4
# acquire only the videos that sample needs
python3 -m eval.datasets.acquire minerva --root /data/videoindex/eval/minerva --fraction 0.25 --seed 1 --download
```

The sample is drawn from the whole benchmark before filtering by what is indexed, so `(seed, fraction)`
names a fixed question set: runs made while videos are still arriving answer a subset of it and record
how many were skipped, and a later run with `--resume` fills them in. Run files are named
`<benchmark>-f0.25-s1-<run>.json`.

## Corpus QA (questions that span the library)

The public benchmarks ask about one video at a time. `eval/data/corpus/questions.json` holds our own
set of 28 questions over the 30-video `dataset` index that need the whole library: *which talks mention
Anthropic / DeepSeek / LoRA*, *show the moments where speakers talk about improving RAG*, *compare how
Noam Brown and Oriol Vinyals describe self-play*, *summarize the themes of both event series*, *which
model family is named most*, and one question nothing answers. Ground truth for mention questions is
derived from the index's transcript and OCR rows by regex (`python3 -m eval.datasets.corpus build`),
with curated exclusions for false friends ("the notion that", a mouse cursor) and on-screen-only hits
treated as acceptable but not required; topic, synthesis and library questions carry curated expected
videos and key facts. `eval/datasets/corpus.py` documents the fields.

```bash
python3 -m eval.run eval/configs/corpus-dataset.toml --jobs 3 --run-reference --pdf ../vi_internal/reports/videoindex-corpus-eval-$(date +%F).pdf
# or step by step
python3 -m eval.runners.corpus --config config/gcp-a100.toml --policy agent --out /data/videoindex/eval/runs/corpus/corpus-vi-agent-sonnet.json
python3 -m eval.runners.corpus_gemini --out /data/videoindex/eval/runs/corpus/corpus-gemini-agentic.json   # GEMINI_API_KEY from .env
python3 -m eval.judge /data/videoindex/eval/runs/corpus/corpus-*.json                                       # ANTHROPIC_API_KEY
python3 -m eval.corpus_report /data/videoindex/eval/runs/corpus/corpus-*.json --out docs/results/corpus-$(date +%F).md --pdf ...
```

Both systems get the same question and answer format (title, timestamps, quote per video, `Videos: N`).
Gemini's API takes at most ten videos per agentic request and the library does not fit its static
context, so the Gemini runner answers each question over three parallel ten-video agentic requests and
merges the partial answers with a text-only call; its latency is the slowest batch plus the merge and
its cost the sum of all four calls. A judge model (Claude Opus 5) maps each answer's named videos to the
catalog, converts timestamps to seconds and marks key facts; the scores (precision / recall / F1 of
named videos with acceptable extras ignored, share of timestamps within 90 s of a real mention, fact
coverage, and a per-kind quality score) are computed deterministically in `eval/judge.py`.

## Multiple-choice protocol

The question and its lettered options go to `vidx ask` restricted to the question's video
(`--video`) with a fixed budget. The prompt asks for reasoning then a final line
`Answer: X`. The parser takes the last `Answer: X`, else a lone `(X)`/`X.` at the end, else
the first option letter that appears in the last line; unparseable answers count as wrong
and are logged. Accuracy is exact match on the letter. LVBench's `time_reference` is kept
for a citation-in-range metric when the answer carries citations.

1H-VideoQA has no public answers: Kaggle injects them server-side when a `kaggle-benchmarks` task runs, so the score comes from a task that asks the hosted VideoIndex API the same questions (kept with the deployment tooling). Local runs over this set produce predictions only; `eval.report --submission out.csv` exports them in the `Final Answer: (X)` form for reference.

Licensing: LVBench is CC BY-NC-SA 4.0 (research only); Minerva CC BY 4.0; 1H-VideoQA via
Kaggle terms. Benchmark videos are public YouTube content and may be in model training
data; the report says so and prefers per-configuration deltas over absolute scores.
