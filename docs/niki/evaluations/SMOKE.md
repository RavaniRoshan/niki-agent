# Smoke runs

## Process-level parser smoke

### SDK-level evidence (completed)

- Model: `groq:qwen/qwen3.8-27b` via the Deep Agents SDK.
- Temp repo: `/tmp/parser-smoke`.
- Modified file: `parser.py` only.
- Result: `python3 -m pytest -q test_parser.py` → 2 passed.
- This is SDK-level evidence, not CLI validation.

### CLI smoke (`niki -n`)

Status: NOT RUN. The `niki` console entry point is available and accepts `-M`, `-S`, and `-n`. Every attempted Groq model fails before producing a file change with `groq.APIStatusError: Error code 413` because the on-demand Groq free tier has an 8,000 TPM limit while the Niki system prompt requires about 19,271 tokens. The required provider access for the planned `niki -n` path is therefore unavailable in this environment; no parser file was changed by a CLI run.

## Matched two-task Harbor smoke

Status: NOT RUN — no working Docker/container runtime or LangSmith credentials available in this environment.
