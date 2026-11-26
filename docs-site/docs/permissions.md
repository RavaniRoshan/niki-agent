# Permissions and sandboxing

Niki runs shell commands and writes files with your user's permissions. The
approval mode decides how much of that it may do without asking; the sandbox and
the shell allow-list add hard boundaries when a prompt is not enough.

::: warning
Auto and YOLO are not sandboxes. The agent runs on this machine with your user's
permissions. Use `--sandbox`, a narrow shell allow-list, or a container when you
need a hard boundary.
:::

## The three modes

<kbd>Shift</kbd>+<kbd>Tab</kbd> cycles **Manual → Auto → YOLO**, and the status
bar names the active mode. `/manual`, `/auto`, and `/yolo` switch directly.

| Mode | What runs without asking first |
| --- | --- |
| Manual | Nothing. Shell commands, file writes and edits, deletes, web search, URL fetches, and task delegation all ask. |
| Auto | Routine gated actions run; anything uncertain is reviewed by a classifier model and still asks when the review fails or is unavailable. |
| YOLO | Everything the policy allows runs unsupervised. The first switch asks for a one-time acknowledgement. |

Manual is the default. In Auto, the classifier reviews an action with the main
agent model unless you name another one:

```bash
niki --auto-approve --auto-classifier-model <model>
```

`-y` is short for `--auto-approve`. `--yolo` starts in YOLO. Both apply to the
interactive UI and the ACP server only; in headless mode they are ignored with a
warning.

To keep YOLO out of the <kbd>Shift</kbd>+<kbd>Tab</kbd> cycle, set it in
`~/.niki/config.toml`:

```toml
startup.yolo_switcher = false
```

The first YOLO acknowledgement is shown once per state directory.

## Sandbox backends

`--sandbox TYPE` moves tool execution to a remote sandbox. The LangSmith sandbox
is included with the package; the other backends are optional extras that you
download first:

| Backend | Extra |
| --- | --- |
| LangSmith | included |
| Agentcore | `agentcore` |
| Daytona | `daytona` |
| Modal | `modal` |
| Runloop | `runloop` |
| Vercel | `vercel` |

Install an extra with `/install <name>` inside a session or `niki install <name>`
outside one.

| Flag | What it does |
| --- | --- |
| `--sandbox TYPE` | Remote sandbox for execution. |
| `--sandbox-id ID` | Attach to an existing sandbox instead of creating one. |
| `--sandbox-setup PATH` | Setup script to run in the sandbox after creation. |
| `--sandbox-snapshot-name NAME` | Snapshot (LangSmith) or blueprint (Runloop) name to use or create. |

The JavaScript interpreter (`js_eval`) is enabled by default when no sandbox is
in use. `--no-interpreter` turns it off; `--interpreter` turns it back on.

## The shell allow-list

`-S/--shell-allow-list` decides which shell commands the agent may run:

```bash
niki -S recommended                 # a safe set of commands
niki -n "fix the tests" -S ls,cat,grep  # an explicit list
niki -S all                         # any command
```

- A comma-separated list names the commands that are allowed.
- `recommended` selects the built-in safe set.
- `all` permits every command.

`NIKI_SHELL_ALLOW_LIST` sets the same list from the environment. Keep it narrow:
setting it to `all` disables pattern checks and is intended for throwaway
containers.

Headless runs have the shell turned off unless an allow-list is configured, so a
`-n` run that needs the shell must pass `-S`.

## Project trust

Project-scoped configuration is code from the repository, so it is gated
separately from tool approvals:

| What | Flag for a headless run |
| --- | --- |
| `.niki/hooks.json` command handlers | `--trust-project-hooks` |
| `.niki/extensions` Python | `--trust-project-extensions` |
| Project MCP configuration | `--trust-project-mcp` |

Interactive sessions ask before loading them. See [Hooks](/extensions/hooks) and
[Python extensions](/extensions/python) for how each prompt behaves.

## What to read next

- [Security](/security) — trust boundaries, reporting, and the hardening
  checklist.
- [Providers](/providers/) — credentials and configuration.
