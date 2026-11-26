# Niki Agent

A calm, fast, keyboard-native terminal coding agent — an independent
implementation. The agent loop lives in this repository: the agent SDK and the
ACP runtime are vendored under `libs/code/niki_agent/` with their MIT notices, a
`LICENSE` and a `PROVENANCE.md` beside each, so nothing imports an upstream
distribution at runtime.

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
(`%LOCALAPPDATA%\niki` on Windows) and never touch your shell profile unless you
ask with `NIKI_SHELL=1`. Requires Python 3.12+.

Then set a provider key and run it:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
niki
```

No key is needed to look around: `niki` boots to a working composer.

## What is in this release

- **Independent product.** `niki_agent` / `niki-agent`, `NIKI_*` environment
  variables, state in `~/.niki` with a one-time migration from the old
  `~/.deepagents` profile. `dcode` and `deepagents-code` still launch it for two
  minor versions and print one line saying so.
- **Accessibility.** Contrast-verified theme variants, `NIKI_REDUCED_MOTION`,
  a screen-reader mode (one turn per invocation, no escape sequences), an ASCII
  glyph registry, and layout tiers checked at 40/50/80/120/160 columns.
- **Interface parity.** `docs/niki/UI_PARITY.md` carries no MISSING P0 row; the
  last gap, in-session `#` memory notes, landed here.
- **Rust client, source only.** `rust/` speaks the same engine HTTP/SSE
  protocol as the Python client: deterministic protocol replay, a live session,
  and pinned parity frames (`docs/niki/RUST_PARITY.md`). It is not shipped as a
  binary in this release and does not yet replace the Python UI.
- **Documentation and branding.** User-facing documentation names only Niki
  Agent. Fork-era records — the upstream README, the diff log, the design audit,
  and the pre-change baselines — moved to `docs/niki/history/` with a framing
  note. `NOTICE` now describes the product accurately as an independent
  implementation with two vendored MIT components, and each vendored directory
  carries its own `LICENSE`.

## Honest status

**57 WORKS · 0 PARTIAL · 0 MISSING** across the 57 checklist rows in
[`docs/niki/CHECKLIST.md`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/docs/niki/CHECKLIST.md),
each decided by a test, a PTY run, or a measured probe.

Where the rows carry a caveat, the caveat is written into the row. Two are worth
repeating here:

- First paint is ~190 ms in process and 505–517 ms for a real process. The
  original 400 ms budget was below the CPython process floor for this import
  graph, so the guard is 1,500 ms; the reasoning is recorded rather than the
  number quietly raised.
- The 4,546.8 ms heavy-import chain is real but off the critical path, because
  the startup path defers those imports.

## Artifacts

| File | Platform |
| --- | --- |
| `niki-<version>-linux.tar.gz` | Linux x86-64 and WSL |
| `niki-<version>-macos.tar.gz` | macOS, Apple Silicon and Intel |
| `niki-<version>-windows.zip` | Windows x86-64 |

`SHA256SUMS` covers all three. Each archive contains the pinned wheel, both
installers, `NOTICE`, and `README.md`. The macOS archive is
architecture-independent because the wheel is pure Python and the installer
resolves the rest at install time.

## Attribution

Includes MIT-licensed components developed by LangChain, Inc. Their copyright
and notices are retained verbatim — see
[`NOTICE`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/NOTICE)
and the vendored `LICENSE` and `PROVENANCE.md` files.
