# Smoke runs

## Process-level parser smoke

### SDK-level evidence (completed)

- Model: `groq:qwen/qwen3.8-27b` via the Deep Agents SDK.
- Temp repo: `/tmp/parser-smoke`.
- Modified file: `parser.py` only.
- Result: `python3 -m pytest -q test_parser.py` → 2 passed.
- This is SDK-level evidence, not CLI validation.

### CLI smoke (`niki -n`)

Status: RUN, NOT PASSED — the CLI path works end to end, no available model solved the task.

Source commit `23f6afc4`; `niki` 0.1.80 from
`libs/evals/.venv/bin/niki`; temp repo `/tmp/parser-smoke` at baseline commit
`f4bd1c8` (2 failed, 5 passed). Prompt context measured by Niki at ~19.6K
tokens per request, so `num_ctx` 24576 was used with the local Ollama provider
(`ANTHROPIC_API_KEY`, `LANGSMITH_API_KEY`, `GROQ_API_KEY` still unset).

Every run below started `Starting LangGraph server… ✓ Server ready`, executed
real tool calls in the temp repo, produced a turn summary, and exited 0. None
left a passing parser:

| Model (`-M`) | Tool calls | Result of `python3 -m pytest -q test_parser.py` |
| --- | --- | --- |
| `ollama:qwen2.5:3b` — full-file spec | `write_file` | 3 failed, 4 passed |
| `ollama:qwen2.5:3b` — precise spec, 5 requests, 99.7K in / 801 out | 3× `edit_file`, `write_file` | 2 failed, 5 passed |
| `ollama:qwen2.5:3b` — read tests first, 2 requests | `read_file`, `edit_file` | 2 failed, 5 passed (file unchanged; edit did not apply) |
| `ollama:qwen3:4b` — 6.4 GB at 24K ctx, 64 % CPU / 36 % GPU | none | no assistant message in 7 m 40 s; stopped |
| `ollama:qwen2.5-coder:1.5b` — 100 % GPU at 24K ctx | none (tool call emitted as text) | 2 failed, 5 passed (file unchanged) |

Blocker: the model capability and local hardware, not the CLI or provider
access. The 3B models write plausible but incorrect parsers and stop before
re-running the suite; the 4B model is CPU-bound at the required context; the
1.5B model emits tool calls as JSON text. No Groq/Anthropic credential exists
to reach a model that can pass it, so CLI acceptance stays `BLOCKED` and the
SDK-level evidence above is not substituted.

## Matched two-task Harbor smoke

Status: NOT RUN — no working Docker/container runtime or LangSmith credentials available in this environment.
