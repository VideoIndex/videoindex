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

## Status (2026-10-01)

M0–M3 complete (index, search, agent, server, bindings, demo app). M4: results pages dated 2026-09-29 on 25% stratified samples (seed 1). Current agent (2026-09-29: the 09-26 loop plus `zoom`, one frame at source resolution with an optional enlarged region, and `view detail`, six 896 px tiles over at most 15 s) with the Gemini 3.8 Flash chat model; **unchanged by the nights of 2026-09-30 and 2026-10-01**, which measured five forms of the counting levers and merged none. Headline protocol **P2** (20 tool calls, $0.30 cost cap, 400k tokens, 300 s; the demo's cap since 2026-09-29): MINERVA 310 q **78.4%** ($0.104/q, p50 25 s; reference Gemini agentic 76.5% at $0.051), LVBench 340 q **83.5%** ($0.076/q; reference 80.8%). At P1 (12 calls, $0.50, 120k tokens, 120 s) the same agent is 73.9% on MINERVA; at P3 (30 calls, $0.75) 80.0% at $0.141/q. Retrieval-only 50.3 / 37.1, uniform-32 52.1 / 41.3. Corpus set (28 cross-video questions, `eval/data/corpus/`): Gemini 3.8 Flash **95%** quality at $0.110/q, Sonnet 5 92% at $0.237/q, recall 100% both. Default `AskBudget` for callers that set only some fields: 120k tokens, $0.50, 300 s, 8 calls; the server clamps a request to 50 calls / $2.00 / 1M tokens / 900 s. **The session of 2026-10-01** (`vi_internal/docs/planning/KICKOFF-tally-2026-10-01.md`, report `NIGHT-2026-10-01-report.md`, DECISIONS 2026-10-01), all at P2 against the v7 P2 rows: the count-twice rule narrowed to counts of things (`exp/count-v3`; its prompt bullet removed after the first check) 77.7% (18 fixed, 20 broken; FINE 12 → 20 with 8/0 and count-of-things rows 58 → 62, but the solid rows 209 → 202 and questions at the cost cap 8 → 16; cost +19%; LVBench 83.5%, 12/12) against the +5 bar; the loop-enforced tally for repeated actions (`exp/tally`: the loop itself issues 4 s pieces at 4 fps over the span the model's looks or citations name and asks for a piece-by-piece tally before the answer; adoption 97% of its rows) 77.4% (13/16; MOTION 11 → 4, 0/7), the fifth form of a denser look to leave MOTION flat or worse. Ported: the runner records `tally` (branch `merge/tally-2026-10-01`). The index-open race of 2026-09-30 was fixed on `main` by the live-core work (`5709551`). What is stable across count v2, v3 and v3b: the FINE gain (+4 to +8) from the second count on things in a frame, paid back by more calls elsewhere; what is settled: neither density nor an enforced tally makes the chat model count repeated actions from frames, and 32- and 63-row subsets read differently on two passes of the same binary (20 vs 12, 44 vs 38). Known gaps unchanged: MOTION, SWEEP and LOC, the P1 shortfall, LVBench entity recognition and event understanding. Queue: lever E (audio tags and word-level timestamps at index time, the only lever aimed at the AUDIO rows); a count-twice form that spends its second look only when the first count was cheap (the cost-cap doubling is where the FINE gain leaks); lever G for LVBench. 1H-VideoQA: 101 predictions, scored only via the Kaggle task in `vi_internal/tools`. Demo runs on the shared host `gcp_internal_1` (see `vi_internal/deploy/gcp_internal_1/README.md`, `vi_internal/deployment_snapshot.md`).
