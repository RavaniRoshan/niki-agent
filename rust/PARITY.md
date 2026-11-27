# Parity plan

The shipped product is `rust/`. The Python client in `libs/code` is the
reference: it is the record of every behaviour a user of 1.1.x had, and this
file tracks the distance between the two. A row moves to "done" only when the
Rust code does the thing and a test pins it.

Positioning: an open model / open provider terminal agent, one native binary,
with the density and speed of the fastest Rust terminals. Nothing here names an
upstream project; the behaviour is the specification, not the source.

## Where the two stand today

| Surface | Python reference | Rust today |
| --- | --- | --- |
| Slash commands | 47 public, plus aliases | 10, plus aliases (`niki-commands`) |
| Tools | file, edit, shell, search, web, todo, subagent, memory | 7: `read_file`, `write_file`, `edit_file`, `list_dir`, `shell`, `grep`, `glob`, plus MCP |
| Agent loop | multi-turn, compaction, subagents | multi-turn, hooks, skills, policy, cancel |
| Sessions | SQLite, thread browser, resume | SQLite, `--resume` / `--continue`, `/resume` |
| Context | token accounting, offload/summarize | per-turn usage only |
| Editor protocol | ACP | ACP |
| Headless | `-n`, JSON, exit codes | `-n`, JSON, exit codes 0/1/2/124/130 |
| Full-screen UI | ~100k lines of Python: diffs, tool cards, pickers, palettes | ~3.1k lines: transcript, composer, approval dialog, model picker, help |

`ROADMAP` in `rust/crates/niki-commands` is the machine-readable half of the
command table: a command leaves it the day `parse` answers it, and a test fails
if a name is in both places.

## Phase 1 — the interaction floor

Everything a person touches every minute. In flight as three slices.

| Slice | Work | Done when |
| --- | --- | --- |
| Commands | `niki-commands` carries the catalogue and aliases; `ROADMAP` names the gap | alias and roadmap tests pass; `/help` cannot list a dead command |
| Compose | slash autocomplete, prompt history in `$NIKI_HOME/history.jsonl`, copy the last answer | tests for filtering, Tab, history round-trip, the clipboard escape |
| Context | usage accumulation across turns, `ContextReport`, real compaction through the provider | compaction replaces older turns once; a provider failure leaves history intact |
| Delegate | `task` tool that runs a child agent with a fresh context and returns its answer | the child cannot spawn a child; cancel reaches it; the step budget is reported |

## Phase 2 — the agent shell

| Work | Detail |
| --- | --- |
| Approval modes in the UI | `/manual`, `/auto`, `/yolo` change the mode without restarting; the status line shows it |
| Session browser | `/threads` lists, filters and resumes; the picker previews the first prompt |
| Memory | `/remember` writes a skill or a note under `~/.niki`; the agent reads it next launch |
| Goals | `/goal` sets an objective and a finish line, and the agent reports against it |
| Prompt reuse | `/prompts` searches history and inserts a previous prompt |
| Editor | `/editor` opens `$EDITOR` over the draft |

## Phase 3 — the Codex-class surface

| Work | Detail |
| --- | --- |
| Transcript | streamed markdown, per-tool cards with collapsed output, unified diffs with line numbers |
| Status bar | model, mode, context percentage, token counts; never a redraw when idle |
| Pickers | one fuzzy picker for models, sessions, prompts and themes; `Ctrl+K` palette |
| Approvals | per-command preview, "always allow this command", a path-boundary warning |
| Accessibility | every binding in the help overlay, ASCII glyphs, reduced motion, contrast gate |

## Rules for this work

- A behaviour moves only with a test that fails without it.
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` green before
  a slice is called done.
- The TUI keeps its invariant: an idle app writes no bytes.
- No claim in README, docs or release notes until the code backs it.
