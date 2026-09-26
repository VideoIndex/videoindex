# VideoIndex core — working notes for Claude

Rust SDK and infrastructure that turns long videos into a queryable knowledge base: index once
(decode, ASR, OCR, shots, embeddings, optional VLM descriptions), then ask questions through an
agent that cites the moment. Public repo `videoindex/videoindex`. Read `docs/README.md` once, in
order; `docs/10-roadmap.md` is the milestone tracker; `docs/results/` holds generated benchmark pages.

## The five repositories

| Repo | Path (GPU box) | Remote | What |
|---|---|---|---|
| core (this) | `~/videoindex_project/videoindex` | `videoindex/videoindex` | Rust crates, CLI, server, Python/Node bindings, eval harness, design docs |
| app | `~/videoindex_project/videoindex_app` | `VideoIndex/videoindex_app` | videoindex.app demo (Express proxy + React chat, social login), MkDocs SDK docs build |
| org | `~/videoindex_project/videoindex_org` | `VideoIndex/videoindex_org` | videoindex.org static front page, `/privacy`, `/terms` |
| docs site | `~/videoindex_project/videoindex.github.io` | `VideoIndex/videoindex.github.io` | Generated MkDocs output only (GitHub Pages) |
| internal | `~/videoindex_project/vi_internal` | `VideoIndex/vi_internal` | Deployment scripts, host facts, decisions log, reports, Kaggle tooling. Private. |

Public repos carry no deployment, machine or planning material; that lives in `vi_internal`.
Every non-obvious technical choice gets a dated entry in `vi_internal/docs/DECISIONS.md`.
What is deployed where (hosts, services, indexes, commits) is tracked in `vi_internal/deployment_snapshot.md`;
update it when a `vidx` build ships, an index is published or moved, or `/data/videoindex` changes shape.

## Layout

- `crates/`: `vi-core` (config, types, time model) · `vi-media` (libav decode in a sandboxed worker, acquirers, content-addressed media cache) · `vi-perceive` (ONNX: SigLIP, bge, RapidOCR, Silero VAD; features `cuda`, `onnx-dynamic`) · `vi-providers` (Anthropic/OpenAI/Gemini chat adapters, pricing, `tool_choice`) · `vi-index` (SQLite + FTS5 + blobs + vector store; `<id>.vidx` directories) · `vi-pipeline` (operator DAG scheduler, cache, planner, Python callback operators) · `vi-query` (hybrid retrieval, RRF) · `vi-agent` (tool loop: search, find_mentions, count_mentions, library_stats, list_videos, timeline, get_transcript, get_ocr, get_descriptions, view, describe; budgets; citations) · `vi-server` (axum: HTTP + SSE + blobs + API keys with daily spend cap + MCP streamable HTTP + metrics + OpenAPI; `vidx serve`) · `vi-cli` (`vidx`) · `vi-testkit`.
- `bindings/python` (PyO3/maturin, operators and policies in Python), `bindings/node` (napi-rs `@videoindex/core`).
- `eval/`: benchmark harness (LVBench, MINERVA, 1H-VideoQA loaders; `runners/answer.py`, `runners/baselines.py`, `runners/gemini.py`; `run.py` matrix from `configs/*.toml`; `report.py` writes `docs/results/*.md`).
- `scripts/`: ASR server, dev-set evals, `report/` (PDF technical reports into `vi_internal/reports`), Gemini dev-set harness, log timing tables.
- `config/gcp-a100.toml`: the only real config; every `vidx` command on the GPU box takes `--config config/gcp-a100.toml`.

## Build and test

```sh
cargo build --release -p vi-cli --features cuda     # GPU box; a plain build runs ONNX on the CPU
cargo test                                          # unit + integration (vi-server tests in crates/vi-server/tests)
cargo clippy --all-targets -- -D warnings
source /data/videoindex/asr/.venv/bin/activate      # Python for eval/, scripts/, bindings tests
cd bindings/python && maturin develop -q && python -m pytest -q tests/
cd bindings/node && npm run build:debug && node --test test/index.test.js
```

## Rules that have bitten us

- **Never `git add -A` here.** `maturin develop` drops a ~440 MB `.so` under `bindings/python/python/videoindex/` (ignored now, but it once forced a history rewrite). Stage files explicitly. `docs/vibe_summaries/` is the user's own notes; leave it unstaged.
- **Never rebuild `target/release/vidx` while a `vidx index` run is active.** Evals use a separate binary at `target/eval/release/vidx` (`cargo build --release -p vi-cli --features cuda --target-dir target/eval`).
- `pkill -f` patterns must not match your own shell (`dev[.]vidx` style); run long loops from script files with `setsid nohup`.
- Keys: `ANTHROPIC_API_KEY` is in the environment; `GEMINI_API_KEY`, `ELEVENLABS_API_KEY` in the git-ignored `.env` (`set -a; source .env; set +a`). Never print them or the host's demo API key.
- The user pushes this repo; commit and say so. Commit messages end with the Co-Authored-By line from the session.
- **Before designing or reading an experiment**, read `vi_internal/docs/eval/EVAL-LESSONS.md` (sample sizes, noise, paired diffs, switches on branches only) and `docs/08-evaluation.md` "Reading results". Experiment switches never land on main; the winning behaviour is ported as a plain change and re-measured with the main build.
- Docs are single-sourced here; the SDK site is regenerated from them (see the app repo's `docs/sync.sh`). Results pages are generated by `eval.report`, never edited by hand.
- **Keep the engineering document current.** `vi_internal/docs/eng/VideoIndex-Engineering.md` explains the system from the basics and logs every improvement tried. Whenever the indexing algorithm (operators, sampling, embeddings, scene/chapter logic, schema) or the question-answering component (agent loop, tools, budgets, prompts, retrieval fusion) changes, update its explanatory sections, add a dated row to its improvements log linking the note or DECISIONS entry, refresh "Where we stand" if results pages changed, rebuild the PDF (`python3 docs/eng/build.py` from `vi_internal`, ASR venv) and commit both files in the same session.

## Data on the GPU box (GCP a2-ultragpu-1g, A100 80 GB)

`/data/videoindex/indexes/{dev,dataset,eval-lvbench,eval-minerva,eval-onehour}.vidx`, media cache `/data/videoindex/videos` (by content hash), models `/data/videoindex/models`, eval data `/data/videoindex/eval/<bench>/`, runs `/data/videoindex/eval/runs/`, logs `/data/videoindex/logs/`. Whisper server: `scripts/asr_server.sh start` (127.0.0.1:9000). YouTube blocks this IP; downloads run on the deploy host or a Mac (`vi_internal/tools/download_videos_mac.py`).

## Status (2026-09-26)

M0–M3 complete (index, search, agent, server, bindings, demo app). M4: results pages dated 2026-09-26 on 25% stratified samples (seed 1). Current agent (2026-09-26: concurrent calls, hard cap, counted last turn, multi-window tools restricted to two or three located ranges, length continue-once) with the Gemini 3.8 Flash chat model at 12 tool calls: LVBench 340 q **82.1%** ($0.047/q, p50 12.7 s; reference Gemini agentic 80.8% at $0.059), MINERVA 310 q **72.6%** ($0.067/q, p50 21.8 s; reference 76.5%); retrieval-only 50.3 / 37.1, uniform-32 52.1 / 41.3. Corpus set (28 cross-video questions, `eval/data/corpus/`): Gemini 3.8 Flash **95%** quality at $0.106/q, Sonnet 5 93% at $0.235/q, recall 100% both; reference 93% at $0.406; retrieval-only 48%. The demo caps every question at 12 calls and $0.30 (app `BUDGET_CAPS`). History of the agent versions and the two regressions found on the way: `vi_internal/docs/DECISIONS.md` (2026-09-21 to 2026-09-26) and `vi_internal/docs/eng/VideoIndex-Engineering.md`. Known gaps: questions that exhaust 12 calls (MINERVA counting 52%, entity recognition, event understanding) and whole-library ranking questions on the corpus set; visual question types. Queue: `vi_internal/docs/planning/EVAL-IMPROVEMENTS.md` §3b (prompt wording for Sonnet cost, D `scan`, J2 `context_secs`, answer shaping, F per-kind RRF weights, H eval infrastructure, latency, docs, then M5 hosted mode). 1H-VideoQA: 101 predictions, scored only via the Kaggle task in `vi_internal/tools`. Demo runs on the shared host `gcp_internal_1` (see `vi_internal/deploy/gcp_internal_1/README.md`, `vi_internal/deployment_snapshot.md`).
