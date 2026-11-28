//! Transcript view model and deterministic frame renderer.
//!
//! Replay and live sessions share this crate: both feed protocol frames into
//! [`TranscriptModel`], and both draw the resulting lines. Keeping drawing out
//! of the terminal backend is what makes the M1 parity frames comparable in a
//! test.

use niki_proto::{EventName, Frame, Transcript, message_content, message_role};
use serde_json::Value;

pub mod diff;
mod markdown;

pub use diff::{DiffKind, DiffLine, DiffTheme, FileDiff, Hunk, parse_unified, render_diff};
pub use markdown::{
    HeadingLevel, LineKind, MarkdownTheme, RenderedLine, Span, SpanStyle, render_markdown,
};

/// An sRGB colour attached to a rendered run of text.
///
/// Kept as plain data: this crate never draws, so it carries the colour the
/// theme chose and leaves the conversion to the backend's own colour type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb {
    /// Build a colour from its channels.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Glyphs with an ASCII fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyphs {
    pub user: &'static str,
    pub assistant: &'static str,
    pub tool: &'static str,
    pub ok: &'static str,
    pub failed: &'static str,
    pub separator: &'static str,
    pub cursor: &'static str,
}

pub const UNICODE_GLYPHS: Glyphs = Glyphs {
    user: "›",
    assistant: "⏺",
    tool: "⚙",
    ok: "✓",
    failed: "×",
    separator: "─",
    cursor: "▌",
};

pub const ASCII_GLYPHS: Glyphs = Glyphs {
    user: ">",
    assistant: "*",
    tool: "#",
    ok: "+",
    failed: "x",
    separator: "-",
    cursor: "|",
};

impl Glyphs {
    /// Pick the registry for a terminal that cannot render Unicode.
    #[must_use]
    pub const fn for_ascii(ascii: bool) -> Self {
        if ascii { ASCII_GLYPHS } else { UNICODE_GLYPHS }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    Tool,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

/// The scrollback the renderer draws.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranscriptModel {
    pub messages: Vec<Message>,
    pub status: Option<String>,
    streaming: bool,
}

impl TranscriptModel {
    /// Build a model from a committed protocol transcript.
    #[must_use]
    pub fn from_transcript(transcript: &Transcript) -> Self {
        let mut model = Self::default();
        for frame in &transcript.frames {
            model.apply(frame);
        }
        model
    }

    /// Fold one protocol frame into the transcript.
    pub fn apply(&mut self, frame: &Frame) {
        match frame.event {
            EventName::Values => self.replace_from_values(&frame.data),
            EventName::MessagesPartial => self.apply_partial(&frame.data),
            EventName::MessagesComplete | EventName::MessagesTuple => {
                self.apply_complete(&frame.data);
            }
            EventName::Updates => self.apply_updates(&frame.data),
            EventName::Error => {
                self.status = Some(format!("engine error: {}", frame.data));
            }
            EventName::End => self.streaming = false,
            EventName::Metadata | EventName::MessagesMetadata | EventName::Events => {}
        }
    }

    fn replace_from_values(&mut self, data: &Value) {
        let Some(messages) = data.get("messages").and_then(Value::as_array) else {
            return;
        };
        self.messages = messages.iter().filter_map(message_from_value).collect();
        self.streaming = false;
    }

    fn apply_partial(&mut self, data: &Value) {
        let Some(chunk) = data.as_array().and_then(|items| items.first()) else {
            return;
        };
        let Some(text) = message_content(chunk) else {
            return;
        };
        match self.messages.last_mut() {
            Some(last) if last.role == Role::Assistant && self.streaming => last.text = text,
            _ => {
                self.messages.push(Message {
                    role: Role::Assistant,
                    text,
                });
                self.streaming = true;
            }
        }
    }

    fn apply_complete(&mut self, data: &Value) {
        let message = data
            .as_array()
            .and_then(|items| items.first())
            .unwrap_or(data);
        if let Some(message) = message_from_value(message) {
            self.upsert(message);
        }
        self.streaming = false;
    }

    fn apply_updates(&mut self, data: &Value) {
        let Some(nodes) = data.as_object() else {
            return;
        };
        for update in nodes.values() {
            let Some(messages) = update.get("messages").and_then(Value::as_array) else {
                continue;
            };
            for message in messages {
                if let Some(message) = message_from_value(message) {
                    self.upsert(message);
                }
            }
        }
        self.streaming = false;
    }

    fn upsert(&mut self, message: Message) {
        match self.messages.last_mut() {
            Some(last) if last.role == message.role && self.streaming => *last = message,
            _ => self.messages.push(message),
        }
    }

    /// Render the transcript into at most `height` lines of exactly `width`
    /// terminal cells (plain text; styling is the backend's job).
    #[must_use]
    pub fn render(&self, width: usize, height: usize, glyphs: Glyphs) -> Screen {
        self.render_labelled(width, height, glyphs, "replay")
    }

    /// Render with an explicit corner label, e.g. `live` for an engine session.
    #[must_use]
    pub fn render_labelled(
        &self,
        width: usize,
        height: usize,
        glyphs: Glyphs,
        label: &str,
    ) -> Screen {
        let width = width.max(8);
        let height = height.max(3);
        let mut lines = Vec::with_capacity(height);

        let title = "Niki Agent";
        let right = label;
        let mut header = String::from(title);
        let padding = width.saturating_sub(title.chars().count() + right.chars().count());
        header.push_str(&" ".repeat(padding.saturating_sub(1)));
        header.push_str(right);
        lines.push(pad(&header, width));
        lines.push(glyphs.separator.repeat(width));

        let body_height = height.saturating_sub(3);
        let mut body: Vec<String> = Vec::new();
        for message in &self.messages {
            let (prefix, continuation) = match message.role {
                Role::User => (format!("{} ", glyphs.user), "  "),
                Role::Assistant => (format!("{} ", glyphs.assistant), "  "),
                Role::Tool => (format!("{} ", glyphs.tool), "  "),
                Role::System => ("! ".to_owned(), "  "),
            };
            let wrap_width = width.saturating_sub(prefix.chars().count()).max(1);
            for (index, line) in wrap(&message.text, wrap_width).into_iter().enumerate() {
                let lead = if index == 0 { &prefix } else { continuation };
                body.push(pad(&format!("{lead}{line}"), width));
            }
            body.push(" ".repeat(width));
        }
        if let Some(status) = &self.status {
            body.push(pad(&format!("! {status}"), width));
        }

        // Show the tail: a transcript scrolls, so the newest lines win. A body
        // shorter than the region is padded below so the footer stays put.
        let start = body.len().saturating_sub(body_height);
        let mut shown: Vec<String> = body.into_iter().skip(start).take(body_height).collect();
        while shown.len() < body_height {
            shown.push(" ".repeat(width));
        }
        lines.extend(shown);

        // Hints drop the parts a narrow frame cannot show rather than
        // truncating mid-word.
        let footer = if self.messages.is_empty() {
            if width >= 60 {
                format!(
                    "{} Type a message, / for commands, @ for files",
                    glyphs.user
                )
            } else {
                format!("{} Type a message, / for commands", glyphs.user)
            }
        } else if width >= 60 {
            "ctrl+c cancel · ctrl+p palette · / commands".to_owned()
        } else {
            "ctrl+c cancel · / commands".to_owned()
        };
        lines.push(pad(&footer, width));

        lines.truncate(height);
        Screen { lines }
    }
}

fn message_from_value(value: &Value) -> Option<Message> {
    let role = match message_role(value) {
        "human" | "user" => Role::User,
        "ai" | "AIMessageChunk" | "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        "system" => Role::System,
        _ => return None,
    };
    let text = message_content(value).unwrap_or_default();
    if text.is_empty() && role == Role::Tool {
        return Some(Message {
            role,
            text: "(tool result)".to_owned(),
        });
    }
    Some(Message { role, text })
}

fn pad(line: &str, width: usize) -> String {
    let mut out = String::with_capacity(width);
    let mut cells = 0;
    for ch in line.chars() {
        let width_of = unicode_width(ch);
        if cells + width_of > width {
            break;
        }
        out.push(ch);
        cells += width_of;
    }
    out.push_str(&" ".repeat(width.saturating_sub(cells)));
    out
}

/// Terminal cells a character occupies: 0 for controls, 4 for a tab, and the
/// Unicode east-asian width otherwise.
fn unicode_width(ch: char) -> usize {
    match ch {
        '\t' => 4,
        c if c.is_control() => 0,
        c => unicode_width::UnicodeWidthChar::width(c).unwrap_or(1),
    }
}

/// Terminal cells a string occupies.
#[must_use]
pub fn display_width(text: &str) -> usize {
    text.chars().map(unicode_width).sum()
}

/// Split `text` into lines no wider than `width` terminal cells, breaking on
/// whitespace and splitting a word that is longer than a line.
#[must_use]
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut current = String::new();
        let mut current_width = 0;
        for word in paragraph.split_whitespace() {
            let word_width = word.chars().map(unicode_width).sum::<usize>();
            if current_width > 0 && current_width + 1 + word_width > width {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            if word_width > width {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                }
                let mut chunk = String::new();
                let mut cells = 0;
                for ch in word.chars() {
                    let w = unicode_width(ch);
                    if cells + w > width {
                        lines.push(std::mem::take(&mut chunk));
                        cells = 0;
                    }
                    chunk.push(ch);
                    cells += w;
                }
                current = chunk;
                current_width = cells;
                continue;
            }
            if current_width > 0 {
                current.push(' ');
                current_width += 1;
            }
            current.push_str(word);
            current_width += word_width;
        }
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// A rendered plain-text screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub lines: Vec<String>,
}

impl Screen {
    #[must_use]
    pub fn to_text(&self) -> String {
        self.lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO: &str = include_str!("../../niki-proto/fixtures/hello.json");

    fn hello() -> TranscriptModel {
        TranscriptModel::from_transcript(&Transcript::from_json(HELLO).unwrap())
    }

    #[test]
    fn replay_produces_the_user_and_assistant_turns() {
        let model = hello();
        assert_eq!(model.messages.len(), 2);
        assert_eq!(model.messages[0].role, Role::User);
        assert_eq!(model.messages[0].text, "Say hello");
        assert_eq!(model.messages[1].role, Role::Assistant);
        assert_eq!(
            model.messages[1].text,
            "Hello from the Niki protocol fixture."
        );
    }

    #[test]
    fn render_is_deterministic_and_sized() {
        let model = hello();
        for (width, height) in [(120, 38), (80, 24), (40, 20)] {
            let first = model.render(width, height, UNICODE_GLYPHS);
            let second = model.render(width, height, UNICODE_GLYPHS);
            assert_eq!(first, second);
            assert_eq!(first.lines.len(), height);
            for line in &first.lines {
                assert_eq!(display_width(line), width, "line must be padded");
            }
            assert!(first.to_text().contains("Say hello"));
            assert!(first.to_text().contains("Hello from the Niki"));
        }
    }

    #[test]
    fn ascii_glyphs_replace_every_marker() {
        let model = hello();
        let unicode = model.render(80, 24, UNICODE_GLYPHS).to_text();
        let ascii = model.render(80, 24, ASCII_GLYPHS).to_text();
        assert!(unicode.contains('›'));
        assert!(!ascii.contains('›'));
        assert!(ascii.contains("> Say hello"));
    }

    #[test]
    fn long_lines_wrap_without_exceeding_the_width() {
        let model = TranscriptModel {
            messages: vec![Message {
                role: Role::Assistant,
                text: "word ".repeat(200),
            }],
            status: None,
            streaming: false,
        };
        let frame = model.render(40, 20, ASCII_GLYPHS);
        for line in &frame.lines {
            assert!(display_width(line) <= 40);
        }
    }

    /// The M1 parity frames. Every line is pinned by the M1 replay fixtures;
    /// a change here is a rendering change and must update that table.
    #[test]
    fn parity_frames_are_pinned() {
        let model = hello();
        let cases = [
            (120, 38, "ctrl+c cancel · ctrl+p palette · / commands"),
            (80, 24, "ctrl+c cancel · ctrl+p palette · / commands"),
            (40, 20, "ctrl+c cancel · / commands"),
        ];
        for (width, height, last) in cases {
            let frame = model.render(width, height, UNICODE_GLYPHS);
            let lines: Vec<&str> = frame.lines.iter().map(|line| line.trim_end()).collect();
            assert_eq!(lines.len(), height, "{width}x{height}");
            assert!(
                lines[0].starts_with("Niki Agent"),
                "{width}x{height} header"
            );
            assert!(
                lines[0].ends_with("replay"),
                "{width}x{height} header label"
            );
            assert_eq!(lines[1], "─".repeat(width), "{width}x{height} separator");
            assert_eq!(lines[2], "› Say hello", "{width}x{height} user turn");
            assert_eq!(
                lines[4], "⏺ Hello from the Niki protocol fixture.",
                "{width}x{height} assistant turn"
            );
            assert_eq!(lines[height - 1], last, "{width}x{height} footer");
        }
    }

    #[test]
    fn an_empty_transcript_shows_the_composer_placeholder() {
        let model = TranscriptModel::default();
        let frame = model.render(80, 24, UNICODE_GLYPHS);
        let text = frame.to_text();
        assert!(text.contains("› Type a message, / for commands, @ for files"));
        assert!(!text.contains("ctrl+c"));
        let narrow = model.render(40, 20, ASCII_GLYPHS);
        assert!(
            narrow
                .to_text()
                .contains("> Type a message, / for commands")
        );
        assert!(!narrow.to_text().contains("@ for files"));
    }

    #[test]
    fn partial_chunks_replace_the_streaming_message() {
        let mut model = TranscriptModel::default();
        let partial = |text: &str| Frame {
            event: EventName::MessagesPartial,
            data: serde_json::json!([{"type": "ai", "content": text}]),
        };
        model.apply(&partial("Hel"));
        model.apply(&partial("Hello"));
        assert_eq!(model.messages.len(), 1);
        assert_eq!(model.messages[0].text, "Hello");
    }
}
