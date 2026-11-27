# Niki Agent

**One binary. No runtime. A terminal coding agent that starts instantly.**

2.1.2 adds the three-answer approval prompt, `niki doctor`, native macOS
archives for Apple silicon and Intel, and memory bounds on the tools that read
or capture output. The shipped product is still a single self-contained
executable: there is no Python interpreter to install, no virtual environment to
create, and no packages to resolve on first run. Linux and WSL share one
archive, Windows has its own, and macOS has one for Apple silicon and one for
Intel.

## Install

**Linux, macOS, and WSL**

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
```

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
```

Both verify the archive against the release's `SHA256SUMS`, install one
executable under `~/.local/niki` (`%LOCALAPPDATA%\niki` on Windows), and leave
your shell profile alone unless you pass `NIKI_SHELL=1`. On macOS the installer
picks `macos-aarch64` on Apple silicon and `macos-x86_64` on Intel.

Then:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
niki
```

No key is needed to look around — `niki` boots to a working composer.

The full guides are at **<https://niki-docs.vercel.app>**.

## What changed in 2.1.2

**A prompt now offers three answers.** `y` or `a` allows the call once, `s`
allows it for the rest of the session, and `d`, `n`, or `Esc` refuses. The body
names the exact command, or the absolute path a write would touch and whether it
resolves inside the workspace. A session rule for a shell program never covers a
command that chains, redirects, or substitutes another command: approving
`git status` cannot later run `git status; rm -rf ~`. A write rule re-checks the
workspace boundary on every call. `/clear` and a new session drop every rule, and
nothing is written to `config.toml`.

**`niki doctor` tells you what is broken.** One command, nine checks: version,
config file, resolved model, provider credential (set or unset, never the
value), session store, instruction files, skills, MCP servers and hooks. It
exits 1 when a turn cannot run and names the environment variable to set.

**Native macOS archives.** `niki-<version>-macos-aarch64.tar.gz` and
`niki-<version>-macos-x86_64.tar.gz` join the Linux and Windows archives. The
installer picks one by `uname -m`, the release workflow runs the installer and a
headless smoke turn on macOS arm64, and `SHA256SUMS` covers all four archives.
The macOS binaries are not notarized: a file fetched with `curl` carries no
quarantine attribute, while a browser download may need the Gatekeeper prompt or
`xattr -d com.apple.quarantine`.

**Output and reads are bounded.** A shell command can no longer exhaust memory:
each stream keeps its first 64 KiB and the header says the output was truncated.
`grep` skips files above 32 MiB and `edit_file` refuses them with the size and
the limit named, instead of loading a database or a core dump into memory.

## What changed in 2.1.1

**The whole conversation, when you need it.** `Ctrl+T` opens the transcript
overlay: every prompt, answer, tool call and result, scrollable with
`Up`/`Down`, `PageUp`/`PageDown` and `Home`/`End`, and closed with `Esc`. The
composer is untouched while it is open.

**A transient failure no longer ends the turn.** A provider request that is
rate-limited, returns 500/502/503/504, or loses the connection is retried twice
(three attempts total) with a short exponential backoff and jitter, honouring a
small `Retry-After`. Once the first token has arrived the stream is never
retried, so text cannot be duplicated. Authentication and validation errors are
not retried.

## What changed in 2.1.0

**The composer keeps up with the turn.** Press Enter while a turn is running and
the message is queued — a dim `queued:` row and `queued N` in the status bar —
then submitted when the turn ends. Alt+Enter or Ctrl+Enter steers the running
turn instead. Esc-Esc, twice within two seconds on an empty composer, restores
the last message and truncates the whole turn from the view and the history. A
line beginning with `!` runs locally through the same allow-list, costs no
tokens, and is not written to the session.

**The working tree is visible from the composer.** `/diff [staged|unstaged|untracked]`
runs `git` directly — no shell — and renders the result inside the transcript,
capped at 20 files and 400 lines with the withheld amount reported. Untracked
files are shown as additions; an empty tree says `no changes`.

**Sessions are inspectable.** `/threads` browses saved sessions with a first-prompt
preview and a filter, and restores the chosen one. `/rename` titles a session,
`/export` writes it out as Markdown, `/context` shows what the window is
holding, and `/compact` actually rewrites the running history.

**A finished turn can call a program.** `[notify] program` in the config runs
one detached executable per finished turn, with a JSON payload on stdin
(session, model, cwd, reason, steps, tool calls, and the last answer clipped to
2000 characters). It never runs through a shell and the turn does not wait for
it.

**Instructions are files.** `AGENTS.md` and `.niki/AGENTS.md` are read from the
repository root down to the working directory, with the user-level file first,
under a 32 KiB budget that truncates by line and says so. Symbolic links that
leave the workspace are skipped. `/init` writes a starter file only when one is
missing.

**Every built-in write asks.** `write_file` and `edit_file` are now gated like a
shell command: Manual asks, Auto asks unless the target resolves inside the
workspace, YOLO runs. Shell commands keep the allow-list rule, and a chained
command is no longer allowed by a prefix match.

**Modes are a command too.** `/manual`, `/auto`, and `/yolo` switch modes
mid-session — the same three states Shift+Tab cycles.

**Headless output can be parsed.** `--output-format jsonl` (also `--output`)
streams `start`, `text`, `tool_call`, `tool_result`, `usage`, and exactly one
`end` event per turn. Tool arguments that failed to parse are passed through as
raw text; tool results are truncated at 4 KiB with `"truncated": true`.

## What changed in 2.0.0

**The product is a Rust binary.** The turn loop, provider clients, tool
execution, permission policy, session store, MCP client, and terminal UI are one
workspace compiled into `niki`. Startup no longer imports a language runtime,
and an idle session does not redraw.

**Every surface is in the binary.** The interactive TUI, headless `-n` runs,
JSON output, editor integration over the Agent Client Protocol, skills, hooks,
slash commands, and session resume all ship in the same executable.

**Installation is one file.** The installer downloads a platform archive,
checks its SHA-256 against the release, and copies the binary into place.
There is nothing to compile and nothing to resolve.

**The Python client is retired.** `libs/code` stays in the repository as the
reference implementation and the source of the behavioural fixtures, but it is
no longer installed, no longer shipped, and no longer the supported product.
If you installed 1.1.x, re-run the install command: the installer replaces the
application under `~/.local/niki`. Your configuration, sessions, and skills in
`~/.niki` are untouched.

## What makes it different

**It is lean.** The composer is a single rule, not a box, and the whole client
is one native executable with no package graph behind it. The screen is drawn
by a key-driven event loop and its layout is exercised in CI against a ratatui
test backend, so a regression fails the build rather than shipping.

**It does not waste your first minute.** There is no interpreter to boot and no
import graph to warm. `niki` is a native executable.

**You choose the model.** Eleven providers have a built-in block — Anthropic,
OpenAI, Google Gemini, Ollama, OpenRouter, NVIDIA, Groq, Together, DeepSeek,
Mistral, and xAI — and any OpenAI-compatible endpoint is a config block in
`~/.niki/config.toml`. MCP servers over stdio or HTTP extend the tool set.

**It asks before it acts.** Shift+Tab cycles Manual → Auto → YOLO. There is no
classifier in this build: Manual asks for every gated command, Auto runs what
the allow-list matches and asks for the rest, and YOLO runs everything. A
workspace path boundary keeps every tool inside the root.

**Accessibility is built in, not bolted on.** Four themes with measured
contrast — body text at 4.5:1, UI glyphs at 3:1, gated in the tests. Reduced
motion and an ASCII glyph set are one environment variable each.

**Your data stays put.** Sessions, history, skills, and stored keys live in
`~/.niki`. No account and no cloud sync.

## Artifacts

| File | Platform |
| --- | --- |
| `niki-<version>-linux-x86_64.tar.gz` | Linux x86-64 and WSL |
| `niki-<version>-macos-aarch64.tar.gz` | macOS on Apple silicon |
| `niki-<version>-macos-x86_64.tar.gz` | macOS on Intel |
| `niki-<version>-windows-x86_64.zip` | Windows x86-64 |

`SHA256SUMS` covers every archive. Each archive contains the binary, both
installers, `NOTICE`, `LICENSE`, and `README.md`.

To verify by hand:

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

## Upgrading from 1.x

Re-run the install command. The installer replaces the application under
`~/.local/niki` (`%LOCALAPPDATA%\niki` on Windows) and leaves `~/.niki` — your
configuration, sessions, and skills — untouched. The old Python environment is
no longer used and can be removed.

## Uninstalling

```bash
rm -rf ~/.local/niki          # the application
rm -rf ~/.niki                # configuration, sessions, and skills
```

Remove the `PATH` line from your shell profile if you added one.

## Attribution

Niki Agent is an independent implementation. It includes MIT-licensed components;
their copyright notices and licences are retained verbatim in
[`NOTICE`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/NOTICE) and
in the `LICENSE` and `PROVENANCE.md` files beside each included directory.
