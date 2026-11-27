# Changelog

All notable changes to Niki Agent are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions use
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The product is a single Rust binary. `2.0.0` is the release where the shipped
client became that binary; `1.x` was the earlier Python implementation.

## [Unreleased]

## [2.1.4] - 2026-11-27

### Added

- A changelog, both in the repository and on the documentation site.
- A beta page and a beta feedback issue form; Discussions is enabled for
  questions.
- Pre-release tags: a version containing `-` is published as a GitHub
  pre-release, so `latest` keeps pointing at the newest stable tag.

### Changed

- The README was rebuilt around install, proof, and the approval gate, and its
  demo recording was replaced with a session recorded from the shipped binary.
  The archive now also carries `CHANGELOG.md`.
- The binary behaves as it did in 2.1.3; the only change is the version string.

## [2.1.3] - 2026-11-27

### Added

- Workspace trust: project hooks, project skills, and project instruction files
  load only after `niki trust` records the canonical workspace path.
  `niki untrust` withdraws the grant and `niki trust --list` shows it.
- `niki sessions fork <id>` copies a session's messages under a new id, and
  `niki sessions archive <id>` hides one from the default lists (`--undo`
  restores it).
- `request_user_input`: the model can ask a question with two to four options
  and an optional free-text answer, answered inside the app.
- MCP configuration expands `${VAR}` from the environment, treats `$$` as a
  literal `$`, and accepts a per-server `timeout` in whole seconds.

### Fixed

- The `Ctrl+T` help entry now describes the transcript overlay it actually
  opens.

## [2.1.2] - 2026-11-27

### Added

- `niki doctor`: nine checks covering version, config, resolved model, provider
  credential (presence, never the value), session store, instruction files,
  skills, MCP servers, and hooks; exits non-zero when a turn cannot run.
- Native macOS archives for Apple silicon and Intel, with the installer
  choosing by `uname -m`.
- `/status`: a read-only card with the model, approval mode, session, workspace,
  context size, loaded skills and MCP tools, and client version.
- `niki sessions list [--json] [--all] [--root <dir>]`.
- `Ctrl+G` hands the composer to `$VISUAL`, `$EDITOR`, or `vi`.

### Changed

- A prompt offers three answers: allow once (`y`/`a`), allow for the session
  (`s`), or refuse (`d`/`n`/`Esc`). A session shell rule never covers a command
  that chains, redirects, or substitutes another command.

### Fixed

- Shell output keeps its first 64 KiB instead of growing without bound; `grep`
  skips files above 32 MiB and `edit_file` refuses them, naming the limit.

## [2.1.1] - 2026-11-27

### Added

- `Ctrl+T` opens the transcript overlay: every prompt, answer, tool call, and
  result, scrollable and closed with `Esc`.

### Fixed

- A rate-limited, 5xx, or dropped provider request is retried twice with
  exponential backoff and jitter, honouring a small `Retry-After`. A stream that
  has produced its first token is never retried, so text cannot duplicate.
  Authentication and validation errors are not retried.

## [2.1.0] - 2026-11-27

### Added

- The composer queues a message sent during a turn (Enter), steers the running
  turn (Alt+Enter or Ctrl+Enter), and takes the last turn back out of the view
  and history (Esc-Esc on an empty composer).
- `!` runs a shell line locally through the same allow-list, costs no tokens,
  and is not written to the session.
- `/diff [staged|unstaged|untracked]` renders the working tree in the transcript,
  capped at 20 files and 400 lines with the withheld amount reported.
- `/threads`, `/rename`, `/export`, `/context`, and a `/compact` that rewrites
  the running history.
- `[notify] program` runs one detached executable per finished turn with a JSON
  payload on stdin.
- `AGENTS.md` and `.niki/AGENTS.md` are read from the repository root down to the
  working directory, under a 32 KiB line-aware budget; `/init` writes a starter
  file when one is missing.
- `--output-format jsonl` streams `start`, `text`, `tool_call`, `tool_result`,
  `usage`, and exactly one `end` event per turn.

### Changed

- `write_file` and `edit_file` are gated like a shell command.
- `/manual`, `/auto`, and `/yolo` switch approval modes mid-session.
- A chained shell command is no longer allowed by a prefix match.

## [2.0.0] - 2026-11-27

### Changed

- The product is the Rust workspace under `rust/`: turn loop, provider clients,
  tools, policy, session store, MCP client, ACP server, and terminal UI compile
  into one `niki` executable.
- Installation is one archive and one executable; the installer verifies
  `SHA256SUMS` before placing it under `~/.local/niki`.
- The Python client in `libs/code` is retired as the product: it remains as the
  reference implementation and fixture source, and installs no console script.

### Removed

- The Python runtime, virtual environment, and package resolution from the
  install path. Existing `~/.niki` configuration, sessions, and skills are kept.

[Unreleased]: https://github.com/RavaniRoshan/niki-agent/compare/v2.1.4...HEAD
[2.1.4]: https://github.com/RavaniRoshan/niki-agent/compare/v2.1.3...v2.1.4
[2.1.3]: https://github.com/RavaniRoshan/niki-agent/compare/v2.1.2...v2.1.3
[2.1.2]: https://github.com/RavaniRoshan/niki-agent/compare/v2.1.1...v2.1.2
[2.1.1]: https://github.com/RavaniRoshan/niki-agent/compare/v2.1.0...v2.1.1
[2.1.0]: https://github.com/RavaniRoshan/niki-agent/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/RavaniRoshan/niki-agent/releases/tag/v2.0.0
