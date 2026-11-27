# Decisions

One short ADR per decision, numbered in order, never renumbered. A changed
decision gets a new ADR that supersedes the old one.

## ADR-001: Rewrite, do not transpile

Status: accepted (2026-11-26)

Context: `rust-anything analyze libs/code` reports 41 units and **zero** Rust
crates for the runtime, the UI toolkit, or the provider integrations. A
unit-by-unit bridge would have nothing to bridge to: the Python code is glue
around three large frameworks that do not exist in Rust.

Decision: the Rust program is a new implementation of the same external
contracts — the CLI, the config file, the provider HTTP APIs, the MCP protocol,
the on-disk session layout. Equivalence is proved with recorded app cases, not
with per-function `diff`.

Consequences: no PyO3 shadow, and `rust-anything bridge` applies to the few pure
units (text, paths, parsing) rather than the app. The `app record` / `app replay`
half of the skill is the proof, and `MAP.md` carries the unit plan instead of an
analyze-generated port order.

## ADR-002: Keep the existing crate split, add the engine as new crates

Status: accepted (2026-11-26)

Context: `rust/crates/` already holds `niki-proto`, `niki-engine-client`,
`niki-render`, `niki-tui` and the `niki` binary, with committed fixtures and a
PTY replay script.

Decision: keep them. Add `niki-config`, `niki-provider`, `niki-agent`,
`niki-tools`, `niki-policy` for W1, then `niki-workspace`, `niki-session`,
`niki-tui-live` for later waves. The engine client becomes one source of
transcript events, not the only one.

Consequences: `niki-render` is shared by the replay path and the live TUI, so a
render change must keep the pinned frames green.

## ADR-003: Two provider protocols first

Status: accepted (2026-11-26)

Context: nine providers ship. Eight speak an OpenAI-compatible Chat Completions
API; Anthropic speaks its Messages API. Ollama is OpenAI-compatible.

Decision: implement OpenAI Chat Completions and Anthropic Messages as protocol
adapters, selected per provider in configuration. Everything else is
configuration. Use `reqwest` + an SSE decoder and `serde_json` for the wire.

Consequences: a provider whose API is neither can be added as one module. No
dynamic class loading: the Rust equivalent is a `Provider` trait with a registry
keyed by protocol name.

## ADR-004: TUI is ratatui, not a port of Textual widgets

Status: accepted (2026-11-26)

Context: Textual's widget tree, CSS-like styling and message bus have no Rust
counterpart. `rust/crates/niki-render` already produces a deterministic frame.

Decision: build the live TUI on `ratatui` + `crossterm`, reusing `niki-render`
for the transcript model and markdown. Layout, keymap and theme tokens are
ported as data; widget internals are written fresh.

Consequences: theme tokens and key bindings become Rust constants with tests
mirroring the Python contrast and layout tests. `niki-tui`'s keymap stays the
single source for bindings.

## ADR-005: Config compatibility, not config parity of internals

Status: accepted (2026-11-26)

Context: `~/.niki/config.toml` is the user's file. Breaking it would make the
Rust binary unusable beside the Python one.

Decision: read the same file, the same table names, the same `NIKI_*`
environment variables. Unknown keys are ignored, not an error. The Rust binary
does not write the file in W1.

Consequences: `class_path` values are read as protocol selectors (for example
`langchain_openai:ChatOpenAI` selects the OpenAI protocol). One config file
serves both binaries during the port; ADR-012 revisits it at cutover.

## ADR-006: Own session store, no cross-reader

Status: accepted (2026-11-26)

Context: the Python store is a graph-checkpointer schema. Reading it from Rust
couples the port to an implementation that is being replaced.

Decision: Rust writes `~/.niki/sessions.db` (SQLite, `rusqlite` with the bundled
feature). Python sessions are not imported in W1–W5. `niki --resume` lists Rust
sessions only, and says so.

Consequences: a migration tool is a W6 deliverable, if it is wanted at all.

## ADR-007: Equivalence bar is behavioral, not byte-identical

Status: accepted (2026-11-26)

Context: help text, spacing and progress output differ between toolkits. Pinning
bytes would force the Rust TUI to impersonate Textual.

Decision: recorded cases compare exit codes; stdout in `-n` (normalized for
timing); the final transcript text from a scripted PTY session; and the set of
files written. Help text is compared for the presence of every flag, not its
wrapping.

Consequences: replay normalizes whitespace and timing. Every normalization is
listed in `RULES.md` with its justification.

## ADR-008: Ship beside, then cut over

Status: accepted (2026-11-26); executed at v2.0.0 — the release archive ships
the Rust `niki`, and the Python client is reference-only.

Context: the Python product is released and installable today.

Decision: the Rust binary ships as `niki` from the `rust/` workspace until
cutover. The Python `niki` remains the product. Cutover requires every remaining
surface to reach the same behaviour and a release carrying both archives.

Consequences: no user is forced onto an incomplete port, and a regression in the
Rust path is answered by telling the user to run `niki`.

## ADR-009: No live model calls in tests

Status: accepted (2026-11-26)

Context: CI has no provider credentials, and a live call is neither deterministic
nor free.

Decision: providers are tested against a local HTTP server that serves recorded
SSE frames from `rust/crates/niki-provider/fixtures/`. Live smoke runs are a
manual, owner-run step recorded in `HANDOFF.md`, never in CI.

Consequences: a provider change requires a new fixture. Fixtures are captured
once and reviewed like code.

## ADR-010: `unsafe` and panics

Status: accepted (2026-11-26)

Context: gate rules for the port.

Decision: no `unsafe` outside vendored or generated code, with a `// SAFETY:`
comment when it is unavoidable. A function whose Python counterpart returns an
error returns `Result`; `unwrap`, `expect` and `panic!` are reserved for states
the Python code also treats as impossible, marked `// INFALLIBLE: <why>`.

Consequences: `cargo clippy -D warnings` plus a grep gate in CI.
