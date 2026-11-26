# Independence from the upstream monorepo

Niki Agent was forked from the Deep Agents monorepo. As of 1.0.0 it is a
standalone distribution with its own package name, environment namespace,
profile directory, console script, and release line. Nothing at runtime imports
an upstream distribution.

This page records the rename, the compatibility aliases, and the one-time
migrations a user may see. It is the reference for "why does this old name
still work?" and "what moved?".

## Rename table

| Surface | Before | 1.0.0 onward |
| --- | --- | --- |
| Python package | `deepagents_code` | `niki_agent` |
| Distribution | `deepagents-code` | `niki-agent` |
| Primary command | `dcode` | `niki` |
| Deprecated commands | — | `dcode`, `deepagents-code` |
| Environment prefix | `DEEPAGENTS_CODE_*` | `NIKI_*` |
| Profile directory | `~/.deepagents` | `~/.niki` |
| Profile env var | `DEEPAGENTS_HOME` | `NIKI_HOME` |
| Home error | `DeepAgentsHomeError` | `NikiHomeError` |
| SDK import | `deepagents.*` | `niki_agent.sdk.*` |
| ACP import | `deepagents_acp.*` | `niki_agent.acp_sdk.*` |

`dcode` and `deepagents-code` remain registered console scripts for two minor
versions. They print a one-line deprecation notice and then run the same app.

## Environment variables

Every `DEEPAGENTS_CODE_*` variable is still read once, under its `NIKI_*` name.

```
DEEPAGENTS_CODE_DEBUG       → NIKI_DEBUG
DEEPAGENTS_CODE_LOG_LEVEL   → NIKI_LOG_LEVEL
DEEPAGENTS_CODE_HOME        → NIKI_HOME
DEEPAGENTS_CODE_INVOKED_AS  → NIKI_INVOKED_AS
```

At import, `niki_agent._env_vars` copies any legacy value to its `NIKI_*` name
when the new name is unset, and removes the legacy key from `os.environ`. One
`DeprecationWarning` names the migration. Code that reads the environment
through `_env_vars` or `model_config.resolve_env_var()` therefore sees the new
name even when the shell still exports the old one.

Provider API keys are untouched: `GROQ_API_KEY`, `OPENAI_API_KEY`, and the rest
keep their vendor names.

## Profile directory migration

On the first launch that resolves the default profile, `_paths` moves
`~/.deepagents` to `~/.niki` when all of the following hold:

- `NIKI_HOME` and `DEEPAGENTS_HOME` are both unset, so the default is in play.
- `~/.niki` does not exist yet.
- `~/.deepagents` exists.

After a successful move, a `.migrated-from-deepagents` marker is written inside
the new directory so a later launch does not try again. If the move fails, the
old directory is left in place, a warning is printed, and the app continues with
the old path.

Project-level configuration keeps reading `.deepagents/` when a project has one.
New projects should use `.niki/`.

## Vendored SDK and ACP

`niki_agent/sdk` and `niki_agent/acp_sdk` are copies of the pinned upstream
packages. Each directory carries a `PROVENANCE.md` naming the source project,
version, commit, license, and the mechanical changes applied. The copies are
not edited for branding; MIT requires the original notices to survive, and a
byte-comparable copy is easier to audit.

Upstream's MIT license and copyright notice remain in the repository root
`LICENSE` and `NOTICE`.

## What "independent" does not mean

- The agent loop, middleware, and prompt behavior are the upstream design. The
  fork does not claim a different engine.
- The Deep Agents SDK is credited in `NOTICE`, the vendored `PROVENANCE.md`
  files, and the in-app About view. Product chrome, `--version`, and the help
  screen say Niki Agent.
- The Rust client (phase 3 of the independence plan) talks to this Python engine
  over the existing protocol. It does not reimplement the agent loop.

## Compatibility window

The deprecated aliases exist for two minor versions after 1.0.0. Removing them
is a breaking change and needs a major release. The brand gate
(`tests/unit_tests/niki/test_brand_gate.py`) allows the old names only in the
alias implementation, the migration constants, and attribution files.
