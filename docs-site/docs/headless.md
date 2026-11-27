# Headless and scripted runs

`niki -n` runs one prompt and exits without opening the full-screen app. It is
the entry point for pipes, scripts and CI jobs, and an editor uses `--acp`
instead. The answer goes to stdout, progress goes to stderr, and the exit status
says how the turn ended.

## One prompt from the shell

```bash
niki -n "summarise the failures in test.log"
```

The turn runs in the workspace named by `--root`, or in the current directory,
with the model from `--model` or `[models].default`. The prompt always belongs to
`-n` or `--prompt`; a bare argument is refused before anything runs:

```text
niki: unexpected argument "summarise the failures"; the prompt is passed with -n
```

`--mode`, `--allow`, `--max-steps` (default 40), `--system` and `--model` shape
the turn exactly as they do in the app. `--resume` and `--continue` append the
new turn to a saved session; see [Sessions](/sessions).

## What goes where

stdout carries the answer. In the default text format it is streamed as the
model produces it, so `niki -n "…" > answer.txt` leaves a file with the answer
and nothing else.

stderr carries everything else:

| Line | Meaning |
| --- | --- |
| `· <tool> <arguments>` | A tool is about to run. |
| `  ✓ <tool> <output>` | The tool succeeded. |
| `  ✗ <tool> <output>` | The tool failed. |
| `niki: <message>` | A notice from the agent. |
| `niki: warning: <message>` | A skill, hook, MCP server or session store problem. |
| `niki: step limit of <n> reached` | The step budget ran out. |
| `niki: cancelled` | Ctrl-C stopped the turn. |
| `N steps, M tool calls, X in / Y out tokens` | The turn's totals. |

`--quiet` (or `-q`) drops the tool lines, the step-limit and cancelled lines and
the totals. Notices and warnings still print.

```bash
niki -n "explain the failing test" > answer.txt
niki -n "explain the failing test" --quiet 2> warnings.txt
```

The tool lines are written to stderr in every output format, including `json`
and `jsonl`, so a pipeline that parses stdout is unaffected either way.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | The turn finished. A refused tool is a result, not a failure. |
| `1` | The turn could not run: a configuration, provider or workspace failure, or an option that could not be parsed. |
| `2` | Usage error: commands or arguments that do not combine. |
| `124` | The step budget ran out before the model stopped calling tools. |
| `130` | Cancelled with Ctrl-C. |

Exit 2 is reserved for combinations Niki refuses before a run starts:

```text
niki: sessions needs `list`; run niki sessions list
niki: --acp and -n cannot be combined; --acp serves an editor on stdin/stdout and -n runs one prompt
niki: --json is only for `niki sessions list`
niki: doctor only diagnoses the install; drop --acp and -n to run it
```

A headless run has nobody to answer an approval prompt. A shell command that is
not on the allow-list is refused, the model sees the refusal as a tool result,
and the turn still exits 0. If a job must run commands, pass `--allow` or choose
a mode that runs them; [Permissions](/permissions) has the exact gate.

## `--output-format json`

`json` (also spelled `--output json`) prints one object after the turn ends:

| Field | Value |
| --- | --- |
| `answer` | The answer text. |
| `steps` | Model turns taken. |
| `tool_calls` | Tool calls made. |
| `usage` | `input_tokens` and `output_tokens` for this turn. |
| `session_usage` | `input_tokens`, `output_tokens` and `steps` for the session. |
| `context_audit` | `system`, `tool_schemas`, `messages` and `total` tokens, plus `tools`, the tool count. |
| `stop` | `end_turn`, `tool_use`, `max_tokens`, `cancelled`, `step_limit` or `other`. |
| `session` | The session id, or null when the turn was not saved. |

```bash
niki -n "list the public functions in src/lib.rs" --output-format json > turn.json
```

The object is the last line on stdout. The tool lines on stderr are unchanged,
so pass `--quiet` when a job's log should hold only the result.

## `--output-format jsonl`

`jsonl` streams one JSON object per line, in order. Each line has a `type`:

| `type` | Fields |
| --- | --- |
| `start` | `model`, `session`, `cwd` |
| `text` | `delta` |
| `tool_call` | `id`, `name`, `arguments` (the parsed JSON, or `{"raw": …}` when it does not parse) |
| `tool_result` | `id`, `name`, `ok`, `text`, and `truncated` when the text was cut |
| `usage` | `input`, `output`, `total` |
| `error` | `message` |
| `end` | `reason`, `steps`, `tool_calls`, `usage` with `input` and `output` |

A tool result longer than 4096 bytes is cut and its line gains
`"truncated": true`. Notices, the step limit and a cancellation produce no line
of their own: `end` carries `reason` `cancelled` or `step_limit`. When the turn
fails, `error` comes first, then `end` with reason `error`, and the process exits
1.

```bash
niki -n "find the TODO comments" --output-format jsonl --quiet
{"type":"start","model":"anthropic:my-model","session":"000000000000000018ebde128ab2bf5e-00","cwd":"/home/you/src/app"}
{"type":"tool_call","id":"call_1","name":"grep","arguments":{"pattern":"TODO"}}
{"type":"tool_result","id":"call_1","name":"grep","ok":true,"text":"src/lib.rs:12: // TODO"}
{"type":"usage","input":412,"output":40,"total":452}
{"type":"text","delta":"There are three"}
{"type":"usage","input":400,"output":56,"total":456}
{"type":"end","reason":"end_turn","steps":2,"tool_calls":1,"usage":{"input":812,"output":96}}
```

## Commands that do not need a model

Four commands are handled before a provider is chosen, so they work without an
API key:

```bash
niki -n "/diff"                     # the working tree's staged, unstaged and untracked changes
niki -n "/init"                     # write an AGENTS.md template when the workspace has none
niki -n "/rename deploy notes"      # name the current session
niki -n "/export"                   # write it as Markdown and print the path
```

The first is matched exactly; the other three take the first word of the prompt.
`/rename` and `/export` act on `--resume`/`--continue` when given, else on the
newest session for the workspace. [Sessions](/sessions) covers what each writes.

## Editor integration with `--acp`

`niki --acp` serves the editor protocol on stdin/stdout instead of opening the
app. The editor starts that command as a child process, names the workspace in
`session/new`, and speaks the protocol over the child's stdin and stdout.
`--model`, `--config`, `--mode`, `--allow`, `--max-steps` and `--system` apply
as they do to a headless run.

```bash
niki --acp
```

Frames are newline-delimited JSON-RPC 2.0. stdout carries frames only, and
warnings go to stderr, so nothing else may write to stdout while the server
runs. The process exits 0 when the editor closes stdin.

The server implements `initialize`, `session/new`, `session/load`,
`session/prompt`, `session/set_mode` and `session/set_config_option`. A
`session/cancel` notification raises the turn's cancel handle. In the other
direction it sends `session/update` notifications for message chunks, tool calls
and plans, and a `session/request_permission` request when the policy gates a
call; the editor's answer decides what the turn does. `authenticate`,
`session/list` and the unstable methods answer `-32601`.

`session/new` takes the workspace in `cwd` and returns a `sessionId` plus the
available modes. `session/load` replays a saved transcript when the session
store is open, and `initialize` reports `loadSession` for that capability.
Sessions from `--acp` are saved in the same `sessions.db` as the app and
headless runs, so their ids work with `niki sessions list` and `--resume`.

The first frames of a session:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/home/you/src/app"}}
{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"<id>","prompt":[{"type":"text","text":"summarise the failing test"}]}}
```

The prompt replies with a stop reason: `end_turn`, `max_tokens`, `cancelled` or
`max_turn_requests`, the last when the step budget ran out.

## `niki doctor`

`doctor` prints a read-only diagnosis of the install. It writes nothing, opens
no network connection, starts no MCP server and creates no session store, so it
is safe to run in a fresh checkout or over a pipe.

It prints nine checks, then a summary:

| Check | Fails when |
| --- | --- |
| `version` | Never; it prints `niki <version>` and the MCP protocol date. |
| `config` | The file exists but cannot be read or parsed. A missing file is a warning. |
| `model` | No model resolves, or the provider is not one this build knows. |
| `credential` | The provider's variable is unset or blank, or the block names no variable. A keyless endpoint passes. |
| `sessions` | Never; a store that cannot be read is a warning. |
| `instructions` | Never; it reports the file count and bytes for the working directory. |
| `skills` | Never; it reports the skill count and warning count. |
| `mcp` | Never; it reports how many servers are configured. |
| `hooks` | Never; hook problems are warnings. |

Each line is `ok`, `warn` or `fail`, padded to five columns, then the check name
and a one-line detail:

```text
ok   version: niki 2.1.1 (MCP protocol 2025-06-18)
ok   config: /home/you/.niki/config.toml exists and parses
ok   model: anthropic:my-model
ok   credential: anthropic: ANTHROPIC_API_KEY is set
ok   sessions: /home/you/.niki: 0 saved session(s)
ok   instructions: 1 file(s), 5170 bytes for the working directory
ok   skills: 2 skill(s), 0 warning(s)
ok   mcp: 0 server(s) configured (none started)
ok   hooks: no hooks configured
summary: 9 check(s): 9 ok, 0 warning(s), 0 failure(s) — the install is usable
```

Only a `fail` changes the exit status: the command exits 1 when a turn could not
start, and 0 when every check is `ok` or `warn`. A missing variable is named, but
its value is never printed:

```text
fail credential: anthropic: ANTHROPIC_API_KEY is unset; export ANTHROPIC_API_KEY before running a turn
summary: 9 check(s): 8 ok, 0 warning(s), 1 failure(s) — fix the failures above
```

`--config` and `--model` let you diagnose a file or a model other than the
default. The exit status makes the command usable as a preflight step in a job.

## A CI job that fails on a non-zero exit

The shell's exit status is the job's. With `set -e`, any non-zero exit from
`niki` ends the step with that status:

```bash
#!/usr/bin/env bash
set -euo pipefail

: "${ANTHROPIC_API_KEY:?set the key in the job}"

niki -n "run the test suite and write a short summary to test-report.md" \
  --root . \
  --allow 'cargo*,git*' \
  --output-format json \
  --quiet > turn.json
```

The same lines work as the body of a CI step. `--allow` keeps the commands the
turn needs from waiting for an approval nobody can give; without it, an
ungated command is refused and the turn still exits 0. `--output-format json`
leaves the answer, the token counts and the stop reason in `turn.json`, and
`--quiet` keeps the job log free of tool lines.

Add `niki doctor` before the turn when a missing key should fail the job with a
named cause rather than a provider error.
