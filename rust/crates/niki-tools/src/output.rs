/// Hard cap on the text a tool returns to the model.
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Hard cap on the lines a tool returns to the model.
pub const MAX_OUTPUT_LINES: usize = 2_000;

/// Text returned for one tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    /// Text the model reads.
    pub text: String,
    /// Whether the text was cut by a cap.
    pub truncated: bool,
}

impl ToolOutput {
    /// Build an output, applying the line and byte caps and appending a marker
    /// when anything was cut.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let (text, truncated) = cap(text.into());
        Self { text, truncated }
    }
}

fn marker() -> String {
    format!("\n\n[output truncated: exceeded {MAX_OUTPUT_LINES} lines or {MAX_OUTPUT_BYTES} bytes]")
}

fn cap(text: String) -> (String, bool) {
    if text.len() <= MAX_OUTPUT_BYTES && text.lines().count() <= MAX_OUTPUT_LINES {
        return (text, false);
    }

    let marker = marker();
    let budget = MAX_OUTPUT_BYTES.saturating_sub(marker.len());
    let mut end = floor_char_boundary(&text, text.len().min(budget));

    let mut newlines = 0usize;
    for (index, byte) in text[..end].bytes().enumerate() {
        if byte == b'\n' {
            newlines += 1;
            if newlines == MAX_OUTPUT_LINES {
                end = index + 1;
                break;
            }
        }
    }

    if end >= text.len() {
        return (text, false);
    }

    let mut out = String::with_capacity(end + marker.len());
    out.push_str(&text[..end]);
    out.push_str(&marker);
    (out, true)
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}
