# Turn Summary Line Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close every finished turn with one quiet line that says what happened — outcome, wall time, output tokens, files changed, tool calls — styled error-red on failure and never written to the session store.

**Architecture:** A new `RowKind::Summary` / `RowKind::SummaryFailed` transcript row is appended by `App::finish` from three values the app already owns (`turn_clock`, `RunOutcome::usage`, per-turn tool events) plus one new set (`turn_files`). The wording lives in a pure `status::TurnSummary::line` so it is unit-testable without a terminal; the renderer elides it to one row instead of wrapping.

**Tech Stack:** Rust 2021, ratatui, `serde_json` (already a dependency of `niki-tui-live`), no new crates.

**Spec:** Addendum A / screen contract S-05 (turn summary line) and gap F6 in the TUI build spec: "status, wall time, output tokens, files changed, tools run. One line. Failure turns say so in the error color and name the recovery action."

## Global Constraints

- Rust gate, always at `-j 2` (machine has 8 GB): `cd rust && cargo fmt --all -- --check && cargo clippy -j 2 --workspace --all-targets -- -D warnings && cargo test -j 2 --workspace`.
- Branch `RavaniRoshan/rust/turn-summary`; Conventional Commit scope `rust`; text after `type(rust):` starts lowercase.
- No new dependencies. `serde_json` and `niki-provider` are already in `rust/crates/niki-tui-live/Cargo.toml`.
- Every user-visible string is original wording. The brand gate (`docs-site/scripts/check-brand.mjs`, `site/scripts`) rejects reference-product names anywhere in the tree; do not introduce them in code, tests, comments, or commit messages.
- Omit a fact rather than print a zero or a guess (RULES.md truthfulness rule, checklist V6). A token count the provider did not report, and a clock that never ran, are absent — not `0`.
- Notices and diffs are view furniture and are not persisted; the summary joins them (`persisted()` returns `None`).

## Review Focus

- A turn that produced no answer text at all (tool-only, or an empty `EndTurn`): the summary still closes the turn, and it is the last row.
- A failed turn after a partial answer: the `Error` row and the summary both exist, the summary is last, and it is the error-tone variant.
- The same file written twice in one turn: counted once.
- A cancelled turn that never reached the model (`StopReason::Other("cancelled")`): summary says `stopped`, no panic, no second summary when `finish` is called again.
- A 40-column (and a 10-column) terminal: the summary occupies exactly one row and is elided, never wrapped and never a panic.

---

### Task 1: A summary row kind the screen can tell apart

**Files:**
- Modify: `rust/crates/niki-tui-live/src/transcript.rs:5-20` (variant), `:152-160` (after `push_notice`)
- Modify: `rust/crates/niki-tui-live/src/ui.rs:291-308`
- Modify: `rust/crates/niki-tui-live/src/transcript_overlay.rs:250-257`
- Modify: `rust/crates/niki-tui-live/src/app.rs:2090-2110` (`persisted`)
- Test: `rust/crates/niki-tui-live/src/transcript.rs` (`mod tests` at the bottom)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `RowKind::Summary` — a turn closed normally or was stopped.
  - `RowKind::SummaryFailed` — a turn failed; painted in the error tone.
  - `Transcript::push_summary(&mut self, text: impl Into<String>)`
  - `Transcript::push_failed_summary(&mut self, text: impl Into<String>)`
  - Both push `Row { title: None, rendered: None }` and end any open stream.

Two flat variants, not `Summary { failed: bool }`: every existing site compares with `row.kind == RowKind::X` or `!=`, and flat variants keep those comparisons compiling while the exhaustive `match` arms still force each paint site to decide.

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `transcript.rs`:

```rust
#[test]
fn a_summary_row_closes_the_stream_and_is_not_a_notice() {
    let mut transcript = Transcript::default();
    transcript.push_assistant("half");
    transcript.push_summary("done · 14s");
    assert!(!transcript.is_streaming());
    assert_eq!(transcript.rows().last().map(|row| row.kind), Some(RowKind::Summary));
    assert!(transcript.has_conversation(), "a summary is the app closing a turn, not furniture");
}

#[test]
fn a_failed_summary_is_its_own_kind() {
    let mut transcript = Transcript::default();
    transcript.push_failed_summary("failed · 12s");
    assert_eq!(
        transcript.rows().last().map(|row| row.kind),
        Some(RowKind::SummaryFailed)
    );
}
```

and to `app.rs`'s `mod tests`, next to `a_saved_turn_drops_the_new_marker_from_the_session`:

```rust
#[test]
fn a_summary_is_view_furniture_and_stays_out_of_the_session() {
    let mut app = app();
    app.transcript.push_user("hello");
    app.transcript.push_summary("done · 14s");
    app.transcript.push_failed_summary("failed · 12s");
    assert!(persisted(&app.transcript.rows()[1]).is_none());
    assert!(persisted(&app.transcript.rows()[2]).is_none());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd rust && cargo test -j 2 -p niki-tui-live summary`
Expected: FAIL — `no variant or associated item named 'Summary' found for enum 'RowKind'` (and the same for `push_summary`).

- [ ] **Step 3: Add the variants, the pushers, and every match arm**

In `transcript.rs`, after `Diff`:

```rust
    /// The line that closes a turn: how it ended and what it cost.
    Summary,
    /// The same line for a turn that failed, painted in the error tone.
    SummaryFailed,
```

In `transcript.rs`, after `push_notice`:

```rust
    /// Record the line that closes a turn.
    pub fn push_summary(&mut self, text: impl Into<String>) {
        self.push_closing(RowKind::Summary, text);
    }

    /// Record the closing line of a failed turn.
    pub fn push_failed_summary(&mut self, text: impl Into<String>) {
        self.push_closing(RowKind::SummaryFailed, text);
    }

    fn push_closing(&mut self, kind: RowKind, text: impl Into<String>) {
        self.streaming = false;
        self.rows.push(Row {
            kind,
            title: None,
            text: text.into(),
            rendered: None,
        });
    }
```

In `ui.rs` `transcript_lines`, before the `match row.kind`, handle both summary kinds so the line is elided to one row rather than wrapped (the same reason a diff is not wrapped):

```rust
        if matches!(row.kind, RowKind::Summary | RowKind::SummaryFailed) {
            if !lines.is_empty() {
                lines.push(Line::default());
            }
            let style = if row.kind == RowKind::SummaryFailed {
                palette.error_style()
            } else {
                palette.muted_style()
            };
            lines.push(Line::from(Span::styled(clip(&row.text, width).0, style)));
            continue;
        }
```

and add `RowKind::Summary | RowKind::SummaryFailed => continue,` to the `match` that follows (it is unreachable, and the arm exists so `match` on `RowKind` stays exhaustive).

In `ui.rs` `notice_lines` and `transcript_overlay.rs`, no change is needed to their `!= Notice` / `_ =>` filters.

In `transcript_overlay.rs` `push_plain`, add one arm to the `match row.kind`:

```rust
        RowKind::Summary => ("".to_owned(), palette.muted_style()),
        RowKind::SummaryFailed => ("".to_owned(), palette.error_style()),
```

In `app.rs` `persisted`, extend the last arm: `RowKind::Notice | RowKind::Diff | RowKind::Summary | RowKind::SummaryFailed => return None,`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd rust && cargo test -j 2 -p niki-tui-live summary`
Expected: PASS, including `app::tests::a_summary_is_view_furniture_and_stays_out_of_the_session`.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/niki-tui-live/src
git commit -m "feat(rust): add a summary row kind that closes a turn"
```

---

### Task 2: The words, as a pure function

**Files:**
- Modify: `rust/crates/niki-tui-live/src/status.rs` (after `format_tokens`, `:207`)
- Modify: `rust/crates/niki-tui-live/src/lib.rs` if `status` is not already re-exported (check `pub use status::…`)
- Test: `rust/crates/niki-tui-live/src/status.rs` (`mod tests` at the bottom)

**Interfaces:**
- Consumes: `status::format_elapsed(Duration) -> String` (exists, `status.rs:189`).
- Produces:
  - `pub fn format_compact_tokens(count: u64) -> String` — `480`, `1.2k`, `28k`, `1.3M`; one decimal, trailing `.0` dropped, rounded to nearest tenth.
  - `pub enum TurnResult { Done, Stopped, Failed }`
  - `pub struct TurnSummary { result: TurnResult, elapsed: Option<Duration>, output_tokens: u64, files: usize, tool_calls: usize, failure: Option<String> }`
  - `impl TurnSummary { pub fn line(&self, separator: &str) -> String }`

The four shapes the tests pin:

| `result` | line |
|---|---|
| `Done` | `done · 14s · 2.8k out · 1 file changed · 3 tool calls` |
| `Stopped` | `stopped · 14s · 1.2k out · 1 file changed · 3 tool calls` |
| `Failed` | `failed · 14s · provider mock failed: 503 · send a new prompt to continue` |
| no facts known | `done` |

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `status.rs`:

```rust
#[test]
fn compact_tokens_round_to_one_decimal() {
    assert_eq!(format_compact_tokens(0), "0");
    assert_eq!(format_compact_tokens(480), "480");
    assert_eq!(format_compact_tokens(1_234), "1.2k");
    assert_eq!(format_compact_tokens(2_000), "2k");
    assert_eq!(format_compact_tokens(28_000), "28k");
    assert_eq!(format_compact_tokens(1_250_000), "1.3M");
}

fn ended() -> TurnSummary {
    TurnSummary {
        result: TurnResult::Done,
        elapsed: Some(Duration::from_secs(14)),
        output_tokens: 2_800,
        files: 1,
        tool_calls: 3,
        failure: None,
    }
}

#[test]
fn a_done_summary_names_the_time_the_tokens_and_what_changed() {
    assert_eq!(
        ended().line(" · "),
        "done · 14s · 2.8k out · 1 file changed · 3 tool calls"
    );
    assert_eq!(
        ended().line(" | "),
        "done | 14s | 2.8k out | 1 file changed | 3 tool calls"
    );
}

#[test]
fn a_stopped_summary_says_stopped_and_keeps_the_tallies() {
    let summary = TurnSummary {
        result: TurnResult::Stopped,
        output_tokens: 1_200,
        ..ended()
    };
    assert_eq!(
        summary.line(" · "),
        "stopped · 14s · 1.2k out · 1 file changed · 3 tool calls"
    );
}

#[test]
fn a_failed_summary_names_the_failure_and_the_next_step() {
    let summary = TurnSummary {
        result: TurnResult::Failed,
        output_tokens: 0,
        files: 0,
        tool_calls: 0,
        failure: Some("provider mock failed: 503".to_owned()),
        ..ended()
    };
    assert_eq!(
        summary.line(" · "),
        "failed · 14s · provider mock failed: 503 · send a new prompt to continue"
    );
}

#[test]
fn a_summary_omits_what_it_does_not_know() {
    let summary = TurnSummary {
        elapsed: None,
        output_tokens: 0,
        files: 0,
        tool_calls: 0,
        ..ended()
    };
    assert_eq!(summary.line(" · "), "done", "no clock, no tokens, no tallies");

    let plural = TurnSummary {
        files: 2,
        tool_calls: 1,
        ..summary
    };
    assert_eq!(plural.line(" · "), "done · 2 files changed · 1 tool call");
}

#[test]
fn a_failure_message_keeps_to_one_line() {
    let summary = TurnSummary {
        result: TurnResult::Failed,
        output_tokens: 0,
        files: 0,
        tool_calls: 0,
        failure: Some("the stream broke\nafter one chunk".to_owned()),
        ..ended()
    };
    assert_eq!(
        summary.line(" · "),
        "failed · 14s · the stream broke · send a new prompt to continue"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd rust && cargo test -j 2 -p niki-tui-live compact_tokens`
Expected: FAIL — `cannot find function 'format_compact_tokens' in this scope`.

- [ ] **Step 3: Implement the formatter and the summary**

`format_compact_tokens`, next to `format_tokens`:

```rust
pub fn format_compact_tokens(count: u64) -> String {
    if count < 1_000 {
        return count.to_string();
    }
    let (unit, scale) = if count < 1_000_000 {
        ("k", 100)
    } else {
        ("M", 100_000)
    };
    let tenths = (count + scale / 2) / scale;
    if tenths % 10 == 0 {
        format!("{}{unit}", tenths / 10)
    } else {
        format!("{}.{}{unit}", tenths / 10, tenths % 10)
    }
}
```

`TurnResult`, `TurnSummary`, and `line`:

```rust
pub enum TurnResult {
    /// The turn ended on its own terms.
    Done,
    /// The person stopped it; what it produced is kept.
    Stopped,
    /// The turn ended on a provider or tool failure.
    Failed,
}
```

```rust
impl TurnSummary {
    pub fn line(&self, separator: &str) -> String {
        let mut parts = vec![match self.result {
            TurnResult::Done => "done".to_owned(),
            TurnResult::Stopped => "stopped".to_owned(),
            TurnResult::Failed => "failed".to_owned(),
        }];
        if let Some(elapsed) = self.elapsed {
            parts.push(format_elapsed(elapsed));
        }
        if self.output_tokens > 0 {
            parts.push(format!("{} out", format_compact_tokens(self.output_tokens)));
        }
        if self.files > 0 {
            parts.push(count_of(self.files, "file changed", "files changed"));
        }
        if self.tool_calls > 0 {
            parts.push(count_of(self.tool_calls, "tool call", "tool calls"));
        }
        if self.result == TurnResult::Failed {
            if let Some(failure) = &self.failure {
                parts.push(clip_line(failure, 120));
            }
            parts.push("send a new prompt to continue".to_owned());
        }
        parts.join(separator)
    }
}

fn count_of(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}

/// The first line of a message, shortened to `limit` characters.
fn clip_line(text: &str, limit: usize) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() > limit {
        format!("{}…", line.chars().take(limit).collect::<String>())
    } else {
        line.to_owned()
    }
}
```

`TurnResult` needs `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` and `TurnSummary` needs `#[derive(Debug, Clone, PartialEq, Eq)]`, matching the neighbours in the file. Re-export both from `lib.rs` next to `TurnState` if that is where `TurnState` is re-exported.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd rust && cargo test -j 2 -p niki-tui-live --lib status`
Expected: PASS, six new tests.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/niki-tui-live/src/status.rs rust/crates/niki-tui-live/src/lib.rs
git commit -m "feat(rust): write the turn summary line from what the turn knows"
```

---

### Task 3: `finish` closes the turn with the line

**Files:**
- Modify: `rust/crates/niki-tui-live/src/app.rs:170-195` (fields), `:1060-1100` (`apply`), `:1095-1125` (`finish`), `:1335-1345` (`begin_turn`), `:2090-2110` (`persisted`, done in Task 1)
- Test: `rust/crates/niki-tui-live/src/app.rs` (`mod tests`)

**Interfaces:**
- Consumes: `Transcript::push_summary` / `push_failed_summary` (Task 1); `status::{TurnResult, TurnSummary}` (Task 2); `RunOutcome::usage.output_tokens`.
- Produces: private `App` fields `turn_files: BTreeSet<String>` and `pending_file: Option<String>`, and `fn closing_line(&self, result: TurnResult, elapsed: Option<Duration>, outcome: Option<&RunOutcome>, failure: Option<&str>) -> String`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `app.rs`:

```rust
#[test]
fn a_finished_turn_closes_with_one_summary_row() {
    let mut app = app();
    type_text(&mut app, "fix the parser");
    assert_eq!(
        press(&mut app, KeyCode::Enter),
        Some(Command::Submit("fix the parser".to_owned()))
    );
    app.apply(AgentEvent::Text("patched".to_owned()));
    let mut outcome = outcome();
    outcome.usage = Usage {
        input_tokens: 12,
        output_tokens: 2_800,
    };
    assert_eq!(app.finish(Ok(outcome)), None);

    let last = app.transcript.rows().last().expect("a summary");
    assert_eq!(last.kind, RowKind::Summary);
    assert!(last.text.starts_with("done"), "{}", last.text);
    assert!(last.text.contains("2.8k out"), "{}", last.text);
    assert!(
        !last.text.contains("file changed"),
        "no file was written: {}",
        last.text
    );
}

#[test]
fn a_stopped_turn_closes_with_a_stopped_summary() {
    let mut app = app();
    type_text(&mut app, "write something long");
    let _ = press(&mut app, KeyCode::Enter);
    app.apply(AgentEvent::Text("half an answer".to_owned()));
    app.apply(AgentEvent::Cancelled);
    let mut outcome = outcome();
    outcome.stop = StopReason::Other("cancelled".to_owned());
    assert_eq!(app.finish(Ok(outcome)), None);

    let rows = app.transcript.rows();
    assert_eq!(rows[rows.len() - 2].text, "cancelled", "the notice stays");
    assert_eq!(rows[rows.len() - 1].kind, RowKind::Summary);
    assert!(rows[rows.len() - 1].text.starts_with("stopped"));
}

#[test]
fn a_failed_turn_closes_with_a_failed_summary_after_the_error() {
    let mut app = app();
    type_text(&mut app, "do the thing");
    let _ = press(&mut app, KeyCode::Enter);
    app.apply(AgentEvent::Text("part of an answer".to_owned()));
    assert_eq!(
        app.finish(Err("provider mock failed: 503".to_owned())),
        None
    );

    let rows = app.transcript.rows();
    let last = rows.last().expect("a summary");
    assert_eq!(last.kind, RowKind::SummaryFailed);
    assert!(last.text.starts_with("failed"), "{}", last.text);
    assert!(last.text.contains("503"), "{}", last.text);
    assert!(
        last.text.contains("send a new prompt to continue"),
        "{}",
        last.text
    );
    assert!(
        rows.iter()
            .any(|row| row.kind == RowKind::Error && row.text.contains("503")),
        "the full message stays in the transcript"
    );
}

#[test]
fn no_turn_ran_so_no_summary_is_written() {
    let mut app = app();
    assert_eq!(app.finish(Ok(outcome())), None);
    assert!(app.transcript.rows().is_empty(), "{:?}", app.transcript.rows());
}

#[test]
fn each_file_written_in_a_turn_counts_once() {
    let mut app = app();
    type_text(&mut app, "write it twice");
    let _ = press(&mut app, KeyCode::Enter);
    for _ in 0..2 {
        app.apply(AgentEvent::ToolStart {
            name: "write_file".to_owned(),
            arguments: r#"{"file_path":"src/a.rs","content":"x"}"#.to_owned(),
        });
        app.apply(AgentEvent::ToolEnd {
            name: "write_file".to_owned(),
            output: "wrote src/a.rs".to_owned(),
            is_error: false,
        });
    }
    app.apply(AgentEvent::ToolStart {
        name: "edit_file".to_owned(),
        arguments: r#"{"path":"src/b.rs","old_text":"a","new_text":"b"}"#.to_owned(),
    });
    app.apply(AgentEvent::ToolEnd {
        name: "edit_file".to_owned(),
        output: "edited src/b.rs".to_owned(),
        is_error: false,
    });
    let mut outcome = outcome();
    outcome.tool_calls = 3;
    assert_eq!(app.finish(Ok(outcome)), None);

    let text = app.transcript.rows().last().expect("a summary").text.clone();
    assert!(text.contains("2 files changed"), "{text}");
    assert!(text.contains("3 tool calls"), "{text}");
}

#[test]
fn a_failed_write_does_not_count_as_a_changed_file() {
    let mut app = app();
    type_text(&mut app, "try to write");
    let _ = press(&mut app, KeyCode::Enter);
    app.apply(AgentEvent::ToolStart {
        name: "write_file".to_owned(),
        arguments: r#"{"file_path":"/etc/passwd","content":"x"}"#.to_owned(),
    });
    app.apply(AgentEvent::ToolEnd {
        name: "write_file".to_owned(),
        output: "refused: outside the workspace".to_owned(),
        is_error: true,
    });
    let mut outcome = outcome();
    outcome.tool_calls = 1;
    assert_eq!(app.finish(Ok(outcome)), None);

    let text = app.transcript.rows().last().expect("a summary").text.clone();
    assert!(!text.contains("file changed"), "{text}");
    assert!(text.contains("1 tool call"), "{text}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd rust && cargo test -j 2 -p niki-tui-live --lib a_finished_turn_closes`
Expected: FAIL — the last row is the assistant row, `assert_eq!` on `RowKind::Summary` panics.

- [ ] **Step 3: Implement the accounting and the closing line**

Fields in `App`, next to `turn_start`:

```rust
    /// Files a successful tool call wrote or edited during the turn in flight,
    /// so the closing line counts each one once.
    turn_files: BTreeSet<String>,
    /// The path the tool call now running will write, if it is one of those.
    pending_file: Option<String>,
```

Initialise both in `App::new` (`BTreeSet::new()`, `None`) and change `use std::collections::VecDeque;` to `use std::collections::{BTreeSet, VecDeque};`.

In `begin_turn`, next to `self.turn_start = …`:

```rust
        self.turn_files.clear();
        self.pending_file = None;
```

In `apply`, record starts and commit on success:

```rust
            AgentEvent::ToolStart { name, arguments } => {
                self.pending_file = written_path(&name, &arguments);
                self.transcript.push_tool_call(&name, &arguments);
            }
            AgentEvent::ToolEnd {
                name,
                output,
                is_error,
            } => {
                if !is_error {
                    if let Some(path) = self.pending_file.take() {
                        self.turn_files.insert(path);
                    }
                } else {
                    self.pending_file = None;
                }
                self.transcript.push_tool_result(&name, &output, is_error);
            }
```

Module-level helper, next to `is_cancelled`:

```rust
/// The file a successful `write_file` or `edit_file` call will have changed.
fn written_path(name: &str, arguments: &str) -> Option<String> {
    if name != "write_file" && name != "edit_file" {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(arguments).ok()?;
    ["file_path", "path"]
        .iter()
        .find_map(|key| value.get(key).and_then(|path| path.as_str()))
        .map(str::to_owned)
}
```

`finish` — take the clock before the state change, remember whether a turn ran, and close:

```rust
    pub fn finish(&mut self, result: Result<RunOutcome, String>) -> Option<Command> {
        self.pending = None;
        self.question = None;
        self.transcript.end_stream();
        let elapsed = self.turn_clock.take().map(|at| at.elapsed());
        let ran = self.turn_start.is_some();
        let closing = match &result {
            Ok(outcome) if is_cancelled(&outcome.stop) => {
                self.status.turn = TurnState::Cancelled;
                Some(self.closing_line(TurnResult::Stopped, elapsed, Some(outcome), None))
            }
            Ok(outcome) => {
                self.status.turn = TurnState::Idle;
                Some(self.closing_line(TurnResult::Done, elapsed, Some(outcome), None))
            }
            Err(message) => {
                self.transcript
                    .push_error(format!("the turn failed: {message}"));
                self.status.turn = TurnState::Failed;
                Some(self.closing_line(TurnResult::Failed, elapsed, None, Some(message.as_str())))
            }
        };
        if ran {
            if let Some(line) = closing {
                match result {
                    Ok(_) => self.transcript.push_summary(line),
                    Err(_) => self.transcript.push_failed_summary(line),
                }
            }
        }
        self.persist_turn();
        if self.status.turn == TurnState::Idle {
            if let Some(prompt) = self.queued.pop_front() {
                return Some(self.begin_turn(prompt));
            }
        }
        None
    }
```

`closing_line`:

```rust
    /// The words that close the turn, from what the run reported.
    fn closing_line(
        &self,
        result: TurnResult,
        elapsed: Option<Duration>,
        outcome: Option<&RunOutcome>,
        failure: Option<&str>,
    ) -> String {
        TurnSummary {
            result,
            elapsed,
            output_tokens: outcome.map_or(0, |outcome| outcome.usage.output_tokens),
            files: self.turn_files.len(),
            tool_calls: outcome.map_or(0, |outcome| outcome.tool_calls),
            failure: failure.map(str::to_owned),
        }
        .line(self.separator())
    }
```

The borrow of `result` in the `match &result` arms ends before `match result` moves it; if the borrow checker objects, match on a `Err(message) => { …; message }` capture as `Option<String>` instead of re-matching.

- [ ] **Step 4: Update the tests the new row changed**

Run: `cd rust && cargo test -j 2 -p niki-tui-live` and fix only these known expectations:

- `a_cancelled_turn_keeps_what_it_wrote`: it drives `finish` without a submit, so it now sees no summary; submit first (`type_text` + `press(Enter)`) and move the `"cancelled"` assertion to `rows()[rows.len() - 2]`. Its second `finish` call is the no-duplicate proof: assert the row count is unchanged by it.
- `a_failed_turn_says_so_in_the_transcript`: same — submit first; find the `RowKind::Error` row with `iter().find(...)` instead of `last()`.
- `a_second_escape_restores_the_last_prompt_and_drops_its_rows`: `rows().len() == 2` becomes `3`.
- `a_single_escape_does_not_backtrack`: `rows().len() == 1` becomes `2`.
- `tests/turn.rs::a_prompt_runs_on_the_worker_and_reaches_the_screen`: `rows.len() == 2` becomes `3` and the third row is `RowKind::Summary`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd rust && cargo test -j 2 -p niki-tui-live`
Expected: PASS, whole crate.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/niki-tui-live
git commit -m "feat(rust): close every turn with a summary line"
```

---

### Task 4: One row on a real screen, at two widths

**Files:**
- Test: `rust/crates/niki-tui-live/tests/screen.rs`

**Interfaces:**
- Consumes: everything from Tasks 1–3, plus the existing helpers `app()`, `render(app, width, height)`, `text(buffer)`, `type_text(app, text)`, `stroke(app, code)`.
- Produces: no production code.

- [ ] **Step 1: Write the failing tests**

Add to `tests/screen.rs` (import `RunOutcome` from `niki_agent` and `StopReason` from `niki_provider`, both already available):

```rust
fn finished_turn(app: &mut App) -> TurnState {
    type_text(app, "fix the login redirect");
    stroke(app, KeyCode::Enter);
    app.apply(AgentEvent::Text("Patched auth.py.".to_owned()));
    let outcome = RunOutcome {
        text: String::new(),
        steps: 1,
        tool_calls: 3,
        stop: StopReason::EndTurn,
        usage: Usage {
            input_tokens: 12,
            output_tokens: 2_800,
        },
    };
    let _ = app.finish(Ok(outcome));
    app.status.turn
}

#[test]
fn a_finished_turn_closes_with_one_summary_row_on_screen() {
    let mut app = app();
    assert_eq!(finished_turn(&mut app), TurnState::Idle);
    let buffer = render(&mut app, 80, 24);
    let screen = text(&buffer);
    let closing: Vec<&str> = screen
        .lines()
        .filter(|row| row.starts_with("done"))
        .collect();
    assert_eq!(closing.len(), 1, "{screen}");
    assert!(closing[0].contains("2.8k out"), "{screen}");
    assert!(closing[0].contains("3 tool calls"), "{screen}");
}

#[test]
fn a_narrow_screen_elides_the_summary_instead_of_wrapping_it() {
    let mut app = app();
    let _ = finished_turn(&mut app);
    let buffer = render(&mut app, 40, 24);
    let screen = text(&buffer);
    let closing: Vec<&str> = screen
        .lines()
        .filter(|row| row.starts_with("done"))
        .collect();
    assert_eq!(closing.len(), 1, "one row, never two: {screen}");
    assert!(closing[0].chars().count() <= 40, "{screen}");
}

#[test]
fn a_tiny_screen_still_draws_a_failed_summary() {
    let mut app = app();
    type_text(&mut app, "do the thing");
    stroke(&mut app, KeyCode::Enter);
    let _ = app.finish(Err("provider mock failed: 503".to_owned()));
    let buffer = render(&mut app, 12, 24);
    let screen = text(&buffer);
    assert!(!screen.is_empty());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd rust && cargo test -j 2 -p niki-tui-live --test screen summary`
Expected: FAIL — `assert_eq!(closing.len(), 1)` fails with `0`, because no row starts with `done` before Task 3 lands (and this suite catches a wrapped second row if the elision is wrong).

- [ ] **Step 3: Make them pass**

No production change expected. If `a_narrow_screen_elides…` fails with two rows, the `clip` branch in `ui.rs` from Task 1 is not being reached — check that the summary arm sits before the `match row.kind` in `transcript_lines`, not inside it.

- [ ] **Step 4: Run the full gate**

Run:
```bash
cd rust && cargo fmt --all -- --check && cargo clippy -j 2 --workspace --all-targets -- -D warnings && cargo test -j 2 --workspace
```
Expected: clean fmt, no clippy warnings, every test passing.

- [ ] **Step 5: Commit and open the pull request**

```bash
git add rust/crates/niki-tui-live/tests/screen.rs
git commit -m "test(rust): prove the turn summary stays one row on screen"
git push -u origin RavaniRoshan/rust/turn-summary
gh pr create --fill --base niki-agent
gh pr merge --squash --admin --delete-branch
```

---

### Task 5: Changelog, docs mirror, and the live sites

**Files:**
- Modify: `CHANGELOG.md` (new `## [Unreleased]` → `### Added` entry)
- Regenerate: `site/src/pages/changelog/changelog.generated.ts`, `docs-site/docs/changelog.md`

**Interfaces:**
- Consumes: the merged feature.
- Produces: public release notes; no code.

- [ ] **Step 1: Write the entry**

One line under `## [Unreleased]` → `### Added`: `Every turn now ends with one quiet line — how it ended, how long it took, how many tokens it spent, and what it changed.`

- [ ] **Step 2: Regenerate the mirrors**

```bash
cd site && node scripts/sync-changelog.mjs && bun run typecheck && bun run check:brand
cd ../docs-site && npm run sync && npm run check:brand
```
Expected: both gates pass; the diff shows only the changelog mirrors.

- [ ] **Step 3: Commit, merge, deploy**

```bash
git add CHANGELOG.md site/src/pages/changelog/changelog.generated.ts docs-site/docs/changelog.md
git commit -m "docs(repo): note the turn summary line"
```
Then open and squash-merge a `RavaniRoshan/docs/turn-summary` PR, and redeploy both Vercel projects from the prebuilt output (`vercel build --prod` then `vercel deploy --prebuilt --prod`), verifying `curl https://niki-site.vercel.app` and `https://niki-docs.vercel.app/changelog` show the new line.
