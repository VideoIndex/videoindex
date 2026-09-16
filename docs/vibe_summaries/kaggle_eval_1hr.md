● The host now has full tool access for the 1H-VideoQA index. I re-ran the Kaggle task against the host on a visual question (what the woman in the purple dress is doing when the first
  call comes in). The agent called view four times, decoded frames on the host with no media errors, and answered B, the same letter the local GPU run gave for that question.
  
  ┌────────────┬──────────────────────┬───────────┐
  │            │     host via API     │ local run │
  ├────────────┼──────────────────────┼───────────┤
  │ answer     │ B                    │ B         │
  ├────────────┼──────────────────────┼───────────┤
  │ tool calls │ 6 (2 search, 4 view) │ 6         │
  ├────────────┼──────────────────────┼───────────┤
  │ cost       │ $0.20                │ $0.21     │
  ├────────────┼──────────────────────┼───────────┤
  │ wall time  │ 31 s                 │ 38 s      │
  └────────────┴──────────────────────┴───────────┘
  
  The Kaggle notebook is ready to run against https://api.videoindex.app with index eval-onehour and the host's API key. Expect about 101 questions at roughly $0.13 each, so around $13
  against the key's $15 daily cap. If the notebook retries many rows the cap could trip mid-run, so raise the cap or run it in two sittings.
