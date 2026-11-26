# UI parity with the Claude Code CLI

Every row below is a capability the current Claude Code CLI documents, checked
against Niki's terminal UI. A row is only `DONE` when a test in this repository
exercises it; the evidence column names the test file, and the note says what it
proves. `PARTIAL` rows are still visible to users — they say what is narrower
than the reference, not that the feature is absent.

There is no `MISSING` P0 row. The one P0 gap found in this audit (`#` memory)
was implemented in `niki_agent/memory_note.py` and is covered by
`tests/unit_tests/test_memory_note.py`.

## Capability rows

| # | Row | Status | Evidence | Note |
| --- | --- | --- | --- | --- |
| 1 | Streaming tool calls with collapse/expand | DONE | `tests/unit_tests/niki/test_flow_and_toolcards.py`, `tests/unit_tests/test_app.py::TestAskUserLifecycle`, `niki_agent/tui/widgets/tool_widgets.py` | Cards stream into one block as chunks arrive; `ctrl+o` toggles the most recent expandable row's output. |
| 1b | Structured diffs with context | DONE | `tests/unit_tests/niki/test_diffs.py`, `niki_agent/tui/widgets/diff.py` | Unified diff with hunks, gutter, and a summary line. |
| 2 | Todo/plan display that survives turns | DONE | `tests/unit_tests/niki/test_goal_status.py`, `_format_todos_output` in `niki_agent/tui/widgets/messages.py` | `write_todos` renders as a checklist message, so it stays in the transcript. |
| 3a | Slash commands | DONE | `niki_agent/command_registry.py` (one `COMMANDS` table), `tests/unit_tests/niki/test_command_palette.py`, `tests/unit_tests/niki/test_keymap.py::test_help_and_markdown_come_from_one_registry` | Registry-driven: palette, autocomplete, help, and handlers share one table. |
| 3b | `@` file mentions | DONE | `tests/unit_tests/niki/test_composer.py`, `_mention_start` in `niki_agent/tui/widgets/autocomplete.py` | Fuzzy path completion; the mention is resolved to the file before send. |
| 3c | `#` memory/instructions | DONE | `tests/unit_tests/test_memory_note.py`, `_save_memory_note` in `niki_agent/app.py` | A leading `#` writes bullets under `## Notes` in the active agent profile's `AGENTS.md` instead of sending a turn. Multi-line notes become one bullet per line. |
| 4a | Plan mode | DONE | `tests/unit_tests/test_main_args.py::TestPlanModeArgument`, `_plan_mode_fs_tools` in `niki_agent/main.py` | `--plan` narrows the filesystem tool set to `ls`, `read_file`, `glob`, and `grep`; writes and shell execution are not in the allowlist. |
| 4b | Approval prompts with a diff preview before write | DONE | `tests/unit_tests/niki/test_approvals.py`, `preview.diff` in `niki_agent/tui/widgets/tool_renderers.py` | The approval modal shows the pending diff before the write is allowed. |
| 5a | Session picker | DONE | `tests/unit_tests/niki/test_session_picker.py`, `niki_agent/tui/widgets/thread_selector.py` | Filter input, cursor rows, resume in place. |
| 5b | Resume | DONE | `tests/unit_tests/test_threads_resume.py`, `-r`/`--resume` in `niki_agent/main.py` | Resume by id or most recent; the hint uses the invoked command name. |
| 5c | Transcript search | DONE | `tests/unit_tests/niki/test_history_search.py`, `niki_agent/tui/widgets/prompt_search.py` | `ctrl+r` opens prompt history search; choosing a result puts it back in the composer. |
| 5d | Message copy | DONE | `tests/unit_tests/niki/test_copy_and_selection.py`, `/copy` in `niki_agent/command_registry.py` | Selection copies through OSC 52 where available, with a clipboard fallback. |
| 6a | Multi-line paste and large-paste placeholder | DONE | `tests/unit_tests/niki/test_pty_paste_and_suspend.py`, `CollapsingPasteTextArea` in `niki_agent/tui/widgets/_paste_textarea.py` | A large paste collapses to `[Pasted text #N]` and expands on submit. |
| 6b | Image paste where the terminal supports it | DONE | `tests/unit_tests/tui/widgets/test_chat_input.py` (`MediaTracker`), `tests/unit_tests/test_media_utils.py` | Attachments become `[image N]` tokens that delete as one unit. Terminals that cannot deliver an image never produce the token. |
| 7 | Subagent/background task display with a stable activity line | DONE | `tests/unit_tests/niki/test_activity_line.py`, `niki_agent/tui/widgets/subagent_panel.py` | One activity line per running task; the line is updated in place, not re-appended. |
| 8 | MCP server status and tool listing | DONE | `tests/unit_tests/client/commands/test_mcp.py`, `tests/unit_tests/test_mcp_lifecycle.py`, `/mcp` + `niki_agent/tui/widgets/mcp_viewer.py` | Status, per-server tool listing, reconnect, and login prompts. |
| 9 | Themes with a contrast-safe default, 16/256/truecolor | DONE | `tests/unit_tests/niki/test_theme_contrast.py`, `tests/unit_tests/niki/test_ansi_palette_contrast.py`, `tests/unit_tests/niki/test_color_depth_and_input.py` | Four themes; every token pair measured at all three depths. |
| 10 | Keybinding registry with discoverable help | DONE | `tests/unit_tests/niki/test_keymap.py`, `docs/niki/KEYMAP.md` | One registry feeds the footer hint, `?` help, the generated keymap, and the command palette. |

## Accessibility bar

| Concern | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| Colour is never the only signal | every status also has a glyph or word | DONE | `tests/unit_tests/niki/test_transcript_roles.py`, `tests/unit_tests/niki/test_turn_summary.py` |
| Contrast | text ≥ 4.5:1, UI glyphs ≥ 3:1, in every theme at 16/256/truecolor | DONE | `tests/unit_tests/niki/test_theme_contrast.py` quantizes each RGB palette to xterm-256 and measures the 16-colour fallbacks against the documented palettes |
| Colour depth | `NO_COLOR`, `NIKI_COLOR_DEPTH=16/256/truecolor` all legible | DONE | `tests/unit_tests/niki/test_color_depth_and_input.py`, `tests/unit_tests/niki/test_ansi_palette_contrast.py` |
| Glyphs | `NIKI_GLYPHS=ascii` covers every glyph | DONE | `tests/unit_tests/niki/test_motion.py` (ASCII alias), glyph registry in `niki_agent/config.py` |
| Motion | `NIKI_MOTION=off` stops spinners and animation | DONE | `tests/unit_tests/niki/test_motion.py` |
| Screen readers | `NIKI_SCREEN_READER=1`: linear, append-only, no in-place rewriting | DONE | `tests/unit_tests/niki/test_screen_reader.py` — in-process run asserts no spinner and no escape sequences; a PTY run asserts a bare launch explains the linear mode and exits 2 |
| Keyboard | every action reachable without a pointer | DONE | `tests/unit_tests/niki/test_keymap.py::test_every_advertised_action_has_a_handler` |
| Layout | 40, 80, 120 columns without clipping; 80×24 default | DONE | `tests/unit_tests/niki/test_snapshots.py` — 40×20, 50×16, 80×24, 120×38, 160×45 first-run and typed; 40/50/120 with the slash menu and help open |
| Language | plain words, no icon-only affordances, errors say what to do next | DONE | `tests/unit_tests/niki/test_error_rendering.py`, copy review in `docs/niki/DESIGN.md` |

### Narrow-width placeholder

The 40-column snapshot exposed a clipped composer placeholder. `composer_hint()`
in `niki_agent/niki/app.py` now picks the widest sentence that fits the measured
text area, and `test_v5_the_placeholder_shrinks_before_it_clips` pins the
breakpoints.

## What is deliberately narrower

- **Screen-reader mode is one turn per invocation.** The linear renderer reads a
  prompt and prints the answer; it does not mount a multi-turn REPL, because a
  REPL's steady-state repaint is exactly what a screen reader cannot follow.
  `niki -n "..."` and `echo ... | niki` both work with the switch set.
- **Image paste depends on the terminal.** Niki records the attachment when the
  terminal hands one over; it does not implement a graphics protocol of its own.

## Reference surface

Rows were taken from the Claude Code CLI documentation and the help screen
captured during the parity audit, not from memory. Where the reference CLI
behaves differently in detail (for example, `#` writing to the project memory
file versus the profile memory file), the note above says what Niki does.
