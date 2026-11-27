# Getting started

Niki Agent is a coding agent you run in your terminal. You say what you want; it
reads your repository, edits files, runs commands, and shows you each step.

- **One binary.** The product is a Rust executable named `niki`; there is no
  interpreter, package manager, or virtual environment to install beside it.
- **One state directory.** Configuration, skills, hooks, and the session store
  live under `~/.niki`, or under `NIKI_HOME` when you set it. A repository can
  carry its own `.niki/` for project-scoped skills and hooks.
- **Your model.** Eleven providers have a built-in endpoint and key variable,
  and any OpenAI-compatible gateway is a configuration block.

## Requirements

| | |
| --- | --- |
| Platform | Linux x86_64, macOS arm64 and macOS x86_64 have prebuilt release archives. Windows has a `.zip` archive that has not been exercised on Windows, so treat it as untested. |
| Rust (source build only) | Rust 1.85 or newer, for the 2024 edition. A release archive needs no Rust. |
| Terminal | Any terminal that runs the full-screen app. `niki -n` also runs without a TTY. |
| A model provider | A default model and a provider key are needed before the first turn. |

## Next

- [Install](/getting-started/install) — the release archive on Linux, macOS
  and Windows, or a source build for development.
- [Your first turn](/getting-started/first-turn) — the flags, the exit codes,
  the keys, and the headless mode.

Once it is installed, the two pages most people read next are
[Providers](/providers/) and [Permissions](/permissions).
