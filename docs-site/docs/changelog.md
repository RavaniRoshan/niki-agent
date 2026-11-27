---
title: Changelog
---

# Changelog

Every release is listed here, newest first. The same history lives in
[`CHANGELOG.md`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/CHANGELOG.md)
in the repository, and each entry links to its release.

The product is a single Rust binary. `2.0.0` is the release where the shipped
client became that binary; `1.x` was the earlier Python implementation.

## 2.1.4 — 2026-11-27

**The published surface catches up with the product.**

- The README is rebuilt around install, proof, and the approval gate, with a
  recording made from the shipped binary.
- A changelog, a beta page, and a beta feedback form; Discussions is enabled for
  questions. Release archives now carry `CHANGELOG.md`.
- A hyphenated tag such as `v2.2.0-beta.1` publishes a GitHub pre-release, so
  `latest` stays on the newest stable release.
- `SECURITY.md` now describes the shipped gate — writes and edits are
  approval-gated, and Auto differs from Manual — and supported versions are
  2.1.x, with security fixes for 2.0.x.
- The binary behaves as it did in 2.1.3; the only change is the version string.

[Release notes](https://github.com/RavaniRoshan/niki-agent/releases/tag/v2.1.4)

## 2.1.3 — 2026-11-27

**Workspace trust, session lifecycle, and questions in the app.**

- Project hooks, skills, and instruction files load only after `niki trust`
  records the canonical workspace path. `niki untrust` withdraws the grant and
  `niki trust --list` shows it.
- `niki sessions fork <id>` copies a session under a new id;
  `niki sessions archive <id>` hides one from the default lists, and `--undo`
  restores it.
- `request_user_input` lets the model ask a question with two to four options
  and an optional free-text answer, answered inside the app.
- MCP configuration expands `${VAR}` from the environment, treats `$$` as a
  literal `$`, and accepts a per-server `timeout` in whole seconds.
- The `Ctrl+T` help entry now describes the transcript overlay it opens.

[Release notes](https://github.com/RavaniRoshan/niki-agent/releases/tag/v2.1.3)

## 2.1.2 — 2026-11-27

**Diagnostics, native macOS archives, and bounded output.**

- `niki doctor` runs nine checks and exits non-zero when a turn cannot run.
- Native macOS archives for Apple silicon and Intel; the installer chooses by
  `uname -m`.
- `/status` shows the model, approval mode, session, workspace, context size,
  loaded skills and MCP tools, and client version in one card.
- `niki sessions list [--json] [--all] [--root <dir>]`.
- `Ctrl+G` hands the composer to `$VISUAL`, `$EDITOR`, or `vi`.
- The approval card offers allow once (`y`/`a`), allow for the session (`s`), or
  refuse (`d`/`n`/`Esc`). A session shell rule never covers a chained,
  redirected, or substituted command.
- Shell output keeps its first 64 KiB per stream; `grep` skips files above
  32 MiB and `edit_file` refuses them with the limit named.

[Release notes](https://github.com/RavaniRoshan/niki-agent/releases/tag/v2.1.2)

## 2.1.1 — 2026-11-27

**The whole conversation, and a turn that survives a transient failure.**

- `Ctrl+T` opens the transcript overlay: every prompt, answer, tool call, and
  result, scrollable and closed with `Esc`.
- A rate-limited, 5xx, or dropped provider request is retried twice with
  exponential backoff and jitter. A stream that has produced its first token is
  never retried, so text cannot duplicate; authentication and validation errors
  are not retried.

[Release notes](https://github.com/RavaniRoshan/niki-agent/releases/tag/v2.1.1)

## 2.1.0 — 2026-11-27

**A composer that keeps up, and a working tree you can see.**

- Enter during a turn queues the next message; Alt+Enter or Ctrl+Enter steers
  the running turn; Esc-Esc takes the last turn back out of the view and
  history. `!` runs a shell line locally through the same allow-list.
- `/diff` renders staged, unstaged, and untracked changes in the transcript.
- `/threads`, `/rename`, `/export`, `/context`, and a `/compact` that rewrites
  the running history.
- `[notify] program` runs one detached executable per finished turn.
- `AGENTS.md` and `.niki/AGENTS.md` are read from the repository root down to
  the working directory, under a 32 KiB budget.
- `--output-format jsonl` streams a turn as events for your own tooling.
- Writes are gated like a shell command; `/manual`, `/auto`, and `/yolo` switch
  modes mid-session; a chained command is no longer allowed by a prefix match.

[Release notes](https://github.com/RavaniRoshan/niki-agent/releases/tag/v2.1.0)

## 2.0.0 — 2026-11-27

**The product becomes a Rust binary.**

- The turn loop, provider clients, tools, policy, session store, MCP client, ACP
  server, and terminal UI compile into one `niki` executable.
- Installation is one archive and one executable, verified against
  `SHA256SUMS` before it is placed under `~/.local/niki`.
- The Python client in `libs/code` is retired as the product and kept as the
  reference implementation. Existing `~/.niki` configuration, sessions, and
  skills are untouched.

[Release notes](https://github.com/RavaniRoshan/niki-agent/releases/tag/v2.0.0)

## 1.x

`1.x` was the Python implementation. It is no longer installed or supported.
If you still have it, re-run the install command: the installer replaces the
application under `~/.local/niki` and leaves `~/.niki` alone.
