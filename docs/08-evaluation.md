# 08. Evaluation

The point of the provider abstraction is measurement. The eval harness runs the three benchmarks named in the README end to end, over the same indexes, across provider and policy configurations, and reports accuracy against cost.

## Benchmarks

| Benchmark | Content | Questions | Format | Source |
|---|---|---|---|---|
| **LVBench** (zai-org, ICCV 2025) | 103 YouTube videos, each over 30 min, average 68 min, about 117 hours total | 1,549 | multiple choice; six task types: entity recognition, event understanding, key information retrieval, temporal grounding, reasoning, summarization | github.com/zai-org/LVBench; videos must be downloaded from YouTube |
| **Minerva** (Google DeepMind, 2025) | Videos averaging about 12 min | 1,515 hand-crafted | multiple choice with reasoning traces; emphasizes multi-step reasoning | DeepMind release; confirm current download location during M4 |
| **1H-VideoQA** (Google DeepMind) | 21 YouTube videos, 40 to 90 min | 101 | five-way multiple choice, collected in-house | github.com/google-deepmind/1h-videoqa (Apache-2.0), data via the linked Kaggle benchmark; videos must be downloaded from YouTube. Small set, so report per-question results and a confidence interval, not just a percentage. |

Google's agentic-video announcement reports up to 88% token reduction, up to 66% cost reduction, and up to 7% accuracy gain over uniform 1 fps sampling. Those three numbers, accuracy, tokens, and cost relative to a uniform-sampling baseline, are the headline metrics here too.

## Harness design

The harness lives in `eval/` and is Python. It uses the SDK; the SDK has no knowledge of benchmarks.

```
eval/
  datasets/
    lvbench.py        loader: questions, options, answers, video ids, task type
    minerva.py
    onehour_videoqa.py
    acquire.py        maps benchmark video ids to Sources, drives `vidx acquire`
  runners/
    index.py          builds indexes per policy config, reuses caches
    answer.py         runs `ask` per question with a fixed budget, records everything
    baselines.py      uniform-sampling baseline: N frames + transcript in one VLM call
  metrics.py          accuracy, per-task accuracy, tokens, cost, latency, tool-call histograms
  report.py           tables and pareto plots (accuracy vs cost), markdown + JSON
  configs/
    *.toml            provider roles + policy + budget matrices
```

### Run structure

1. **Acquire**: download benchmark videos once. All three benchmarks are YouTube content and YouTube blocks most downloads from datacenter IPs, so downloads run on the development Mac with `scripts/download_videos.sh` and are transferred to azuremc's media cache with rsync. LVBench alone is about 117 hours, roughly 100 to 150 GB at 720p; budget disk and a day or more of residential bandwidth.
2. **Index**: for each index configuration (sampling policy × provider roles), build or extend the index. Caching means changing only the VLM re-runs only the VLM stage.
3. **Answer**: for each question, call `ask` with the benchmark's option list in the prompt and a fixed `Budget`. Record the streamed event log, tool calls, usage, and the final choice. The answer parser extracts the option letter; unparseable answers count as wrong and are logged.
4. **Score**: accuracy overall and per task type; mean and p95 tokens, cost, wall-clock, tool calls per question.
5. **Report**: one markdown table per benchmark per run set and a pareto plot of accuracy versus cost per question with one point per configuration.

### Configurations to compare first

| Axis | Values |
|---|---|
| Agent LLM | Gemini 2.5 Flash, Claude Sonnet 5, an open model via vLLM |
| VLM describe | same three |
| Policy | retrieval-only; default agentic; fixed "view top-3 scenes"; uniform-sampling baseline (no index) |
| Sampling | 0.5, 1, 2 fps coarse pass |
| Budget | 20k, 50k, 150k tokens per question |

### Controls

- Temperature 0 where the provider allows it, fixed seeds where it applies.
- Every run pins prompt hashes and provider model versions; the report lists them.
- Retrieval-only and uniform-sampling baselines run on every benchmark so the marginal value of the agentic loop and of the index are each visible.
- Contamination note: benchmark videos are public YouTube content and may be in model training data. Report it, and prefer per-configuration deltas over absolute scores.

## Metrics

| Metric | Definition |
|---|---|
| Accuracy | fraction of questions with the correct option |
| Accuracy by task type | LVBench's six categories, Minerva's categories |
| Tokens per question | input plus output across all provider calls in the `ask` |
| Cost per question | from Provenance rows, USD |
| Latency | wall-clock per `ask`, p50 and p95 |
| Index cost | USD and wall-clock per hour of video, per policy |
| Tool-call profile | histogram of tool sequences; fraction of questions answered without decode |
| Citation precision | on a manually labeled subset, fraction of citations whose window contains the evidence |

## Development dataset

`dataset/videolist.md` lists two YouTube playlists: AI Engineer conference workshops (multi-hour sessions) and the Berkeley AI MOOC 2025. These are the demo app's content and the development smoke set. They are long, lecture-style, slide-heavy videos, which stresses OCR, transcript search, and chapter segmentation, and matches the LVBench and 1H-VideoQA regime. A small hand-written QA set of about 50 questions with timestamps over these videos serves as a fast regression test that runs on every change to prompts or policies.

Acquisition runs on the development Mac, not on azuremc, because YouTube blocks most datacenter IPs. `scripts/download_videos.sh` installs yt-dlp via Homebrew if missing, downloads the playlists at 720p with subtitles, chapters, and `.info.json` metadata into `dataset/videos/`, keeps a download archive so re-runs only fetch new entries, and prints the rsync command that moves the files into `/data/videoindex/videos/` on azuremc. The same script takes a plain-text file of URLs, which is how benchmark video lists are fetched. On azuremc the `LocalFile` acquirer imports the `.info.json` and subtitle sidecars so transferred downloads keep their metadata.

## Reading results

Lessons from the first rounds, so the pages under `docs/results/` are read the way they were
produced.

- The samples are fixed (25% stratified, seed 1: 340 LVBench and 310 MINERVA questions) so runs
  pair question by question. Between two near-identical agents about 40 of 310 questions flip,
  which is ±2 points of accuracy; the 95% interval on 340 questions is about ±4 points. A paired
  comparison (questions fixed against questions broken, with an exact McNemar test) is the reading
  that means something; two headline percentages one or two points apart do not.
- Per-task-type deltas are noise unless the type is large: a 30-question type swings ±10 points
  between runs of equal overall accuracy.
- Effects of a few points hide in the full sample. The first 120 questions of the MINERVA sample
  showed no gap between two agents that differ by two points overall; slices of 60 or 120
  questions are for checking a mechanism on the questions where it applies, not for measuring.
- Split before you rerun. The two regressions found so far were located from the run files alone,
  by splitting the questions on a feature of the run (calls used, a tool called, a multi-window
  call made) and comparing agents on the same questions.
- Every tool argument that can change behaviour is recorded per call in the run file
  (`calls: [{tool, turn, ms, windows}]`); a hypothesis that cannot be read from the run file
  cannot be tested without a rerun.
- Cost and latency medians are stable at these sizes; a 20% latency change is real when a
  two-point accuracy change is not.
- Every experiment gets a new run name (the runner resumes an existing file), and results pages
  are generated, never edited.

## Regression gates

- The 50-question dev set runs in CI nightly against the default configuration; accuracy drops of more than 3 points fail the run.
- Unit fixtures: a 2-minute synthetic video with known cuts, burned-in text, and a scripted voice track validates shot detection, OCR, and ASR alignment deterministically without network.
