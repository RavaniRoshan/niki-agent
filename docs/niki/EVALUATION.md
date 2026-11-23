# Niki Evaluation Protocol

## Benchmark

- Terminal-Bench 4.1 via Harbor 0.25.0.
- Niki target: Harbor `langgraph` agent, graph `dcode`.
- Competitor: Harbor `claude-code` agent.
- Model: `anthropic:claude-opus-5-5`, reasoning effort `max`.
- Sandbox: `langsmith`, timeout multiplier 1.0, retries 0, 3 trials per task.

## Process-level smoke case

In a fresh temp repo, ask Niki to update `parser.py` so `parse_host_port` handles IPv4, hostnames, and bracketed IPv6, raising `ValueError` for malformed inputs, then run `python -m pytest -q test_parser.py`. Only `parser.py` may change.

## Metrics

Resolution = reward == 1.0. Resolution rate = mean across task-trials. pass@3 = at least one successful trial per task. Paired bootstrap resamples shared task IDs (10,000 samples, seed 20261123).

## Gate

Niki resolution rate >= 55% and lower bound of paired 95% CI >= -10 percentage points vs Claude Code.
