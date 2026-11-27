---
title: Beta
---

# Beta builds

Niki is usable today and still moving. A beta build is a tagged GitHub
pre-release: it passed the same Rust test gate as a stable release, but a
behaviour may change again before the next stable tag. Betas are for people who
want the newest fixes and are willing to report what breaks.

Stable releases are always available; you do not have to run a beta to use Niki.

## Supported platforms

| Platform | Artifact | How it is checked |
| --- | --- | --- |
| Linux x86_64 | `niki-<version>-linux-x86_64.tar.gz` | Built and run in CI: installer, one headless turn, `--version` |
| WSL | The Linux archive | Same archive; not exercised on WSL in CI |
| macOS Apple silicon | `niki-<version>-macos-aarch64.tar.gz` | Built and run in CI: installer and one headless turn |
| macOS Intel | `niki-<version>-macos-x86_64.tar.gz` | Built in CI; not executed on Intel hardware |
| Windows x86_64 | `niki-<version>-windows-x86_64.zip` | Built in CI; not executed on Windows in CI |

The macOS binaries are not notarized. A file fetched with `curl` carries no
quarantine attribute; a browser download may need the Gatekeeper prompt or
`xattr -d com.apple.quarantine`.

## Install a beta

Pin the exact pre-release with `NIKI_VERSION` (no leading `v`):

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh \
  | NIKI_VERSION=2.2.0-beta.1 bash
```

On Windows, pass the same value to the PowerShell installer:

```powershell
$env:NIKI_VERSION = "2.2.0-beta.1"
irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
```

The installer verifies the archive against that release's `SHA256SUMS` before
placing it under `~/.local/niki` (`%LOCALAPPDATA%\niki` on Windows). Your
configuration, sessions, and skills in `~/.niki` are untouched.

To go back to the newest stable release, install without `NIKI_VERSION`:

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
```

A downgrade keeps `~/.niki`, so a session written by a newer build remains on
disk. If a beta changes the session schema, the older build will say so rather
than corrupt it; keep a copy of `~/.niki` if you need to move between builds
often.

## What to try

- Install, run `niki`, and complete one turn with your own provider key.
- Ask it to edit two files and run a command; answer the approval card with
  `y` (once), `s` (session), and `d` (refuse) at different moments.
- Run `niki doctor` and `niki --version`; both should agree with what you
  installed.
- Resume a session with `/resume`, fork it with `niki sessions fork <id>`, and
  archive it with `niki sessions archive <id>`.
- Try a headless turn: `niki -n "summarize this repo" --output-format jsonl`.
- On a repository with `.niki/hooks.json` or `.niki/skills/`, confirm that the
  content loads only after `niki trust`.

## Known gaps

These are understood and not yet in a release:

- The app does not ask "do you trust this workspace?"; trust is the `niki trust`
  command today.
- No `/fork` or `/archive` slash commands yet, and the session picker is not
  filterable. The behaviour ships in `niki sessions`.
- No `/ps` or `/stop` for background shell work started by a turn.
- No `/recap` summary when a session is resumed cold.
- Permissions are one axis: "what may run" and "what may write" are not
  separate, so a read-only exploration mode is not expressible.
- `request_user_input` returns one choice; a multi-select form is not
  expressible.
- There is no sandbox and no classifier. Approval modes decide what asks; they
  do not constrain what an approved command can do.

The public [roadmap issue](https://github.com/RavaniRoshan/niki-agent/issues/114)
tracks the same list and moves items to "Shipped" as they land.

## Report a problem

Use the [beta feedback form](https://github.com/RavaniRoshan/niki-agent/issues/new?template=beta-feedback.yml).
Include the beta version (`niki --version`), your platform, the approval mode
you were in, and the smallest sequence that reproduces the problem. For
anything with a security impact, use the
[private advisory form](https://github.com/RavaniRoshan/niki-agent/security/advisories/new)
instead.

Niki sends no telemetry. The only network requests are the ones your configured
provider and MCP servers make.
