# Niki Agent

**One binary. No runtime. A terminal coding agent that starts instantly.**

Niki is an independent terminal coding agent written in Rust: one native
executable that runs a real agent loop, edits your files, runs commands, and puts
an approval card in front of every change. Sessions live on your disk, eleven
providers are built in, and any OpenAI-compatible endpoint works. There is no
language runtime to install, no virtual environment to create, and no packages
to resolve on first run. Linux and WSL share one archive, Windows has its own,
and macOS has one for Apple silicon and one for Intel. Each release's changes are
listed under **What changed** below.

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

## What changed in 2.3.0

**Providers are set up inside the app.** `/connect` opens a catalog: pick one of
the eleven built-in providers or name a custom endpoint, paste a key, choose a
model. The key goes to `credentials.toml` beside your configuration with
owner-only permissions, and the session switches without a restart. Eleven
providers ship built in; any other OpenAI-compatible endpoint is a config block.

**The agent can read the web, with the gate in front of it.** `web_fetch` and
`web_search` are built-in tools. Both ask before the first call, like any gated
command, and `Always` covers the rest of the session. Pages are capped at 4 MiB
and the text returned is capped so one call cannot fill the context window.

**More of the session is addressable from the keyboard.** `Ctrl+N` starts a new
session, `Ctrl+R` opens the session browser, and the command list matches
loosely — prefix, then substring, then letters in order — so `/cmpct` reaches
`/compact`. `/copy`, `/editor` and `/cost` work as commands as well as chords.

**The screen says what it is doing.** A busy row above the composer shows the
elapsed clock and the interrupt key while a turn runs; a second footer line
carries key hints before the first turn and context and token counts after it; a
finished tool call gets a `✓` or `×` and `(no output)` when it printed nothing.

**Settings that are actually settings.** `/settings` lists the `[ui]` keys —
theme, motion, welcome, progress, shimmer, title — applies a change live, and
writes it back to `config.toml`. `/mcp` lists every configured MCP server with
its transport and what became of it.

**Two mistakes fixed.** A narrow terminal cut the welcome sentence mid-word; it
now wraps at word boundaries. `--root .` printed as a literal `.`; the workspace
root is resolved at start-up.

## What changed in 2.2.0

**The first thing you see is the product.** A fresh session now opens on a
welcome block: the name and version, the one-line promise, the model, the
approval mode, and the folder you are in, followed by the four commands worth
knowing first. It yields to the transcript the moment you send a prompt.

**The repository holds one implementation.** Every workflow, label, pre-commit
hook and release step now refers only to the Rust workspace; the earlier client
tree and its tooling are gone. The gate that proves an installed archive runs a
real turn is a Rust binary in that workspace rather than a script beside it.

**A download page and a changelog page.** The marketing site now detects your
platform, offers the one-line installer, and lists every archive with its size,
its SHA-256, and the command that verifies it. The changelog page is generated
from `CHANGELOG.md` and checked in CI, so it cannot drift.

**A just-saved hook script runs.** A notify program written moments earlier
could fail to start with `ETXTBSY` and be dropped with a warning. Niki now
retries for up to a second.

Older releases are recorded in
[`CHANGELOG.md`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/CHANGELOG.md).
This page keeps the two most recent.

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

## Upgrading

Re-run the install command. The installer replaces the application under
`~/.local/niki` (`%LOCALAPPDATA%\niki` on Windows) and leaves `~/.niki` — your
configuration, sessions, and skills — untouched.

## Uninstalling

```bash
rm -rf ~/.local/niki          # the application
rm -rf ~/.niki                # configuration, sessions, and skills
```

Remove the `PATH` line from your shell profile if you added one.

## Attribution

Niki Agent is an independent implementation. The binary links third-party Rust
crates, each under its own licence, with the exact versions recorded in
`Cargo.lock`. The MIT licence at the repository root applies to Niki Agent; see
[`NOTICE`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/NOTICE).
