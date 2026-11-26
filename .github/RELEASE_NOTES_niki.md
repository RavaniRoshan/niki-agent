# Niki Agent

**A terminal coding agent that stays quiet, starts fast, and runs on your
machine.**

1.1.2 is the release that makes the installed product complete: the wheel now
carries everything a turn needs, the project directory is `.niki`, and every
page a new user reads describes the product rather than how it was built.

## Install

**Linux / macOS**

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
```

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
```

Both create an isolated environment under `~/.local/niki`
(`%LOCALAPPDATA%\niki` on Windows), verify the release checksum, and leave your
shell profile alone unless you pass `NIKI_SHELL=1`. Requires Python 3.12+.

Then:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
niki
```

No key is needed to look around — `niki` boots to a working composer.

## What is fixed in 1.1.2

**The first turn works.** The shipped wheel used to install, answer
`niki --version`, and then fail when you asked it to do something: the JS
interpreter it uses for code execution was pulled from a package whose published
release did not match the interface the agent calls. The interpreter now ships
inside this wheel, so an installed release does a complete turn — prompt, tool
call, file written — with no missing pieces. The release pipeline now proves
that with a real turn before publishing, not with a version print.

**The project directory is `.niki`.** Configuration, sessions, history, skills,
and stored keys live under `~/.niki`; a repository-level `.niki/` holds
project-scoped settings. A `.deepagents/` directory is still read when it is the
only one present, and never written to.

**The documentation is the product's.** The README, the provider guide, the
architecture note, the independence page, the security policy, and the issue and
pull-request templates were rewritten for the person using the agent. Two real
documentation bugs are gone with them: the provider-table example used a TOML
table the config reader ignores, and the command palette was advertised on a key
that has no binding.

## What makes it different

**It is quiet.** Zero idle redraws. The composer is a single rule, not a box. A
long transcript is mounted in a bounded window, so hour three of a session
renders like minute one.

**It does not waste your first minute.** `niki --version` answers without
importing the agent runtime, and the TUI paints before model setup finishes.

**You choose the model.** Anthropic, OpenAI, and Google Gemini work immediately;
eighteen more provider integrations are one `/install` away; any
OpenAI-compatible endpoint is a config block in `~/.niki/config.toml`.

**It asks before it acts.** Shift+Tab cycles Manual → Auto → YOLO. Auto sends
anything uncertain to a classifier and still asks when review fails. A shell
allow-list and sandbox backends give you a hard boundary when you need one.

**Accessibility is built in, not bolted on.** Four themes with measured
contrast — body text at 4.5:1, UI glyphs at 3:1. Reduced motion, an ASCII glyph
set, a screen-reader mode, and layouts checked at 40, 50, 80, 120, and 160
columns.

**Your data stays put.** Sessions, history, skills, and stored keys live in
`~/.niki`. No account and no cloud sync. The only network calls outside a turn
are the update check, the optional self-update, and the model-pricing refresh,
and each is described in `SECURITY.md`.

## Artifacts

| File | Platform |
| --- | --- |
| `niki-<version>-linux.tar.gz` | Linux x86-64 and WSL |
| `niki-<version>-macos.tar.gz` | macOS, Apple Silicon and Intel |
| `niki-<version>-windows.zip` | Windows x86-64 |

`SHA256SUMS` covers all three. Each archive contains the pinned wheel, both
installers, `NOTICE`, and `README.md`. The macOS archive is
architecture-independent because the wheel is pure Python.

To verify by hand:

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

## Upgrading

Re-run the install command. The installer replaces the environment under
`~/.local/niki` and leaves `~/.niki` — your configuration, sessions, and skills —
untouched.

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
