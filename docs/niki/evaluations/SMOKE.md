# Smoke runs

## Process-level parser smoke

### SDK-level evidence (completed)

- Model: `groq:qwen/qwen3.8-27b` via the Deep Agents SDK.
- Temp repo: `/tmp/parser-smoke`.
- Modified file: `parser.py` only.
- Result: `python3 -m pytest -q test_parser.py` → 2 passed.
- This is SDK-level evidence, not CLI validation.

### CLI smoke (`niki -n`)

Status: **superseded by owner decision (2026-11-26).** Local Ollama models are
dropped from this project's evidence: five runs at 24K context either wrote
plausible-but-wrong parsers or could not emit a tool call at all, so the runs
measured model capability on this machine, not the CLI. The next CLI smoke uses
a hosted free-tier model — Groq or the NVIDIA NIM free catalog — with the key
supplied per session; it is not a local-model test.

The earlier run log is preserved in git history at `71d49b65` for the record.
What it did establish, and what still holds: every run started the LangGraph
server, executed real tool calls in the temp repo, produced a turn summary, and
exited 0 — the CLI path works end to end. What it did not establish is a pass on
the parser task, and it is not claimed as one.

## Matched two-task Harbor smoke

Status: **removed (2026-11-26).** The Harbor rig and the Claude Code comparison are
out of scope until a later session has a working sandbox runtime and keys; see
`docs/niki/EVALUATION.md`. The harness was deleted on 2026-11-26, so this section
will not survive the next change to this file.

Rechecked 2026-11-25 before publishing the completion branch: `ANTHROPIC_API_KEY`,
`LANGSMITH_API_KEY`, and `GROQ_API_KEY` are still unset, no Docker binary is
installed, and Podman fails to start (`default OCI runtime "crun" not found`;
`/run/user/1000` is missing, so the rootless runtime directory is not writable).
Nothing was submitted, so there is no paired result and no competitiveness claim.
A mock or SDK-level run would not count as a matched comparison and is not
recorded as one.
