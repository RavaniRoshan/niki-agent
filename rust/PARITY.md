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
| Slash commands | 47 public, plus aliases | 15, plus aliases; 21 on `ROADMAP` (`niki-commands`) |
| Tools | file, edit, shell, search, web, todo, subagent, memory | 8: `read_file`, `write_file`, `edit_file`, `list_dir`, `shell`, `grep`, `glob`, `task`; plus MCP |
| Agent loop | multi-turn, compaction, subagents | multi-turn, hooks, skills, policy, cancel, delegation |
| Sessions | SQLite, thread browser, resume | SQLite, `--resume` / `--continue`, `/resume` by index or id, `/threads` browser |
| Context | token accounting, offload/summarize | `context_report`, `context_audit`, `/compact` through the provider, occupancy in the status line |
| Editor protocol | ACP | ACP |
| Headless | `-n`, JSON, exit codes | `-n`, JSON (usage, session totals, context audit), exit codes 0/1/2/124/130 |
| Full-screen UI | ~100k lines of Python: diffs, tool cards, pickers, palettes | ~4k lines: transcript, composer with completion and history, copy, approval dialog, model picker, session browser, help, status line |

`ROADMAP` in `rust/crates/niki-commands` is the machine-readable half of the
command table: a command leaves it the day `parse` answers it, and a test fails
if a name is in both places.

## Phase 1 — the interaction floor

Everything a person touches every minute. All four slices are in.

| Slice | State |
| --- | --- |
| Commands | Done: `niki-commands` carries the catalogue and aliases; `ROADMAP` names the gap; alias and roadmap tests keep `/help` from listing a dead command. |
| Compose | Done: slash autocomplete, prompt history in `$NIKI_HOME/history.jsonl`, `Ctrl+Y` copy, all in the help overlay. |
| Context | Done: usage and steps across turns, `ContextReport`, `ContextAudit`, real compaction through the provider, occupancy in the status line. |
| Delegate | Done: the `task` tool runs a child with a fresh history, no `task` of its own, the parent's cancel handle and a 20-step budget. |

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
| Transcript | the Markdown renderer exists in `niki-render` with tests; the full-screen transcript does not call it yet. Per-tool cards with collapsed output and unified diffs with line numbers are still open. |
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
