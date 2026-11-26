# Niki Agent Development Guide

New to the package? Start with [`ARCHITECTURE.md`](./ARCHITECTURE.md) for a high-level map of how the TUI, the `langgraph dev` server subprocess, and the agent graph fit together.

## Contents

- [Quickstart](#quickstart) — get a local checkout running and run the checks CI enforces
- [LangSmith tracing projects](#langsmith-tracing-projects) — route dev traces away from the shared GA project
- [Local dev installs](#local-dev-installs) — keep an editable `niki-dev` separate from a released install
- [Debugging](#debugging) — diagnose startup crashes and client-side issues
- [Live CSS development with Textual devtools](#live-css-development-with-textual-devtools) — UI/CSS hot-reload

## Quickstart

This package uses [`uv`](https://docs.astral.sh/uv/) for environment and dependency management. Install it first if you haven't:

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

Clone the monorepo and bootstrap the `code` package. This creates the virtualenv, installs test dependencies, and installs local git hooks (`pre-commit` + `commit-msg`) so the same checks can run before you push:

```bash
git clone https://github.com/RavaniRoshan/niki-agent.git
cd niki-agent/libs/code
make bootstrap
```

If you only want to sync dependencies without installing hooks, run `uv sync --group test` instead.

Run the TUI from `libs/code` in your local checkout:

```bash
uv run niki-agent
```

`uv run` uses the project environment with the package installed editable, so source changes take effect on the next launch. If you want a persistent `niki-dev` command that stays separate from a released install, use the local dev install setup below.

### Running the tests and linters

All commands run from `libs/code`. Before opening a PR, run the full local check suite:

```bash
make check
```

This runs linting, import checks, unit tests, and lockfile/version/extras checks.

For targeted checks while iterating:

```bash
# Unit tests (no network)
make test

# A single test file
make test TEST_FILE=tests/unit_tests/test_specific.py

# Integration tests (network permitted)
make integration_test
```

```bash
# Auto-format (ruff format + autofix)
make format

# Lint + type-check (ruff, ty, commands-catalog check)
make lint
```

Run `make help` to see every available target.

## LangSmith tracing projects

When LangSmith tracing is enabled and a `LANGSMITH_API_KEY` is available, `niki-agent` traces its own agent runs to LangSmith. A key alone is not enough for environment-supplied keys — set `LANGSMITH_TRACING=true`. Keys stored via `/auth` auto- enable tracing unless you explicitly opt out (for example `NIKI_LANGSMITH_TRACING=false`).

By default those runs land in the `niki-agent` project, but the team routes shared runs to a GA project (`shared-niki-agent`) that is monitored by LangSmith Engine, which raises high-priority Slack alerts on suspected issues.

**Do not point local development at the GA project.** Active development produces failures, half-finished branches, and experiments that Engine flags as issues — noise that buries the alerts that matter for the production project and costs the team triage time. Keep dev and prod runs in separate projects so Engine alerting on the GA project stays high-signal (this also mirrors LangSmith best practice of splitting dev and prod environments into distinct projects).

Route your local runs to a dev-scoped project via `NIKI_LANGSMITH_PROJECT` (this overrides only the agent's own traces, leaving your shell's `LANGSMITH_PROJECT` for user code untouched):

```bash
# A shared dev bucket, or your own personal project — anything but the GA project
export NIKI_LANGSMITH_PROJECT=shared-niki-agent-dev
uv run niki-agent
```

Set the same override for [local dev installs](#local-dev-installs) (`niki-dev`), not only ad-hoc `uv run` sessions. You can also set it persistently under `[tracing]` in the config file (`tracing.langsmith_project`) or from the `/auth` screen. The startup splash and `/trace` show the project a run is writing to — confirm it is not the GA project before doing noisy work.

When onboarding a new tracing project to Engine, capture the nature of that project first: a production/GA app warrants high-priority alerts, while a staging or active-dev project should use looser thresholds (or stay off Engine) so it does not generate false positives.

A server keeps the first workspace's tracing configuration for its lifetime. Workspaces sharing that server must use matching tracing credentials, projects, endpoints, profile selectors, tracing flags, and redaction policy. A conflicting workspace is refused before its settings can affect cached or concurrent runs; start a separate server for that workspace. Restart the server to change its tracing configuration.

## Debugging

Niki Agent runs as two processes: the **Textual TUI** you interact with, and a **`langgraph dev` subprocess** that hosts the agent graph. Each writes its own log, and a single switch turns both on:

```bash
cd libs/code
export NIKI_DEBUG=1
uv run niki-agent
```

| Variable | Effect |
| --- | --- |
| `NIKI_DEBUG` | Master switch. Preserves the server subprocess log on exit (printing its path to stderr) and attaches the client `DEBUG` file handler. Truthy: `1`/`true`/`yes`/`on` (case-insensitive). Falsy: `0`/`false`/`no`/`off`/empty/unset. |
| `NIKI_DEBUG_DIRECTORY=<path>` | Overrides the client log directory (default `/tmp/niki_debug`). Each thread writes to `<thread-id>.log`. **Only takes effect when `NIKI_DEBUG` is truthy**; does **not** affect the server subprocess log. |
| `NIKI_DEBUG_FILE=<path>` | Deprecated compatibility override. When `NIKI_DEBUG_DIRECTORY` and `[debug].directory` are unset, per-thread logs use this path's parent directory. Existing `[debug].file` config works the same way. |

Then pick the log you need by symptom:

| Symptom | Log you want | Default location |
| --- | --- | --- |
| App crashes on launch (one-line failure banner) | **Server subprocess log** — the real traceback | `$TMPDIR/niki_server_log_*.txt` |
| App starts, then misbehaves (UI, model calls, slash commands) | **Client app log** — `niki_agent` at `DEBUG` | `/tmp/niki_debug/<thread-id>.log` |

### Startup crash -> server subprocess log

The TUI only surfaces a one-line banner; the actual exception lives in the subprocess's combined stdout/stderr. To get it:

1. **Re-run with debugging on** (see above). After the app exits and the terminal is restored, the log is preserved and its path is printed to stderr as `Server log preserved at: ...`.
2. **Open the newest log.** On macOS, `tempfile` resolves to `$TMPDIR` (a path under `/var/folders/.../T/`):

   ```bash
   # Newest first
   ls -lt ${TMPDIR:-/tmp}/niki_server_log_*.txt | head -5

   # Or tail the latest live while you reproduce the crash
   tail -F "$(ls -t ${TMPDIR:-/tmp}/niki_server_log_*.txt | head -1)"
   ```

3. **Search for `Failed to initialize server graph`.** The traceback beneath it names the concrete failure — MCP config validation, sandbox init, model resolution, subagent load, and so on. Everything above that line is uvicorn/lifespan unwinding and can be ignored.

### Client-side issue -> app log

For problems that appear after the app is up, tail the client log in another terminal while reproducing:

```bash
tail -f /tmp/niki_debug/<thread-id>.log
```

To send logs elsewhere, also `export NIKI_DEBUG_DIRECTORY=<path>`. Each thread gets its own append-only file, and changing threads in the app switches the active log file.

The directory is created or tightened to owner-only access, and each file is created or tightened to user-only access. Symlinks are refused. If either cannot be secured, no file handler is attached and a warning goes to stderr. Use the in-app Debug Console in that case.

Raw stdio MCP server stderr is discarded to protect the TUI and prevent unbounded disk growth, including in debug mode. Structured MCP log notifications still reach the application logger with the server name and severity.

### In-app Debug Console (`Ctrl+\`)

Press `Ctrl+\` (or run the hidden `/debug` command) inside a session to toggle a read-only Debug Console overlay. It shows a point-in-time session/runtime snapshot (version, model, thread, cwd, auto-approve, sandbox, MCP servers, token usage, debug-log path) plus a live tail of recent `niki_agent.*` log records.

The tail is fed by an always-on in-memory ring buffer (`_debug_buffer.install_log_buffer`, installed on the package logger in `__init__.py`), so it works **without** `NIKI_DEBUG` — though enabling that switch raises the captured level to `DEBUG` and adds the file handler above. In the console, `Ctrl+X` clears the on-screen view (the buffer keeps accruing) and `c` copies the visible lines. `Ctrl+L` remains an alias for clearing the view.

In `/btw`, `Esc` hides the modal and cancels any unfinished answer. Completed side exchanges remain available when you reopen `/btw` or switch away from the main thread and return. Each main thread has its own side history, kept only while the app runs and separate from the main conversation. `Ctrl+X` cancels the side answer and clears that thread's side transcript, follow-up context, and draft while keeping the modal open.

## Local dev installs

A *local dev install* gives you a persistent `niki-dev` command that launches your checkout directly. It lives in a dedicated editable venv under `~/.local/share/niki-dev`, symlinked into `~/.local/bin/niki-dev`. It can sit alongside a released `niki` without interfering:

- `niki` — the released tool, installed with `install/install.sh` (or the one-line installer in the README).
- `niki-dev` — your local checkout.

That lets you compare released behavior against local, and fall back to a known-good build if your checkout breaks. Either way, the dedicated venv keeps the dev binary's dependency experiments out of the repo's locked `uv sync` environment.

### Setup

`~/.local/bin` must be on your `PATH` for the symlink to resolve (`uv tool install` adds its own shim directory automatically, but a hand-rolled symlink does not). Replace `<repo>` with your local checkout path:

```bash
# 1. Create an isolated venv for the dev binary
uv venv ~/.local/share/niki-dev --python 3.13

# 2. Install your checkout into it, editable
uv pip install --python ~/.local/share/niki-dev/bin/python -e <repo>/libs/code

# 3. Expose it as `niki-dev` on your PATH
ln -sf ~/.local/share/niki-dev/bin/niki ~/.local/bin/niki-dev
```

The `--python 3.13` is illustrative — any interpreter satisfying the package's `requires-python` (currently `>=3.12`) works; omit the flag to let `uv` pick.

> **Why `uv venv` + `uv pip install -e` rather than `uv sync` or `uv tool install --editable`?** This builds an isolated venv *outside* the workspace's locked environment, so the dev binary can be re-resolved on demand without disturbing the released tool or the repo's `uv.lock`. (`uv pip` and `uv venv` are first-class `uv` subcommands here, not bare `pip`.)

### Updating

When dependency constraints change in `libs/code/pyproject.toml`, refresh the dev venv:

```bash
uv pip install --python ~/.local/share/niki-dev/bin/python -e <repo>/libs/code --upgrade
```

### Verifying

Confirm command resolution and editable imports (the `niki` checks assume the released tool is installed separately, per above):

```bash
which niki
which niki-dev
niki --version
niki-dev --version
~/.local/share/niki-dev/bin/python -c 'import niki_agent; print(niki_agent.__file__)'
```

## Live CSS development with Textual devtools

After completing the [Quickstart](#quickstart), use Textual's devtools console for CSS hot-reload and live `self.log()` output during development.

Create the dev wrapper script once:

```bash
cat > /tmp/dev_niki.py << 'PYEOF'
"""Dev wrapper to run Niki Agent with textual devtools."""
import sys
sys.argv = ["niki"] + sys.argv[1:]

from niki_agent.main import cli_main
cli_main()
PYEOF
```

Run both commands from `libs/code`:

**Terminal 1** — devtools console:

```bash
uv run --group test textual console
```

**Terminal 2** — TUI with live reload:

```bash
uv run --group test textual run --dev /tmp/dev_niki.py
```

Edit any `.tcss` file and save — changes appear immediately. Any `self.log()` calls in widget code show in the console.

### Console options

- `textual console -v` — verbose mode, shows all events (key presses, mouse, etc.)
- `textual console -x EVENT` — exclude noisy event groups
- `textual console --port 7342` — custom port (pass matching `--port` to `textual run`)

### Why the wrapper script?

`textual run --dev` handles the devtools connection, but it needs to run inside the project's virtualenv to import `niki_agent`. The wrapper script bridges the gap — `uv run --group test textual run --dev` ensures both `textual-dev` (from the `test` group) and `niki_agent` are available in the same environment.
