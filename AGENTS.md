# Niki Agent repository guide

Niki Agent is a terminal coding agent. The product is a single Python package in
`libs/code`; it vendors the Deep Agents SDK and ACP runtime inside that package,
so nothing imports an upstream distribution at runtime. `rust/` holds a second
front end for the same engine, built crate by crate; it does not replace the
Python client until its parity gates are green.

## Layout

- `libs/code` — the whole product: Textual TUI, headless mode, MCP, skills,
  slash commands, the vendored SDK (`niki_agent/sdk`), and ACP
  (`niki_agent/acp_sdk`).
- `libs/partners/quickjs` — the JS interpreter middleware the agent depends on.
  It is built from this repository because its unreleased line carries the PTC
  API the agent imports.
- `rust/` — the Rust client workspace (`niki`, `niki-proto`,
  `niki-engine-client`, `niki-render`, `niki-tui`) that speaks the same
  HTTP/SSE protocol as the Python engine. The port plan is `rust/MAP.md`.
- `install/` — the release installer for Linux/macOS and Windows.
- `docs/niki` — architecture, independence, keymap, and provider guides.
- `SECURITY.md` — reporting, supported versions, and trust boundaries.

## Commands

```bash
cd libs/code
uv sync --all-groups     # install dependencies
make check               # lint, types, imports, tests, repo gates — the local CI
make lint                # ruff + ty + command catalogue + process-cwd check
make test                # pytest
```

`make check` is authoritative. Run it before claiming a change is done.

```bash
cd rust
cargo test -j 4 --workspace   # M0 fixtures + pinned M1 replay frames
cargo fmt --all -- --check
cargo clippy -j 4 --workspace --all-targets -- -D warnings
bash tools/m2_session.sh      # M2: live PTY session vs the replay frame
```

## Development rules

- Fix what the task asks for. Do not refactor nearby code or add features.
- Read a file before changing it. Do not propose changes to unread code.
- Add tests only when they protect changed observable behavior.
- Keep user-visible language Niki Agent. Attribution belongs in `NOTICE`,
  `LICENSE`, and the vendored `PROVENANCE.md` files; the version line and help
  screen do not credit another product.
- `dcode` and `deepagents-code` are deprecated console scripts for two minor
  versions. Do not add new references to them outside the deprecation path.
- Do not edit the vendored trees for style. They are excluded from ruff and ty so
  their diff against `PROVENANCE.md` stays auditable.

## Suppressing ruff rules

Use inline `# noqa: RULE` with a justification for individual exceptions. Reserve
`[tool.ruff.lint.per-file-ignores]` for categorical policies. If you cannot
justify a suppression, the code is probably the problem.

## Branches and commits

- Branch names: `<github-username>/<scope>/<short-description>`.
- Conventional Commits, with a scope. Allowed scopes are defined in
  `.github/workflows/pr_lint.yml`: `ci`, `code`, `deps`, `docs`, `examples`,
  `infra`, `quickjs`, `repo`, `rust`, `sdk`.
- Text after `type(scope):` starts lowercase unless it begins with a proper noun
  or a code entity. Wrap code identifiers in backticks.
- Never create a `main` branch. The default branch is `niki-agent`.

## Releases

Push a `v<version>` tag or dispatch **Niki Release**. The workflow builds the
wheel on Linux and Windows, assembles three archives (Linux, macOS, Windows)
with `install/install.sh` or `install/install.ps1`, and publishes a GitHub
Release. The macOS archive is assembled on Linux: the wheel is pure Python, and
this repository has no macOS runner entitlement. See `.github/RELEASING.md`.

## CI

`.github/workflows/ci.yml` runs `make check` in `libs/code` and exposes the
aggregate check **✅ CI Success**. Archived packages have no CI.

## Scoped guide

- [`libs/code/AGENTS.md`](libs/code/AGENTS.md) — Textual patterns, startup
  performance, slash commands, providers, and the vendored SDK.
