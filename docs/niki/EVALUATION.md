# Niki Evaluation Protocol

## Benchmark

- Terminal-Bench 4.1 via Harbor 0.25.0.
- Dataset ref: `terminal-bench/terminal-bench@4.1.0`.
- Niki target: Harbor `langgraph` agent, graph `dcode`.
- Competitor: Harbor `claude-code` agent.
- Model: `anthropic:claude-opus-5-5`, reasoning effort `max`.
- Sandbox: `langsmith`, timeout multiplier 1.0, retries 0, 3 attempts per task, 1 concurrent trial.
- Separate output directories: `harbor-jobs/niki-terminal-bench/` and `harbor-jobs/claude-code-terminal-bench/`.

## Process-level smoke case

In a fresh temp repo, ask Niki to update `parser.py` so `parse_host_port` handles IPv4, hostnames, and bracketed IPv6, raising `ValueError` for malformed inputs, then run `python -m pytest -q test_parser.py`. Only `parser.py` may change.

## Metrics

- Resolution = reward == 1.0. Resolution rate = mean across task-trials.
- pass@1 = fraction of tasks whose first attempt has reward 1.0.
- pass@3 = fraction of tasks with any successful attempt.
- Paired bootstrap resamples shared task IDs (10,000 samples, seed 20261123) with linear interpolation for the 2.5th and 97.5th percentiles.
- Duration percentiles use the same linear interpolation rule.
- Token and cost totals are reported where Harbor provides them.

## Gate

Niki resolution rate >= 55% and lower bound of paired 95% CI >= -10 percentage points vs Claude Code. The planned pre-Task-1 baseline was missed and cannot be reconstructed; label any capture `first post-Task-11 baseline`.
EOF