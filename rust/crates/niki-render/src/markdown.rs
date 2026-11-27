//! Line-oriented Markdown rendering into styled spans.
//!
//! A growing model answer is rendered by calling [`render_markdown`] with the
//! text so far. The function keeps no state, so a later call with more text
//! produces the same lines the earlier call did, plus whatever was added.
//!
//! Links become `text (url)`: the label carries [`SpanStyle::link`] and the
//! address follows as plain text, which survives terminals and copy-paste
//! alike. [`MarkdownTheme::plain`] sets no colours, so structure can be
//! asserted without a palette.
//!
//! Deliberately outside this renderer: setext headings, indented code blocks,
//! tables, images, HTML, reference links, and backslash escapes. Such input
//! stays literal paragraph text. Blockquotes and list items render their
//! content inline; a block nested inside one is not parsed further. A run of
//! blank lines collapses to one, a single trailing newline adds no blank line,
//! and a tab becomes four spaces. An unordered bullet is drawn with the theme's
//! glyph, so `-`, `*` and `+` look alike; an ordered number is kept as written,
//! and a blockquote's `>` becomes the gutter rather than text. Fence info
//! strings are discarded.

use std::borrow::Cow;

use super::diff::DiffKind;
use super::{Rgb, display_width, unicode_width, wrap};

/// One of the six ATX heading depths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeadingLevel {
    /// `#`.
    H1,
    /// `##`.
    H2,
    /// `###`.
    H3,
    /// `####`.
    H4,
    /// `#####`.
    H5,
    /// `######`.
    H6,
}

impl HeadingLevel {
    /// The level for a run of `depth` hash marks, if it is a heading depth.
    #[must_use]
    pub const fn from_depth(depth: usize) -> Option<Self> {
        match depth {
            1 => Some(Self::H1),
            2 => Some(Self::H2),
            3 => Some(Self::H3),
            4 => Some(Self::H4),
            5 => Some(Self::H5),
            6 => Some(Self::H6),
            _ => None,
        }
    }

    /// How many hash marks this level is written with.
    #[must_use]
    pub const fn depth(self) -> usize {
        match self {
            Self::H1 => 1,
            Self::H2 => 2,
            Self::H3 => 3,
            Self::H4 => 4,
            Self::H5 => 5,
            Self::H6 => 6,
        }
    }
}

/// The attributes a rendered run of text carries.
///
/// A theme decides which attributes a heading or an inline marker sets; the
/// attributes are semantic, not colours, so a plain theme is still meaningful.
/// The one exception is [`color`](Self::color): a surface that knows its
/// palette, such as the diff renderer, may pin a colour to a run, and a plain
/// theme simply leaves it unset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SpanStyle {
    /// Strong emphasis.
    pub bold: bool,
    /// Emphasis.
    pub italic: bool,
    /// Underlined, as a link often is.
    pub underline: bool,
    /// Inline code or a fenced code block.
    pub code: bool,
    /// The label of a link.
    pub link: bool,
    /// Inside a blockquote.
    pub quote: bool,
    /// The heading level this text belongs to, when it belongs to one.
    pub heading: Option<HeadingLevel>,
    /// An explicit colour for this run, when the surface knows one.
    pub color: Option<Rgb>,
}

impl SpanStyle {
    /// The style with no attributes set.
    pub const PLAIN: Self = Self {
        bold: false,
        italic: false,
        underline: false,
        code: false,
        link: false,
        quote: false,
        heading: None,
        color: None,
    };

    /// Pin a palette colour to this run.
    #[must_use]
    pub const fn with_color(mut self, color: Rgb) -> Self {
        self.color = Some(color);
        self
    }

    const fn with_bold(mut self) -> Self {
        self.bold = true;
        self
    }

    const fn with_italic(mut self) -> Self {
        self.italic = true;
        self
    }

    const fn with_underline(mut self) -> Self {
        self.underline = true;
        self
    }

    const fn with_code(mut self) -> Self {
        self.code = true;
        self
    }

    const fn with_link(mut self) -> Self {
        self.link = true;
        self
    }

    const fn with_quote(mut self) -> Self {
        self.quote = true;
        self
    }

    const fn with_heading(mut self, level: HeadingLevel) -> Self {
        self.heading = Some(level);
        self
    }

    /// Add the attributes of an inline marker without dropping the block's
    /// own attributes.
    const fn overlay(self, accent: Self) -> Self {
        Self {
            bold: self.bold || accent.bold,
            italic: self.italic || accent.italic,
            underline: self.underline || accent.underline,
            code: self.code || accent.code,
            link: self.link || accent.link,
            quote: self.quote || accent.quote,
            heading: self.heading,
            color: match self.color {
                Some(color) => Some(color),
                None => accent.color,
            },
        }
    }
}

/// Styles and glyphs for one Markdown surface.
///
/// [`plain`](Self::plain) leaves every colour decision to the caller and keeps
/// only the structural attributes, which makes it the right theme for tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkdownTheme {
    /// Paragraph text.
    pub text: SpanStyle,
    /// One style per heading depth, `H1` first.
    pub headings: [SpanStyle; 6],
    /// A fenced code block.
    pub code: SpanStyle,
    /// An inline code span.
    pub code_span: SpanStyle,
    /// `**strong**`.
    pub strong: SpanStyle,
    /// `*emphasis*`.
    pub emphasis: SpanStyle,
    /// The label of `[label](address)`.
    pub link: SpanStyle,
    /// Text inside a blockquote.
    pub quote: SpanStyle,
    /// The bullet or number that starts a list item.
    pub list_marker: SpanStyle,
    /// A thematic break.
    pub rule: SpanStyle,
    /// The glyph an unordered list item starts with.
    pub bullet: &'static str,
    /// Repeated once per blockquote nesting level at the left edge.
    pub quote_gutter: &'static str,
    /// Repeated to fill the width of a thematic break.
    pub rule_text: &'static str,
}

impl MarkdownTheme {
    /// Structural styling with no palette attached.
    #[must_use]
    pub const fn plain() -> Self {
        Self {
            text: SpanStyle::PLAIN,
            headings: [
                SpanStyle::PLAIN.with_bold().with_heading(HeadingLevel::H1),
                SpanStyle::PLAIN.with_bold().with_heading(HeadingLevel::H2),
                SpanStyle::PLAIN.with_bold().with_heading(HeadingLevel::H3),
                SpanStyle::PLAIN.with_bold().with_heading(HeadingLevel::H4),
                SpanStyle::PLAIN.with_bold().with_heading(HeadingLevel::H5),
                SpanStyle::PLAIN.with_bold().with_heading(HeadingLevel::H6),
            ],
            code: SpanStyle::PLAIN.with_code(),
            code_span: SpanStyle::PLAIN.with_code(),
            strong: SpanStyle::PLAIN.with_bold(),
            emphasis: SpanStyle::PLAIN.with_italic(),
            link: SpanStyle::PLAIN.with_link().with_underline(),
            quote: SpanStyle::PLAIN.with_quote(),
            list_marker: SpanStyle::PLAIN.with_bold(),
            rule: SpanStyle::PLAIN,
            bullet: "-",
            quote_gutter: "│ ",
            rule_text: "─",
        }
    }
}

impl Default for MarkdownTheme {
    fn default() -> Self {
        Self::plain()
    }
}

/// A run of text with one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// The text exactly as it should be drawn.
    pub text: String,
    /// How to draw it.
    pub style: SpanStyle,
}

/// What a [`RenderedLine`] represents in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// A preserved blank line.
    Blank,
    /// Wrapped paragraph text.
    Paragraph,
    /// An ATX heading of the given depth.
    Heading(HeadingLevel),
    /// A list item; `ordered` separates `1.` from `-`.
    ListItem {
        /// Whether the source marker was a number.
        ordered: bool,
    },
    /// A blockquote line, gutter included.
    Quote,
    /// A line inside a fenced code block, verbatim.
    Code,
    /// A line inside a unified diff.
    Diff(DiffKind),
    /// A `@@` hunk header in a unified diff.
    DiffHunk,
    /// A thematic break.
    ThematicBreak,
}

/// One output line: styled spans and their structural role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedLine {
    /// The text, in draw order.
    pub spans: Vec<Span>,
    /// The role of the line.
    pub kind: LineKind,
}

impl RenderedLine {
    /// Concatenate the spans, for callers that only need the plain text.
    #[must_use]
    pub fn text(&self) -> String {
        let mut text = String::new();
        for span in &self.spans {
            text.push_str(&span.text);
        }
        text
    }
}

/// Render Markdown into styled lines no wider than `width`, except for fenced
/// code, which stays verbatim.
#[must_use]
pub fn render_markdown(text: &str, width: usize, theme: &MarkdownTheme) -> Vec<RenderedLine> {
    if text.is_empty() {
        return Vec::new();
    }
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut fence: Option<Fence> = None;
    let mut rest = text;

    loop {
        let (raw, tail) = match rest.find('\n') {
            Some(index) => (&rest[..index], Some(&rest[index + 1..])),
            None => (rest, None),
        };
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let line = expand_tabs(raw);

        if let Some(active) = &fence {
            if fence_closes(&line, active) {
                fence = None;
            } else {
                lines.push(code_line(&line, theme));
            }
        } else if let Some(open) = fence_open(&line) {
            fence = Some(open);
        } else if line.trim().is_empty() {
            push_blank(&mut lines);
        } else if let Some((level, content)) = heading(&line) {
            push_heading(&mut lines, level, content, width, theme);
        } else if let Some((depth, content)) = blockquote(&line) {
            push_quote(&mut lines, depth, content, width, theme);
        } else if is_thematic_break(&line) {
            lines.push(thematic_break(width, theme));
        } else if let Some(item) = list_item(&line, theme) {
            push_list_item(&mut lines, &item, width, theme);
        } else {
            push_paragraph(&mut lines, &line, width, theme);
        }

        match tail {
            Some(next) if !next.is_empty() => rest = next,
            _ => break,
        }
    }

    lines
}

fn expand_tabs(line: &str) -> Cow<'_, str> {
    if line.contains('\t') {
        Cow::Owned(line.replace('\t', "    "))
    } else {
        Cow::Borrowed(line)
    }
}

#[derive(Debug, Clone, Copy)]
struct Fence {
    marker: u8,
    length: usize,
}

fn fence_open(line: &str) -> Option<Fence> {
    let trimmed = line.trim_start();
    let marker = *trimmed.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let length = trimmed.bytes().take_while(|&byte| byte == marker).count();
    if length < 3 {
        return None;
    }
    Some(Fence { marker, length })
}

fn fence_closes(line: &str, fence: &Fence) -> bool {
    let trimmed = line.trim();
    let bytes = trimmed.as_bytes();
    let length = bytes
        .iter()
        .take_while(|&&byte| byte == fence.marker)
        .count();
    length >= fence.length && length == bytes.len()
}

fn code_line(line: &str, theme: &MarkdownTheme) -> RenderedLine {
    let spans = if line.is_empty() {
        Vec::new()
    } else {
        vec![Span {
            text: line.to_owned(),
            style: theme.code,
        }]
    };
    RenderedLine {
        spans,
        kind: LineKind::Code,
    }
}

fn push_blank(lines: &mut Vec<RenderedLine>) {
    if lines.last().is_none_or(|line| line.kind != LineKind::Blank) {
        lines.push(RenderedLine {
            spans: Vec::new(),
            kind: LineKind::Blank,
        });
    }
}

fn heading(line: &str) -> Option<(HeadingLevel, &str)> {
    let trimmed = line.trim_start();
    let depth = trimmed.bytes().take_while(|&byte| byte == b'#').count();
    if depth == 0 || depth > 6 {
        return None;
    }
    let rest = &trimmed[depth..];
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let level = HeadingLevel::from_depth(depth)?;
    Some((level, strip_heading_close(rest.trim())))
}

fn strip_heading_close(text: &str) -> &str {
    let trimmed = text.trim_end();
    let hashes = trimmed
        .bytes()
        .rev()
        .take_while(|&byte| byte == b'#')
        .count();
    if hashes == 0 {
        return trimmed;
    }
    let before = &trimmed[..trimmed.len() - hashes];
    if before.is_empty() || before.ends_with(' ') {
        before.trim_end()
    } else {
        trimmed
    }
}

fn blockquote(line: &str) -> Option<(usize, &str)> {
    let mut rest = line.trim_start();
    let mut depth = 0;
    while let Some(after) = rest.strip_prefix('>') {
        depth += 1;
        rest = after;
        if let Some(stripped) = rest.strip_prefix(' ') {
            rest = stripped;
        }
    }
    (depth > 0).then_some((depth, rest))
}

fn is_thematic_break(line: &str) -> bool {
    let mut marker = None;
    let mut count = 0;
    for ch in line.trim().chars() {
        match ch {
            ' ' | '\t' => {}
            '-' | '*' | '_' => {
                if marker.is_some_and(|seen| seen != ch) {
                    return false;
                }
                marker = Some(ch);
                count += 1;
            }
            _ => return false,
        }
    }
    count >= 3
}

fn thematic_break(width: usize, theme: &MarkdownTheme) -> RenderedLine {
    let glyph = if theme.rule_text.is_empty() {
        "-"
    } else {
        theme.rule_text
    };
    let unit = display_width(glyph).max(1);
    let count = (width / unit).max(1);
    RenderedLine {
        spans: vec![Span {
            text: glyph.repeat(count),
            style: theme.rule,
        }],
        kind: LineKind::ThematicBreak,
    }
}

struct ListItem<'a> {
    ordered: bool,
    prefix: String,
    content: &'a str,
}

fn list_item<'a>(line: &'a str, theme: &MarkdownTheme) -> Option<ListItem<'a>> {
    let indent_len = line.len() - line.trim_start_matches(' ').len();
    let indent = &line[..indent_len];
    let rest = &line[indent_len..];
    let first = rest.chars().next()?;

    if matches!(first, '-' | '*' | '+') {
        let after = &rest[first.len_utf8()..];
        if after.is_empty() {
            return Some(ListItem {
                ordered: false,
                prefix: format!("{indent}{}", theme.bullet),
                content: "",
            });
        }
        let content = after.strip_prefix(' ')?;
        return Some(ListItem {
            ordered: false,
            prefix: format!("{indent}{}", theme.bullet),
            content: content.trim_start_matches(' '),
        });
    }

    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let after_digits = rest.get(digits..)?;
    let after_dot = after_digits.strip_prefix('.')?;
    let number = rest.get(..digits)?;
    if after_dot.is_empty() {
        return Some(ListItem {
            ordered: true,
            prefix: format!("{indent}{number}."),
            content: "",
        });
    }
    let content = after_dot.strip_prefix(' ')?;
    Some(ListItem {
        ordered: true,
        prefix: format!("{indent}{number}."),
        content: content.trim_start_matches(' '),
    })
}

fn push_heading(
    lines: &mut Vec<RenderedLine>,
    level: HeadingLevel,
    content: &str,
    width: usize,
    theme: &MarkdownTheme,
) {
    let base = theme.headings[level.depth() - 1];
    let runs = parse_inline(content, base, theme);
    for spans in styled_lines(&runs, width, base) {
        lines.push(RenderedLine {
            spans,
            kind: LineKind::Heading(level),
        });
    }
}

fn push_paragraph(lines: &mut Vec<RenderedLine>, text: &str, width: usize, theme: &MarkdownTheme) {
    let runs = parse_inline(text, theme.text, theme);
    for spans in styled_lines(&runs, width, theme.text) {
        lines.push(RenderedLine {
            spans,
            kind: LineKind::Paragraph,
        });
    }
}

fn push_quote(
    lines: &mut Vec<RenderedLine>,
    depth: usize,
    content: &str,
    width: usize,
    theme: &MarkdownTheme,
) {
    let gutter = theme.quote_gutter.repeat(depth);
    let gutter_width = display_width(&gutter);
    let inner = width.saturating_sub(gutter_width).max(1);
    let base = theme.quote;
    let runs = parse_inline(content, base, theme);
    for spans in styled_lines(&runs, inner, base) {
        let mut line_spans = Vec::with_capacity(spans.len() + 1);
        if !gutter.is_empty() {
            line_spans.push(Span {
                text: gutter.clone(),
                style: base,
            });
        }
        line_spans.extend(spans);
        lines.push(RenderedLine {
            spans: line_spans,
            kind: LineKind::Quote,
        });
    }
}

fn push_list_item(
    lines: &mut Vec<RenderedLine>,
    item: &ListItem<'_>,
    width: usize,
    theme: &MarkdownTheme,
) {
    let separator = if item.content.is_empty() { "" } else { " " };
    let first_prefix = format!("{}{separator}", item.prefix);
    let hanging_width = display_width(&first_prefix);
    let hanging = " ".repeat(hanging_width);
    let inner = width.saturating_sub(hanging_width).max(1);
    let base = theme.text;
    let runs = parse_inline(item.content, base, theme);

    for (index, spans) in styled_lines(&runs, inner, base).into_iter().enumerate() {
        let mut line_spans = Vec::with_capacity(spans.len() + 1);
        if index == 0 {
            line_spans.push(Span {
                text: first_prefix.clone(),
                style: theme.list_marker,
            });
        } else if !hanging.is_empty() {
            line_spans.push(Span {
                text: hanging.clone(),
                style: base,
            });
        }
        line_spans.extend(spans);
        lines.push(RenderedLine {
            spans: line_spans,
            kind: LineKind::ListItem {
                ordered: item.ordered,
            },
        });
    }
}

#[derive(Debug, Clone)]
struct InlineRun {
    text: String,
    style: SpanStyle,
}

fn push_run(runs: &mut Vec<InlineRun>, text: String, style: SpanStyle) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = runs.last_mut() {
        if last.style == style {
            last.text.push_str(&text);
            return;
        }
    }
    runs.push(InlineRun { text, style });
}

fn flush_plain(plain: &mut String, style: SpanStyle, runs: &mut Vec<InlineRun>) {
    if plain.is_empty() {
        return;
    }
    let text = std::mem::take(plain);
    push_run(runs, text, style);
}

fn append_runs(runs: &mut Vec<InlineRun>, extra: Vec<InlineRun>) {
    for run in extra {
        push_run(runs, run.text, run.style);
    }
}

fn parse_inline(text: &str, base: SpanStyle, theme: &MarkdownTheme) -> Vec<InlineRun> {
    let mut runs = Vec::new();
    let mut plain = String::new();
    let mut index = 0;

    while let Some(ch) = text[index..].chars().next() {
        let rest = &text[index..];

        if ch == '`' {
            let ticks = rest.bytes().take_while(|&byte| byte == b'`').count();
            if let Some((inner, remaining)) = code_span(rest) {
                flush_plain(&mut plain, base, &mut runs);
                push_run(&mut runs, inner.to_owned(), base.overlay(theme.code_span));
                index = text.len() - remaining.len();
                continue;
            }
            plain.push_str(&rest[..ticks]);
            index += ticks;
            continue;
        }

        if ch == '[' {
            if let Some((label, url, remaining)) = link(rest) {
                flush_plain(&mut plain, base, &mut runs);
                let styled = base.overlay(theme.link);
                append_runs(&mut runs, parse_inline(label, styled, theme));
                if !url.is_empty() {
                    let mut address = String::with_capacity(url.len() + 3);
                    address.push_str(" (");
                    address.push_str(url);
                    address.push(')');
                    push_run(&mut runs, address, base);
                }
                index = text.len() - remaining.len();
                continue;
            }
            plain.push('[');
            index += 1;
            continue;
        }

        if let Some(marker) = rest
            .get(..2)
            .filter(|marker| *marker == "**" || *marker == "__")
        {
            let underscore = marker.starts_with("__");
            if !underscore || boundary_before(text, index) {
                if let Some(offset) = find_close(rest, marker) {
                    let close_at = index + offset + 2;
                    if !underscore || boundary_after(text, close_at) {
                        flush_plain(&mut plain, base, &mut runs);
                        let styled = base.overlay(theme.strong);
                        let inner = &rest[2..offset];
                        append_runs(&mut runs, parse_inline(inner, styled, theme));
                        index = close_at;
                        continue;
                    }
                }
            }
            plain.push_str(marker);
            index += 2;
            continue;
        }

        if ch == '*' || ch == '_' {
            let underscore = ch == '_';
            if !underscore || boundary_before(text, index) {
                if let Some(offset) = find_single_close(&rest[ch.len_utf8()..], ch) {
                    let close_at = index + 1 + offset;
                    if !underscore || boundary_after(text, close_at + 1) {
                        flush_plain(&mut plain, base, &mut runs);
                        let styled = base.overlay(theme.emphasis);
                        let inner = &rest[1..1 + offset];
                        append_runs(&mut runs, parse_inline(inner, styled, theme));
                        index = close_at + 1;
                        continue;
                    }
                }
            }
            plain.push(ch);
            index += ch.len_utf8();
            continue;
        }

        plain.push(ch);
        index += ch.len_utf8();
    }

    flush_plain(&mut plain, base, &mut runs);
    runs
}

fn boundary_before(text: &str, index: usize) -> bool {
    text[..index]
        .chars()
        .next_back()
        .is_none_or(|ch| !ch.is_alphanumeric())
}

fn boundary_after(text: &str, index: usize) -> bool {
    text.get(index..)
        .and_then(|rest| rest.chars().next())
        .is_none_or(|ch| !ch.is_alphanumeric())
}

fn find_close(text: &str, marker: &str) -> Option<usize> {
    text[marker.len()..]
        .find(marker)
        .map(|offset| offset + marker.len())
}

fn find_single_close(haystack: &str, marker: char) -> Option<usize> {
    let mut previous = None;
    let mut chars = haystack.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == marker {
            let next_same = chars.peek().is_some_and(|&(_, next)| next == marker);
            if !next_same && previous != Some(marker) {
                return Some(index);
            }
        }
        previous = Some(ch);
    }
    None
}

fn find_backtick_close(text: &str, ticks: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'`' {
            let run = bytes[index..]
                .iter()
                .take_while(|&&byte| byte == b'`')
                .count();
            if run == ticks {
                return Some(index);
            }
            index += run;
        } else {
            index += text[index..].chars().next()?.len_utf8();
        }
    }
    None
}

fn code_span(rest: &str) -> Option<(&str, &str)> {
    let ticks = rest.bytes().take_while(|&byte| byte == b'`').count();
    if ticks == 0 {
        return None;
    }
    let after = &rest[ticks..];
    let offset = find_backtick_close(after, ticks)?;
    Some((&after[..offset], &after[offset + ticks..]))
}

fn link(rest: &str) -> Option<(&str, &str, &str)> {
    let label_end = rest.find(']')?;
    let after = rest.get(label_end + 1..)?;
    let after_open = after.strip_prefix('(')?;
    let url_end = after_open.find(')')?;
    Some((
        rest.get(1..label_end)?,
        after_open.get(..url_end)?,
        after_open.get(url_end + 1..)?,
    ))
}

#[derive(Debug)]
struct Word {
    pieces: Vec<(String, SpanStyle)>,
    width: usize,
}

fn flush_piece(piece: &mut String, style: SpanStyle, pieces: &mut Vec<(String, SpanStyle)>) {
    if piece.is_empty() {
        return;
    }
    let text = std::mem::take(piece);
    if let Some(last) = pieces.last_mut() {
        if last.1 == style {
            last.0.push_str(&text);
            return;
        }
    }
    pieces.push((text, style));
}

fn flush_word(pieces: &mut Vec<(String, SpanStyle)>, words: &mut Vec<Word>) {
    if pieces.is_empty() {
        return;
    }
    let pieces = std::mem::take(pieces);
    let width = pieces
        .iter()
        .map(|(text, _)| display_width(text))
        .sum::<usize>();
    words.push(Word { pieces, width });
}

fn words(runs: &[InlineRun]) -> Vec<Word> {
    let mut words = Vec::new();
    let mut pieces = Vec::new();
    let mut piece = String::new();

    for run in runs {
        for ch in run.text.chars() {
            if ch.is_whitespace() {
                flush_piece(&mut piece, run.style, &mut pieces);
                flush_word(&mut pieces, &mut words);
            } else {
                piece.push(ch);
            }
        }
        flush_piece(&mut piece, run.style, &mut pieces);
    }
    flush_word(&mut pieces, &mut words);
    words
}

fn push_span(spans: &mut Vec<Span>, text: &str, style: SpanStyle) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut() {
        if last.style == style {
            last.text.push_str(text);
            return;
        }
    }
    spans.push(Span {
        text: text.to_owned(),
        style,
    });
}

fn styled_lines(runs: &[InlineRun], width: usize, base: SpanStyle) -> Vec<Vec<Span>> {
    if runs.is_empty() {
        return vec![Vec::new()];
    }
    if runs.len() == 1 && runs[0].style == base {
        return wrap(&runs[0].text, width)
            .into_iter()
            .map(|text| {
                if text.is_empty() {
                    Vec::new()
                } else {
                    vec![Span { text, style: base }]
                }
            })
            .collect();
    }

    let words = words(runs);
    if words.is_empty() {
        return vec![Vec::new()];
    }

    let mut lines = Vec::new();
    let mut current = Vec::new();
    let mut current_width = 0;

    for word in words {
        if current_width > 0 && current_width + 1 + word.width > width {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        if current_width > 0 {
            push_span(&mut current, " ", base);
            current_width += 1;
        }
        if word.width > width {
            push_long_word(&mut lines, &mut current, &mut current_width, word, width);
        } else {
            for (text, style) in word.pieces {
                push_span(&mut current, &text, style);
            }
            current_width += word.width;
        }
    }
    lines.push(current);
    lines
}

fn push_long_word(
    lines: &mut Vec<Vec<Span>>,
    current: &mut Vec<Span>,
    current_width: &mut usize,
    word: Word,
    width: usize,
) {
    for (text, style) in word.pieces {
        for ch in text.chars() {
            let cells = unicode_width(ch);
            if *current_width > 0 && *current_width + cells > width {
                lines.push(std::mem::take(current));
                *current_width = 0;
            }
            let mut buffer = [0_u8; 4];
            push_span(current, ch.encode_utf8(&mut buffer), style);
            *current_width += cells;
        }
    }
}
