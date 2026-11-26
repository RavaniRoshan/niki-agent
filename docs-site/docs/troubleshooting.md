# Troubleshooting

Start with what Niki says about itself:

```bash
niki --version     # the version you run
niki doctor        # install diagnostics and the managed-policy report
niki config path   # the configuration file in use
```

`niki doctor` is the first stop for an install that behaves unlike a fresh one.

## A provider does not appear in `/model`

Its integration is not installed, or it is disabled.

- Install it: `/install <extra>` inside a session, or `niki install <name>`
  outside one. In a source checkout, `uv sync --extra <name>`.
- Check `~/.niki/config.toml` for `enabled = false` on that provider.

Anthropic, OpenAI, and Google Gemini are installed with the app. Every other
integration is an extra. The table of names is in [Providers](/providers/).

## `401` or `403` from a provider

- The environment variable named by `api_key_env` is not set, and `/auth` has no
  stored key for that provider.
- The endpoint expects a different wire protocol. A gateway that serves more than
  one needs one provider block per protocol, because `class_path` picks the
  protocol.

`/auth` shows which providers are configured. A key pasted there is stored in
`~/.niki/.state/auth.json` with mode `0600`; a key in the environment is never
copied into the tree.

## A model ID is rejected

Add the exact ID your gateway serves to that provider's `models` list. The
selector only offers IDs listed there.

## The first turn fails, or the app exits on launch

The terminal UI surfaces a one-line banner; the real exception is in the log of
the local server subprocess that hosts the agent graph. Turn debugging on and
reproduce:

```bash
export NIKI_DEBUG=1
niki
```

On exit, the server log is preserved and its path is printed to stderr as
`Server log preserved at: ...`. Logs are named `niki_server_log_*.txt` in
`$TMPDIR` (or `/tmp`). Search the newest one for `Failed to initialize server
graph`: the traceback beneath that line names the concrete failure — MCP config
validation, sandbox init, model resolution, subagent load, and so on. Everything
above it is server startup unwinding.

```bash
ls -lt "${TMPDIR:-/tmp}"/niki_server_log_*.txt | head -5
tail -F "$(ls -t "${TMPDIR:-/tmp}"/niki_server_log_*.txt | head -1)"
```

## The UI is up but a turn misbehaves

The client writes its own log at `debug` level once `NIKI_DEBUG=1` is set. Each
thread appends to its own file:

```bash
tail -f /tmp/niki_debug/<thread-id>.log
```

`NIKI_DEBUG_DIRECTORY=<path>` moves the client log directory. It takes effect
only when `NIKI_DEBUG` is truthy, and it does not move the server log.

For a quick look without file logging, open the in-app Debug Console with
<kbd>Ctrl</kbd>+<kbd>\\</kbd> (or the hidden `/debug` command). It shows a
session snapshot — version, model, thread, working directory, approval mode,
sandbox, MCP servers, token usage, debug-log path — plus a live tail of recent
records. In the console, <kbd>Ctrl</kbd>+<kbd>X</kbd> clears the view (the buffer
keeps accruing) and <kbd>c</kbd> copies the visible lines.

## A project's hooks or extensions are ignored

Project-scoped code is read only after you trust the project.

- Interactive launches ask before loading `.niki/hooks.json` or
  `.niki/extensions`. Cancelling the prompt aborts startup; denying skips project
  hooks for the session and continues with user hooks.
- Headless runs never prompt. Pass `--trust-project-hooks`,
  `--trust-project-extensions`, or `--trust-project-mcp` for that run.
- A newly enabled plugin's hooks take effect after `/reload`; Python extension
  backend changes need `/restart`.

## Auto mode still asks before everything

Auto runs routine actions and sends anything uncertain to a classifier model. A
review that fails or is unavailable falls back to asking. That is the intended
behaviour, not a bug — use Manual when you want every step, or YOLO when you want
none.

## A shell command is blocked

The shell allow-list decides which commands may run.

- `-S/--shell-allow-list` takes a comma-separated list, `recommended`, or `all`.
- `NIKI_SHELL_ALLOW_LIST` sets the same list from the environment.
- Headless (`-n`) runs have the shell turned off unless an allow-list is
  configured.

Setting the list to `all` disables pattern checks and is intended for throwaway
containers. See [Permissions and sandboxing](/permissions).

## `--sandbox` is rejected

The LangSmith sandbox is included with the package; Agentcore, Daytona, Modal,
Runloop, and Vercel require an extra. Install it with `/install <name>` or
`niki install <name>`.

## Text is unreadable or the terminal renders badly

| Symptom | Setting |
| --- | --- |
| Colours fight the terminal | `NO_COLOR=1 niki` |
| Glyphs render as boxes | `NIKI_UI_CHARSET_MODE=ascii niki` |
| Animation is distracting | `NIKI_REDUCED_MOTION=1 niki` |
| A screen reader is in use | `NIKI_SCREEN_READER=1 niki` |
| The default palette is too bright or too dark | `/theme` — `niki`, `niki-light`, `niki-contrast`, `niki-dim` |

## Where state lives, and how to move it

Configuration, sessions, history, skills, and stored credentials live in
`~/.niki`. `NIKI_HOME` moves the whole tree, which is also how a portable install
or a test harness works. Deleting `~/.niki` discards your sessions and keys.

## Updating and network calls

Re-run the install command to upgrade. Inside a session, `/update` checks for and
installs updates; `niki update` does the same outside the UI.

Outside a turn, the network calls are the update check, the optional
self-update, and the model-pricing refresh — plus a remote `managed_config.toml`
fetch when a deployment configures one. Each can be disabled; see
[Security](/security).

## Nothing here matches

Report a vulnerability privately through [Security](/security). For anything
else, include `niki --version`, your platform, and the smallest reproduction you
have.
