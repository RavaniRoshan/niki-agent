# Getting started

Niki Agent is a coding agent you run in your terminal. You say what you want; it
reads your repository, edits files, runs commands, and shows you each step.

- **One command.** The package ships a single console script, `niki`.
- **One state directory.** Configuration, sessions, history, skills, and stored
  credentials live under `~/.niki`. A repository can carry its own `.niki/` for
  project-scoped settings.
- **No account.** Nothing is required to launch it. A model provider is only
  needed when you want it to do work.

## Requirements

| | |
| --- | --- |
| Python | 3.12 or newer, on `PATH` as `python3` (Linux and macOS). The installer builds its own virtual environment, so your system Python is not modified. |
| Platform | Linux x86-64 (including WSL), macOS on Apple Silicon or Intel, Windows x86-64. |
| Terminal | Any terminal that runs the interactive session. The UI is built with [Textual](https://textual.textualize.io) and degrades to ASCII glyphs when the terminal needs it. |
| A model provider | Optional for the first launch. Anthropic, OpenAI, and Google Gemini work immediately once a key is set. |

## Next

- [Install](/getting-started/install) — the release installer on Linux, macOS,
  and Windows, or a source checkout with `uv`.
- [Your first turn](/getting-started/first-turn) — what the first launch asks,
  what a turn looks like, and the headless mode for scripts.

Once it is installed, the two pages most people read next are
[Providers](/providers/) and
[Permissions and sandboxing](/permissions).
