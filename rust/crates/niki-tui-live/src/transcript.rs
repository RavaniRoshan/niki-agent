use niki_render::{RenderedLine, display_width};

/// What one row of the transcript represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Something the person typed.
    User,
    /// Model answer text, which streams in.
    Assistant,
    /// A tool the model asked for, before it ran.
    ToolCall,
    /// What a tool returned.
    ToolResult,
    /// A failed call, a refused command, or a failed turn.
    Error,
    /// A note from the app itself: a step limit, a cancellation.
    Notice,
    /// A file's diff, drawn line for line instead of wrapped.
    Diff,
    /// The line that closes a turn: how it ended and what it cost.
    Summary,
    /// The same line for a turn that failed, painted in the error tone.
    SummaryFailed,
}

/// One entry in the transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What the row represents, which decides how it is painted.
    pub kind: RowKind,
    /// Tool name, file path, or other short label shown before the text.
    pub title: Option<String>,
    /// The text itself, already shortened for display; empty for a diff row.
    pub text: String,
    /// Pre-rendered lines for a row that must not be wrapped.
    pub rendered: Option<Vec<RenderedLine>>,
}

/// The conversation as the screen shows it.
///
/// The model's answer streams in one delta at a time, so an assistant row grows
/// in place until a tool call, a note or a new turn ends it. Rows are only ever
/// appended, which is what keeps a cancelled turn's output on screen
/// (RULES.md 6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Transcript {
    rows: Vec<Row>,
    streaming: bool,
}

/// A tool result longer than this is shortened: a shell command can print more
/// than a person can read, and the model already has the whole text.
const MAX_RESULT_LINES: usize = 120;
/// Longest single tool result kept, in characters.
const MAX_RESULT_CHARS: usize = 8_000;

impl Transcript {
    /// The rows in arrival order.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Whether nothing has happened yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Whether anyone has actually spoken yet.
    ///
    /// Notices are the app talking to itself — a trust warning, a theme
    /// change — so they do not count. The first screen stays a welcome until a
    /// real row arrives, and a warning never takes it away.
    #[must_use]
    pub fn has_conversation(&self) -> bool {
        self.rows.iter().any(|row| row.kind != RowKind::Notice)
    }

    /// Add something the person typed.
    pub fn push_user(&mut self, text: impl Into<String>) {
        self.streaming = false;
        self.rows.push(Row {
            kind: RowKind::User,
            title: None,
            text: text.into(),
            rendered: None,
        });
    }

    /// Append answer text, growing the streaming row or starting a new one.
    pub fn push_assistant(&mut self, delta: &str) {
        let appended = match self.rows.last_mut() {
            Some(last) if self.streaming && last.kind == RowKind::Assistant => {
                last.text.push_str(delta);
                true
            }
            _ => false,
        };
        if appended {
            return;
        }
        self.streaming = true;
        self.rows.push(Row {
            kind: RowKind::Assistant,
            title: None,
            text: delta.to_owned(),
            rendered: None,
        });
    }

    /// Record a tool call the model asked for.
    pub fn push_tool_call(&mut self, name: &str, arguments: &str) {
        self.streaming = false;
        self.rows.push(Row {
            kind: RowKind::ToolCall,
            title: Some(name.to_owned()),
            text: one_line(arguments),
            rendered: None,
        });
    }

    /// Record what a tool returned.
    pub fn push_tool_result(&mut self, name: &str, output: &str, is_error: bool) {
        self.streaming = false;
        // An empty result is evidence: the call ran and printed nothing. A
        // blank row would read as a lost event instead.
        let text = if output.trim().is_empty() {
            "(no output)".to_owned()
        } else {
            shorten(output)
        };
        self.rows.push(Row {
            kind: if is_error {
                RowKind::Error
            } else {
                RowKind::ToolResult
            },
            title: Some(name.to_owned()),
            text,
            rendered: None,
        });
    }

    /// Record a note from the app.
    pub fn push_notice(&mut self, text: impl Into<String>) {
        self.push_closing(RowKind::Notice, text);
    }

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

    /// Record a file's diff under its path, drawn without wrapping.
    pub fn push_diff(&mut self, path: &str, rendered: Vec<RenderedLine>) {
        self.streaming = false;
        self.rows.push(Row {
            kind: RowKind::Diff,
            title: Some(path.to_owned()),
            text: String::new(),
            rendered: Some(rendered),
        });
    }

    /// Record a failure the person has to know about.
    pub fn push_error(&mut self, text: impl Into<String>) {
        self.streaming = false;
        self.rows.push(Row {
            kind: RowKind::Error,
            title: None,
            text: text.into(),
            rendered: None,
        });
    }

    /// End the streaming assistant row, if one is open.
    pub fn end_stream(&mut self) {
        self.streaming = false;
    }

    /// Whether answer text is still being appended to the last row.
    #[must_use]
    pub fn is_streaming(&self) -> bool {
        self.streaming
    }

    /// The index of the newest row the person typed.
    #[must_use]
    pub fn last_user(&self) -> Option<usize> {
        self.rows.iter().rposition(|row| row.kind == RowKind::User)
    }

    /// Drop every row from `index` on, and return how many left.
    ///
    /// The store is not touched: restoring a prompt to the composer is a view
    /// change, and the saved session keeps what happened.
    pub fn truncate(&mut self, index: usize) -> usize {
        if index >= self.rows.len() {
            return 0;
        }
        let dropped = self.rows.len() - index;
        self.rows.truncate(index);
        self.streaming = false;
        dropped
    }

    /// Empty the view for a new session.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.streaming = false;
    }

    /// Keep only the newest `keep` rows and return how many left the view.
    ///
    /// This is a view-level fold: the session store is not touched, so a
    /// resume brings back everything the view dropped.
    pub fn fold(&mut self, keep: usize) -> usize {
        let dropped = self.rows.len().saturating_sub(keep);
        if dropped > 0 {
            self.rows.drain(..dropped);
            self.streaming = false;
        }
        dropped
    }
}

/// First line of a multi-line argument string, so a tool call stays on one row.
fn one_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() > 160 {
        format!("{}...", line.chars().take(157).collect::<String>())
    } else {
        line.to_owned()
    }
}

/// Shorten a tool result to something a person can read.
fn shorten(text: &str) -> String {
    let mut kept = String::new();
    let total = text.lines().count();
    for (lines, line) in text.lines().enumerate() {
        if lines == MAX_RESULT_LINES
            || display_width(&kept) + display_width(line) > MAX_RESULT_CHARS
        {
            kept.push_str(&format!(
                "\n... {} more lines, which the model has already read",
                total.saturating_sub(lines)
            ));
            return kept;
        }
        if lines > 0 {
            kept.push('\n');
        }
        kept.push_str(line);
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_text_streams_into_one_row() {
        let mut transcript = Transcript::default();
        transcript.push_user("hi");
        for delta in ["Hel", "lo, ", "world"] {
            transcript.push_assistant(delta);
        }
        assert_eq!(transcript.rows().len(), 2);
        assert_eq!(transcript.rows()[0].kind, RowKind::User);
        assert_eq!(transcript.rows()[1].kind, RowKind::Assistant);
        assert_eq!(transcript.rows()[1].text, "Hello, world");
        assert!(transcript.is_streaming());
        transcript.end_stream();
        transcript.push_assistant("again");
        assert_eq!(transcript.rows().len(), 3, "a new turn starts a new row");
    }

    #[test]
    fn a_tool_call_ends_the_answer_row() {
        let mut transcript = Transcript::default();
        transcript.push_assistant("let me look");
        transcript.push_tool_call("shell", "{\"command\":\"ls\"}");
        transcript.push_assistant("found it");
        let rows = transcript.rows();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].kind, RowKind::ToolCall);
        assert_eq!(rows[1].title.as_deref(), Some("shell"));
        assert_eq!(rows[1].text, "{\"command\":\"ls\"}");
        assert_eq!(rows[2].kind, RowKind::Assistant);
    }

    #[test]
    fn a_failed_tool_result_is_an_error_row() {
        let mut transcript = Transcript::default();
        transcript.push_tool_result("shell", "refused: no", true);
        transcript.push_tool_result("read_file", "contents", false);
        assert_eq!(transcript.rows()[0].kind, RowKind::Error);
        assert_eq!(transcript.rows()[1].kind, RowKind::ToolResult);
        assert_eq!(transcript.rows()[1].title.as_deref(), Some("read_file"));
    }

    #[test]
    fn long_tool_output_is_shortened_with_a_count() {
        let text = (0..200)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>();
        let shortened = shorten(&text.join("\n"));
        assert_eq!(shortened.lines().count(), MAX_RESULT_LINES + 1);
        assert!(
            shortened.ends_with("... 80 more lines, which the model has already read"),
            "{shortened}"
        );
        assert_eq!(shorten("one line"), "one line");
        assert_eq!(shorten(""), "");
    }

    #[test]
    fn only_the_first_line_of_arguments_is_kept() {
        assert_eq!(one_line("{\n  \"a\": 1\n}"), "{");
        let long = "x".repeat(200);
        let shortened = one_line(&long);
        assert_eq!(shortened.chars().count(), 160);
        assert!(shortened.ends_with("..."));
    }

    #[test]
    fn last_user_finds_the_newest_prompt_and_truncate_drops_from_it() {
        let mut transcript = Transcript::default();
        transcript.push_user("first");
        transcript.push_assistant("answer");
        transcript.push_user("second");
        transcript.push_assistant("another");
        assert_eq!(transcript.last_user(), Some(2));

        assert_eq!(transcript.truncate(2), 2);
        let rows = transcript.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].text, "answer");
        assert_eq!(transcript.truncate(9), 0, "nothing past the end to drop");

        let empty = Transcript::default();
        assert_eq!(empty.last_user(), None);
    }

    #[test]
    fn notices_alone_are_not_a_conversation() {
        let mut transcript = Transcript::default();
        assert!(!transcript.has_conversation());
        transcript.push_notice("warning: this project is not trusted");
        assert!(
            !transcript.has_conversation(),
            "a warning is the app talking, not the person"
        );
        transcript.push_user("hi");
        assert!(transcript.has_conversation());
    }

    #[test]
    fn clear_empties_the_view_and_ends_the_stream() {
        let mut transcript = Transcript::default();
        transcript.push_user("hi");
        transcript.push_assistant("there");
        transcript.clear();
        assert!(transcript.is_empty());
        assert!(!transcript.is_streaming());
        transcript.push_assistant("new");
        assert_eq!(transcript.rows().len(), 1, "a new answer starts a row");
    }

    #[test]
    fn a_summary_row_closes_the_stream_and_is_not_a_notice() {
        let mut transcript = Transcript::default();
        transcript.push_assistant("half");
        transcript.push_summary("done · 14s");
        assert!(!transcript.is_streaming());
        assert_eq!(
            transcript.rows().last().map(|row| row.kind),
            Some(RowKind::Summary)
        );
        assert!(
            transcript.has_conversation(),
            "a summary is the app closing a turn, not furniture"
        );
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

    #[test]
    fn fold_keeps_the_newest_rows_and_reports_how_many_left() {
        let mut transcript = Transcript::default();
        for index in 0..20 {
            transcript.push_user(format!("row {index}"));
        }
        assert_eq!(transcript.fold(5), 15);
        assert_eq!(transcript.rows().len(), 5);
        assert_eq!(transcript.rows()[0].text, "row 15");
        assert_eq!(transcript.fold(5), 0, "a short view folds nothing");
        assert_eq!(transcript.rows().len(), 5);
    }
}
