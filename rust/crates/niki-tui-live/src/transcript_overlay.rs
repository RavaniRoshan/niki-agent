//! The whole conversation on one screen, scrolled with the keyboard.
//!
//! The live viewport shows only the rows that fit above the composer. This
//! overlay paints every row from the first prompt to the newest answer, using
//! the same rows the screen draws: an assistant row goes through
//! [`render_markdown`], and a diff row uses the lines [`Row::rendered`] already
//! holds.
//!
//! It is state plus a key handler, so a test can open it, move it and close it
//! without a terminal. Drawing happens only when the event loop has something
//! to draw; nothing here schedules a repaint.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use niki_render::{Glyphs, MarkdownTheme, SpanStyle, display_width, render_markdown, wrap};
use niki_tui_themes::{GlyphMode, Palette};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::transcript::{Row, RowKind};
use crate::ui;

/// Rows a page key moves before the first draw has sized the view.
const DEFAULT_PAGE: usize = 10;
/// The footer hint on a terminal that draws Unicode.
pub const FOOTER: &str = "Esc close · ↑↓ scroll · PgUp/PgDn page";
/// The footer hint on a terminal that draws only ASCII.
pub const ASCII_FOOTER: &str = "Esc close | Up/Down scroll | PgUp/PgDn page";

/// The overlay's scroll position and the height it had at the last draw.
///
/// `offset` counts lines above the newest one, as [`crate::Scroll`] does, so a
/// fresh overlay opens at the bottom. [`usize::MAX`] is the request to pin the
/// view to the top; the next draw clamps it to the oldest line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptOverlay {
    offset: usize,
    page: usize,
}

impl Default for TranscriptOverlay {
    fn default() -> Self {
        Self {
            offset: 0,
            page: DEFAULT_PAGE,
        }
    }
}

impl TranscriptOverlay {
    /// A fresh overlay, showing the newest line.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Lines above the newest one that the top of the view shows.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// The footer hint for the terminal's glyph mode.
    #[must_use]
    pub fn footer(glyphs: GlyphMode) -> &'static str {
        if glyphs.is_ascii() {
            ASCII_FOOTER
        } else {
            FOOTER
        }
    }

    /// Scroll back by `lines`.
    pub fn scroll_up(&mut self, lines: usize) {
        self.offset = self.offset.saturating_add(lines.max(1));
    }

    /// Scroll forward by `lines`, never past the newest line.
    pub fn scroll_down(&mut self, lines: usize) {
        self.offset = self.offset.saturating_sub(lines.max(1));
    }

    /// Handle one key press. Returns whether the overlay stays open.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => return false,
            KeyCode::Char('t') if control => return false,
            KeyCode::Up => self.scroll_up(1),
            KeyCode::Down => self.scroll_down(1),
            KeyCode::PageUp => self.scroll_up(self.page),
            KeyCode::PageDown => self.scroll_down(self.page),
            KeyCode::Home | KeyCode::Char('g') if !control => self.offset = usize::MAX,
            KeyCode::End | KeyCode::Char('G') => self.offset = 0,
            _ => {}
        }
        true
    }

    /// The heading: the overlay's name and the first line it shows.
    #[must_use]
    pub fn header(&self, lines: usize, height: usize) -> String {
        if lines == 0 {
            return "transcript  line 0/0".to_owned();
        }
        format!("transcript  line {}/{lines}", self.start(lines, height) + 1)
    }

    /// Index of the first line the viewport shows.
    fn start(&self, lines: usize, height: usize) -> usize {
        lines
            .saturating_sub(height.max(1))
            .saturating_sub(self.offset)
    }
}

/// Draw the overlay over the whole screen.
pub(crate) fn draw(
    frame: &mut Frame,
    area: Rect,
    overlay: &mut TranscriptOverlay,
    rows: &[Row],
    palette: &Palette,
    glyphs: GlyphMode,
) {
    if area.is_empty() {
        return;
    }
    let mut block = Block::bordered()
        .border_set(glyphs.border())
        .border_style(palette.accent_style())
        .style(palette.panel_style());
    let inner = block.inner(area);
    let width = usize::from(inner.width).max(1);
    let lines = lines(rows, width, palette, glyphs);

    let footer_height = u16::from(inner.height > 1);
    let body_height = inner.height.saturating_sub(footer_height);
    let height = usize::from(body_height).max(1);
    overlay.page = height;
    overlay.offset = overlay.offset.min(lines.len().saturating_sub(height));

    block = block.title(Line::from(Span::styled(
        overlay.header(lines.len(), height),
        palette.accent_style(),
    )));
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if body_height == 0 {
        return;
    }

    let body = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: body_height,
    };
    let start = overlay.start(lines.len(), height);
    let visible: Vec<Line<'static>> = lines.into_iter().skip(start).take(height).collect();
    frame.render_widget(Paragraph::new(visible).style(palette.panel_style()), body);

    if footer_height == 1 {
        let footer = Rect {
            x: inner.x,
            y: inner.y.saturating_add(body_height),
            width: inner.width,
            height: 1,
        };
        let hint = Line::from(Span::styled(
            TranscriptOverlay::footer(glyphs),
            palette.muted_style(),
        ));
        frame.render_widget(Paragraph::new(hint).style(palette.panel_style()), footer);
    }
}

/// Every row as display lines, each block separated by a blank line.
fn lines(rows: &[Row], width: usize, palette: &Palette, glyphs: GlyphMode) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return vec![Line::from(Span::styled(
            "nothing yet",
            palette.muted_style(),
        ))];
    }
    let marks = Glyphs::for_ascii(glyphs.is_ascii());
    let theme = markdown_theme(palette, glyphs);
    let mut lines = Vec::new();
    for row in rows {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        match row.kind {
            RowKind::Assistant => push_markdown(&mut lines, row, width, &theme, marks, palette),
            RowKind::Diff => push_diff(&mut lines, row, width, palette),
            _ => push_plain(&mut lines, row, width, marks, palette),
        }
    }
    lines
}

/// An assistant row, rendered from its Markdown source.
fn push_markdown(
    lines: &mut Vec<Line<'static>>,
    row: &Row,
    width: usize,
    theme: &MarkdownTheme,
    marks: Glyphs,
    palette: &Palette,
) {
    let marker = format!("{} ", marks.assistant);
    let indent = " ".repeat(display_width(&marker));
    let wrap_width = width.saturating_sub(display_width(&marker)).max(1);
    let rendered = render_markdown(&row.text, wrap_width, theme);
    if rendered.is_empty() {
        lines.push(Line::from(Span::styled(marker, palette.muted_style())));
        return;
    }
    for (index, line) in rendered.iter().enumerate() {
        let lead = if index == 0 {
            marker.clone()
        } else {
            indent.clone()
        };
        let mut spans = vec![Span::styled(lead, palette.muted_style())];
        spans.extend(ui::rendered_line(line, wrap_width).spans);
        lines.push(Line::from(spans));
    }
}

/// A diff row: its path, then the lines the renderer already wrapped.
fn push_diff(lines: &mut Vec<Line<'static>>, row: &Row, width: usize, palette: &Palette) {
    let title = row.title.clone().unwrap_or_default();
    lines.push(Line::from(Span::styled(title, palette.accent_style())));
    for line in row.rendered.iter().flatten() {
        lines.push(ui::rendered_line(line, width));
    }
}

/// A prompt, a tool row, a notice or a failure: a marker and wrapped text.
fn push_plain(
    lines: &mut Vec<Line<'static>>,
    row: &Row,
    width: usize,
    marks: Glyphs,
    palette: &Palette,
) {
    let (marker, style) = match row.kind {
        RowKind::User => (format!("{} ", marks.user), palette.accent_style()),
        RowKind::ToolCall | RowKind::ToolResult => {
            (format!("{} ", marks.tool), palette.muted_style())
        }
        RowKind::Error => ("! ".to_owned(), palette.error_style()),
        RowKind::Notice => ("- ".to_owned(), palette.muted_style()),
        RowKind::Summary => ("".to_owned(), palette.muted_style()),
        RowKind::SummaryFailed => ("".to_owned(), palette.error_style()),
        RowKind::Assistant | RowKind::Diff => return,
    };
    let indent = " ".repeat(display_width(&marker));
    let wrap_width = width.saturating_sub(display_width(&marker)).max(1);
    let text = match &row.title {
        Some(title) => format!("{title} {}", row.text),
        None => row.text.clone(),
    };
    let wrapped = wrap(&text, wrap_width);
    if wrapped.is_empty() {
        lines.push(Line::from(Span::styled(marker, palette.muted_style())));
        return;
    }
    for (index, chunk) in wrapped.into_iter().enumerate() {
        let lead = if index == 0 {
            marker.clone()
        } else {
            indent.clone()
        };
        lines.push(Line::from(vec![
            Span::styled(lead, palette.muted_style()),
            Span::styled(chunk, style),
        ]));
    }
}

/// The Markdown styles the active palette paints with.
fn markdown_theme(palette: &Palette, glyphs: GlyphMode) -> MarkdownTheme {
    let rgb = |color: niki_tui_themes::Rgb| niki_render::Rgb::new(color.r, color.g, color.b);
    let body = rgb(palette.body);
    let muted = rgb(palette.muted);
    let accent = rgb(palette.accent);
    let mut headings = MarkdownTheme::plain().headings;
    for heading in &mut headings {
        heading.color = Some(accent);
    }
    MarkdownTheme {
        text: SpanStyle {
            color: Some(body),
            ..SpanStyle::PLAIN
        },
        headings,
        code: SpanStyle {
            color: Some(muted),
            code: true,
            ..SpanStyle::PLAIN
        },
        code_span: SpanStyle {
            color: Some(muted),
            code: true,
            ..SpanStyle::PLAIN
        },
        strong: SpanStyle {
            color: Some(body),
            bold: true,
            ..SpanStyle::PLAIN
        },
        emphasis: SpanStyle {
            color: Some(body),
            italic: true,
            ..SpanStyle::PLAIN
        },
        link: SpanStyle {
            color: Some(accent),
            underline: true,
            link: true,
            ..SpanStyle::PLAIN
        },
        quote: SpanStyle {
            color: Some(muted),
            quote: true,
            ..SpanStyle::PLAIN
        },
        list_marker: SpanStyle {
            color: Some(accent),
            bold: true,
            ..SpanStyle::PLAIN
        },
        rule: SpanStyle {
            color: Some(muted),
            ..SpanStyle::PLAIN
        },
        bullet: "-",
        quote_gutter: if glyphs.is_ascii() { "| " } else { "│ " },
        rule_text: if glyphs.is_ascii() { "-" } else { "─" },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn a_fresh_overlay_sits_at_the_bottom_and_names_the_first_visible_line() {
        let overlay = TranscriptOverlay::new();
        assert_eq!(overlay.offset(), 0);
        assert_eq!(overlay.header(0, 10), "transcript  line 0/0");
        assert_eq!(overlay.header(100, 10), "transcript  line 91/100");
        assert_eq!(
            overlay.start(5, 10),
            0,
            "a short transcript starts at the top"
        );
    }

    #[test]
    fn keys_move_by_a_line_a_page_and_to_the_ends() {
        let mut overlay = TranscriptOverlay::new();
        assert!(overlay.handle_key(key(KeyCode::Up)));
        assert_eq!(overlay.offset(), 1);
        assert!(overlay.handle_key(key(KeyCode::PageUp)));
        assert_eq!(overlay.offset(), 11);
        assert!(overlay.handle_key(key(KeyCode::PageDown)));
        assert_eq!(overlay.offset(), 1);
        assert!(overlay.handle_key(key(KeyCode::Down)));
        assert_eq!(overlay.offset(), 0);
        assert!(overlay.handle_key(key(KeyCode::Down)));
        assert_eq!(overlay.offset(), 0, "it stops at the newest line");

        assert!(overlay.handle_key(key(KeyCode::Char('g'))));
        assert_eq!(overlay.offset(), usize::MAX, "g asks for the top");
        assert!(overlay.handle_key(key(KeyCode::Char('G'))));
        assert_eq!(overlay.offset(), 0, "G asks for the bottom");

        assert!(!overlay.handle_key(key(KeyCode::Esc)));
        assert!(!overlay.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)));
    }

    #[test]
    fn the_footer_follows_the_glyph_mode() {
        assert_eq!(TranscriptOverlay::footer(GlyphMode::Unicode), FOOTER);
        assert_eq!(TranscriptOverlay::footer(GlyphMode::Ascii), ASCII_FOOTER);
        assert!(ASCII_FOOTER.is_ascii());
    }

    #[test]
    fn every_row_kind_becomes_at_least_one_line() {
        let mut rows = Vec::new();
        rows.push(Row {
            kind: RowKind::User,
            title: None,
            text: "a prompt".to_owned(),
            rendered: None,
        });
        for (kind, title) in [
            (RowKind::Assistant, None),
            (RowKind::ToolCall, Some("shell".to_owned())),
            (RowKind::ToolResult, Some("shell".to_owned())),
            (RowKind::Error, None),
            (RowKind::Notice, None),
        ] {
            rows.push(Row {
                kind,
                title,
                text: "text".to_owned(),
                rendered: None,
            });
        }
        let lines = lines(&rows, 40, &niki_tui_themes::NIKI, GlyphMode::Unicode);
        assert!(lines.len() >= rows.len() * 2 - 1, "a block per row");
        let first = lines.first().expect("the prompt").spans[1].content.clone();
        assert_eq!(first, "a prompt");
        let assistant = lines.iter().any(|line| line.to_string().contains("text"));
        assert!(assistant, "the assistant row is drawn");
    }
}
