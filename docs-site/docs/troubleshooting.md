# Troubleshooting

Start with what Niki says about itself:

```bash
niki doctor      # nine checks: config, model, credential, store, skills, hooks, MCP
niki --version   # the version line
niki --help      # every option
```

`niki` either opens the app, runs one prompt with `-n`, serves an editor with
`--acp`, or runs the read-only `doctor` report. `doctor` prints one `ok`, `warn`
or `fail` line per check, never prints a credential value, and exits 1 when a
turn could not run — it names the environment variable to set.

A headless run prints failures as `niki: <message>` and warnings as
`niki: warning: <message>` on stderr, so the answer on stdout stays clean. The
full-screen app shows warnings as rows in the transcript and failures before the
first frame.

## `the full-screen app needs a terminal`

A pipe or a non-interactive shell has no screen to draw. `niki` without `-n`
requires a terminal:

```text
niki: the full-screen app needs a terminal; pass -n <PROMPT> to run one prompt headlessly
```

Run one prompt instead:

```bash
niki -n "summarize this repository"
```

## `unexpected argument`

The prompt is never positional. Every value belongs to a flag, and the prompt
goes after `-n`:

```text
niki: unexpected argument "fix the failing test"; the prompt is passed with -n
```

```bash
niki -n "fix the failing test"        # right
niki "fix the failing test"           # wrong: unexpected argument
```

An unknown option is `niki: unknown option <name>; run niki --help`.

## The configuration does not load

The default file is `$NIKI_HOME/config.toml`, else `~/.niki/config.toml`.
`--config <PATH>` selects another. A missing file is an empty configuration, not
a failure.

| Message | Cause |
| --- | --- |
| `cannot read <path>: …` | The file exists but permission or I/O stopped the read. |
| `invalid TOML in <path>: …` | The parser names the line and column. A wrong type for a known key is this error too. |
| ``mcp server `<name>` in config.toml: …`` | One `[mcp_servers.<name>]` entry is malformed. The message names the server and what it needs. |

Unknown keys are ignored, so a configuration written for a different Niki
surface still loads.

## No model, or an unknown model

| Message | Fix |
| --- | --- |
| `niki: no model selected: set [models].default in <path> or pass --model provider:model` | Add `[models].default`, or pass `--model`. |
| `niki: model spec "<spec>" is not provider:model (for example groq:llama-3.3-70b)` | The spec needs a colon. |
| `niki: unknown provider "<name>"; this build resolves: …` | Use a name from the list, or add a `[providers.<name>]` block. |

The model picker lists `[models].default` and every `provider:model` in a user
block's `models` array. A built-in provider with no block does not appear
although it still runs from `--model` or the default.

An unknown `--mode` is a warning, not a failure: the run continues in `manual`.

## A provider rejects the key

| Message | Fix |
| --- | --- |
| `niki: <provider> needs an API key: set <VAR>` | Export the variable named by that provider's `api_key_env`. |
| `niki: <provider> returned HTTP 401: …` | The key is wrong or expired. |
| `niki: <provider> returned HTTP 403: …` | The key lacks access to that model or endpoint. |
| `niki: <provider> sent a stream this client cannot read: …` | The endpoint speaks the other wire protocol; change `protocol`. |

Niki reads keys only from the environment variable named in the provider block.
It does not write a key to disk, so there is no stored-key fallback to check.
The variable name for each provider is in [Providers](/providers/).

## The workspace will not open

`niki: open the workspace at <path>: …` means the root could not be canonicalised.
The path must exist; `--root` does not create it. `niki` uses the current
directory when `--root` is absent.

A tool path outside the root is refused with `<path> is outside the workspace
root <root>`. Change the argument, or start Niki at a root that contains it. The
shell tool's `cwd` is checked the same way.

## An MCP server times out

```text
mcp server `name`: initialize timed out after 10s
```

Startup gives every server 10 seconds for the handshake, and connects them
concurrently. One slow server does not hold the others. Raise the budget for a
slow server:

```bash
export NIKI_MCP_STARTUP_TIMEOUT_SECS=30
```

A failed server is a warning, not a failed start: the session keeps the tools of
every server that answered. Later requests use a fixed one-minute budget; a
per-server `timeout` key is not implemented. A message such as
``mcp server `name`: initialize cannot run: the connection is closed (…)`` means
an earlier failure closed a stdio connection, so restart the session.

`type = "sse"` is rejected at load:

```text
mcp server `docs` in config.toml: unsupported transport `sse`; this build speaks `stdio` and `http`
```

Use `type = "http"` (streamable HTTP) or the default inference from `url`.

## A skill was skipped

```text
niki: warning: /repo/.niki/skills/release-notes/SKILL.md: the front matter has no description
```

Discovery never fails the session; it skips the file and keeps the rest. Check:

- The file starts with `---` and closes the block with `---`.
- `description` is present and not empty.
- Every front-matter line outside a nested block is `key: value`.

Run `/skills` to list what loaded and what was skipped.

## A hook timed out or was ignored

A hook is a `/bin/sh -c` child with a timeout: 600 seconds by default, 30
seconds for `UserPromptSubmit`. A command that cannot start, times out, exits
non-zero for a reason other than `2`, or writes output the runner cannot read is
a warning, and the turn continues.

- Give a slow handler its own `timeout` in `hooks.json`.
- Exit `2` to block, with the reason on stderr.
- An unsupported event name, a `permissionDecision` other than `deny`, or
  `async: true` produces a warning; see [Hooks](/extensions/hooks).

`.niki/hooks.json` from a repository is loaded without a trust prompt in this
build, so review it before you run Niki in an unfamiliar repository.

## `--acp` and `-n` cannot be combined

They are two different entry points: `-n` runs one prompt and exits, `--acp`
serves an editor until stdin closes. Pick one. The exit status is 2.

```text
niki: --acp and -n cannot be combined; --acp serves an editor on stdin/stdout and -n runs one prompt
```

The ACP server writes protocol frames to stdout, so a client that does not speak
the protocol sees JSON, and warnings appear on stderr.

## `no saved session` and unsaved sessions

```text
niki: no saved session "<id>" for <workspace>; run without --resume to start one
```

`--resume` names a session that is not in `sessions.db`, or `--continue` found
no saved session for the current directory. Run without either to start a new
one, or open the app and use `/resume` to list what exists.

```text
niki: warning: sessions are not saved: …
```

`sessions.db` could not be opened, so this turn runs but is not written down.
The warning includes the reason — usually a permissions problem under `~/.niki`
or a `NIKI_HOME` that points at a read-only path. The session still works; it
just cannot be resumed.

## The turn stops early

```text
niki: step limit of 40 reached
```

One run may make 40 model turns by default. Raise it with `--max-steps <N>`. The
exit status is 124 when the budget runs out.

```text
niki: cancelled
```

The turn was cancelled with <kbd>Ctrl</kbd>+<kbd>C</kbd>. A headless run exits
`130` in that case. The transcript keeps what the turn already produced.

## A tool call is refused

A refusal is a tool result the model can read, not a crash. The common causes:

- A shell command is not on the allow-list and the mode is Manual or Auto; add
  it with `--allow`, approve the prompt, or use YOLO.
- A gated MCP tool has no approver because the run is headless.
- A path resolves outside the workspace root.

See [Permissions](/permissions) for the exact gate.

## Nothing here matches

Report a vulnerability privately through [Security](/security). For anything
else, include `niki --version`, your platform, the command you ran, and the
smallest reproduction you have.
