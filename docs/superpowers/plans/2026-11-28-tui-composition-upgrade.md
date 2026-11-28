# TUI composition upgrade — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Take the terminal interface from "every primitive is right" to "the composition is right": the first screen, the footer, the busy state, the tool rows, and the key map should read like a product, not a debug view.

**Architecture:** All drawing stays in `niki-tui-live` (`App` state + `ui.rs` paint) and `niki-tui-modals` (overlays); glyphs and palettes stay in `niki-render` / `niki-tui-themes`. No new crate, no new dependency. Every change is a `TestBackend` buffer assertion, because the state layer is terminal-free.

**Tech Stack:** Rust 2021, `ratatui` 0.29, `crossterm`.

**Spec:** the design checklist supplied 2026-11-28 (`libs/ui-parity/AUDIT-reference-vs-niki.md` holds the evidence table it was written against; that file is untracked on purpose because it names the reference).

## Global Constraints

- Reference products are not named in any tracked file. "The reference client" is the only permitted phrasing in comments, docs, tests and commit messages.
- Gate before any Rust change is called done: `cd rust && cargo fmt --all -- --check && cargo clippy -j 2 --workspace --all-targets -- -D warnings && cargo test -j 2 --workspace`.
- The machine has 8 GB of RAM: cargo runs at `-j 2`.
- User-visible language is Niki Agent; no compatibility aliases for older names.
- Every animated element obeys `Motion::Reduced` and `Effects`; every non-ASCII glyph has an ASCII twin in the same registry.
- Animated elements never change the layout: a frame may repaint a cell, never insert or remove a row.

## Review Focus

- **A tool call that returns nothing.** Today the result row is empty, so the screen shows a bullet with no evidence the call happened. Expected: `(no output)`.
- **A tool call that fails.** The row switches to the error colour, but the marker does not say the call failed. Expected: a `×` marker and the failure wording.
- **A 60-column terminal with a busy turn plus counts.** Live row, composer, state line and hints must all still fit; nothing may be clipped mid-glyph.
- **Reduced motion while busy.** The live row must still say a turn is running, and must not move.
- **A very long tool result.** The hidden-line notice must say how many lines were hidden and how to see them.

---

### Task 1: First screen, composer copy, live turn row, footer split

**Files:**
- Modify: `rust/crates/niki-tui-live/src/ui.rs`
- Modify: `rust/crates/niki-tui-live/src/app.rs`
- Modify: `rust/crates/niki-tui-live/src/status.rs`
- Test: `rust/crates/niki-tui-live/tests/screen.rs`, `rust/crates/niki-tui-live/src/app.rs` (`mod tests`), `rust/crates/niki-tui-live/src/status.rs` (`mod tests`), `rust/crates/niki-tui-live/src/ui.rs` (`mod tests`)

**Interfaces:**
- Consumes: `App::sync_turn_clock(&mut self)`, `App::working_line(&self) -> Option<String>`, `Status::context_line(&self, separator: &str) -> String`, `status::format_elapsed(Duration) -> String`.
- Produces: `pub const PLACEHOLDER: &str = "Ask Niki to do anything"`; `draw_working(frame, app, area, line)`; `display_dir_in(cwd: &str, home: Option<&str>) -> String`.

- [x] **Step 1: Welcome block** — `  >_ NIKI AGENT (vX)`, then the home-relative directory indented five, then `  To get started, describe a task or try one of these commands:`, then five `  /cmd - description` rows. A YOLO session adds `  permissions: YOLO mode`.
- [x] **Step 2: Placeholder** — `Ask Niki to do anything`; the key list moves entirely to the footer's hint row.
- [x] **Step 3: Live turn row** — one row above the composer while `TurnState::Working`: `• Working (12s · esc to interrupt)`. The clock starts on the first busy draw (`sync_turn_clock`) and clears when the turn stops.
- [x] **Step 4: Footer** — line 1 keeps the state contract; line 2 is hints on the left and context/token accounting right-aligned. Hints drop whole entries until they fit; below twelve columns for keys the counts take the row alone.
- [x] **Step 5: No duplicated state** — while working, `status_line()` prints `model · mode · cwd` and lets the live row own the state word.
- [x] **Step 6: Tests** — `the_composer_rule_sits_above_the_two_line_footer`, `a_fresh_session_shows_the_welcome_block`, `a_busy_turn_gets_a_clocked_row_above_the_composer`, `the_footer_keeps_the_hints_and_puts_the_counts_on_the_right`, `a_narrow_footer_gives_the_row_to_the_counts`, `elapsed_reads_as_seconds_then_minutes_then_hours`, `the_context_line_leaves_the_turn_out`, `the_home_directory_collapses_to_a_tilde`.

### Task 2: Key overlay in three columns

**Files:**
- Modify: `rust/crates/niki-tui-modals/src/help.rs`
- Modify: `rust/crates/niki-tui-modals/src/overlay.rs`
- Test: `rust/crates/niki-tui-modals/src/overlay.rs` (`mod tests`), `rust/crates/niki-tui-live/tests/screen.rs`

**Interfaces:**
- Consumes: `HelpOverlay::groups() -> &'static [ShortcutGroup]`.
- Produces: `pub struct ShortcutGroup { pub title: &'static str, pub entries: &'static [(&'static str, &'static str)] }`; `pub const GROUPS: [ShortcutGroup; 3]`; `render_with_footer(...)`.

- [x] **Step 1: Groups** — `Compose` / `Session` / `Transcript`, each entry a chord and a label of at most 21 columns.
- [x] **Step 2: Column flow** — three columns when the widths plus two four-cell gaps fit, two when the first group plus the merged pair fit, one otherwise.
- [x] **Step 3: Fixed footer** — the last inner row always shows `? or escape closes; page up and page down scroll`, so the way out survives a body taller than the terminal.
- [x] **Step 4: Wider box** — the key map uses `HELP_WIDTH = 104`; every other overlay keeps `MAX_WIDTH = 76`.
- [ ] **Step 5: Test the collapse** — add to `overlay.rs`: `the_key_map_folds_from_three_columns_to_one` asserting the rendered body at 120×40, 70×40 and 40×40 has three, two and one `Compose`/`Session`/`Transcript` headings on the first heading row.
- [ ] **Step 6: Run** `cargo test -j 2 -p niki-tui-modals` — expect all green.

### Task 3: Status line carries the branch

**Files:**
- Modify: `rust/crates/niki-tui-live/src/status.rs`
- Modify: `rust/crates/niki-tui-live/src/app.rs` (where `SessionInfo` is turned into `Status`)
- Modify: `rust/crates/niki/src/` — whatever builds `SessionInfo`
- Test: `rust/crates/niki-tui-live/src/status.rs`, `rust/crates/niki-tui-live/tests/screen.rs`

**Interfaces:**
- Produces: `Status::git_branch: Option<String>`, `Status::set_branch(&mut self, branch: Option<String>)`, and the field order `model · mode · branch · cwd` in `context_line`.

- [ ] **Step 1: Write the failing test** in `status.rs`: with `status.set_branch(Some("niki-agent".into()))`, `state_line(" · ", None)` is `mock:mock-model · manual · niki-agent · idle · /work`, and with `None` the line is unchanged from today.
- [ ] **Step 2: Run** it — expect a missing-method error.
- [ ] **Step 3: Implement** the field, the setter, and insertion into `state_line` and `context_line`; nothing prints when the branch is `None` or `"HEAD"`.
- [ ] **Step 4: Read the branch** in the binary: `git rev-parse --abbrev-ref HEAD` in the session's cwd, once at start-up, off the draw path. A missing `git` or a non-repository leaves it `None`.
- [ ] **Step 5: Run** `cargo test -j 2 -p niki-tui-live` and the workspace gate.
- [ ] **Step 6: Commit** `feat(rust): put the git branch on the state line`.

### Task 4: Tool rows say what happened

**Files:**
- Modify: `rust/crates/niki-render/src/lib.rs` (`Glyphs`)
- Modify: `rust/crates/niki-tui-live/src/transcript.rs`
- Modify: `rust/crates/niki-tui-live/src/ui.rs`
- Test: `rust/crates/niki-tui-live/tests/screen.rs`, `rust/crates/niki-render/src/lib.rs`

**Interfaces:**
- Produces: `Row::status: Option<ToolStatus>` where `enum ToolStatus { Running, Ok, Failed }`; `Glyphs::ok`, `Glyphs::failed` (`✓`/`×`, ASCII `+`/`x`); `Transcript::push_tool_result` sets `Ok` or `Failed`.

- [ ] **Step 1: Write the failing test** in `tests/screen.rs`: after `ToolStart{name:"shell"}` and `ToolEnd{output:"", is_error:false}`, the screen contains `• shell` on the call row, `✓ done` on the result row, and `(no output)`.
- [ ] **Step 2: Run** it — expect the assertions to fail on `✓ done`.
- [ ] **Step 3: Implement** the status word on the result row: `✓ done` / `× failed` / nothing while running (the call row keeps the activity marker), and `(no output)` when the trimmed output is empty.
- [ ] **Step 4: Run** the gate; **Commit** `feat(rust): give tool rows a status word`.

### Task 5: Documentation and public surface

**Files:**
- Modify: `docs/niki/UI.md` (the screen sample, the footer contract, the busy row, the key map)
- Modify: `CHANGELOG.md` (`## [Unreleased]`)
- Modify: `docs-site/docs/changelog.md` via `npm run sync`
- Test: `cd site && bun run check:brand`, `cd docs-site && npm run check:brand`

- [ ] **Step 1: Rewrite the screen sample** in `UI.md` from a real `TestBackend` capture at 100×26.
- [ ] **Step 2: Restate the footer contract** — hints are permanent; counts are right-aligned; the state word moves to the live row while working.
- [ ] **Step 3: Document the key map** — three groups, the collapse rule, the fixed footer line.
- [ ] **Step 4: Add the changelog entry** under `## [Unreleased]`, in the file's existing voice.
- [ ] **Step 5: Run both brand checks** and the docs build.
- [ ] **Step 6: Commit** `docs(rust): describe the composed interface`. New Markdown needs a `docs` type to pass the acknowledgement gate.

### Deferred, with reasons

- **Collapsible activity sections.** Needs a per-call height model and a key to toggle; the transcript is intentionally append-only, so this is a design change, not a paint change. Do it after a plan tool exists.
- **Live task checklist.** Requires a model-callable `todo` tool; `niki-tools` has no plan surface yet.
- **File autocomplete, sub-agent tree, dashboard pages.** Product features, not interface composition.
- **Glyph expansion beyond `✓`/`×`.** `▾`/`▸` and `├─`/`└─` exist to serve collapsing and nesting, which are deferred; adding them now would decorate nothing.
