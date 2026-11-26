use niki_render::display_width;

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
}

/// One entry in the transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What the row represents, which decides how it is painted.
    pub kind: RowKind,
    /// Tool name or other short label shown before the text.
    pub title: Option<String>,
    /// The text itself, already shortened for display.
    pub text: String,
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

    /// Add something the person typed.
    pub fn push_user(&mut self, text: impl Into<String>) {
        self.streaming = false;
        self.rows.push(Row {
            kind: RowKind::User,
            title: None,
            text: text.into(),
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
        });
    }

    /// Record a tool call the model asked for.
    pub fn push_tool_call(&mut self, name: &str, arguments: &str) {
        self.streaming = false;
        self.rows.push(Row {
            kind: RowKind::ToolCall,
            title: Some(name.to_owned()),
            text: one_line(arguments),
        });
    }

    /// Record what a tool returned.
    pub fn push_tool_result(&mut self, name: &str, output: &str, is_error: bool) {
        self.streaming = false;
        self.rows.push(Row {
            kind: if is_error {
                RowKind::Error
            } else {
                RowKind::ToolResult
            },
            title: Some(name.to_owned()),
            text: shorten(output),
        });
    }

    /// Record a note from the app.
    pub fn push_notice(&mut self, text: impl Into<String>) {
        self.streaming = false;
        self.rows.push(Row {
            kind: RowKind::Notice,
            title: None,
            text: text.into(),
        });
    }

    /// Record a failure the person has to know about.
    pub fn push_error(&mut self, text: impl Into<String>) {
        self.streaming = false;
        self.rows.push(Row {
            kind: RowKind::Error,
            title: None,
            text: text.into(),
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
