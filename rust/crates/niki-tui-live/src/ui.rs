//! Drawing the screen.
//!
//! One layout, three regions: the transcript viewport, the composer block and
//! the status bar. Nothing here reads a terminal or an event; [`draw`] takes the
//! same [`App`] a test can build, which is what lets the whole view be checked
//! against a `TestBackend` buffer.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::App;
use crate::transcript::RowKind;

/// The composer's hint when nothing has been typed.
pub const PLACEHOLDER: &str = "Type a prompt; / for commands, ctrl+d to quit";

/// Paint the whole screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    frame.render_widget(Block::default().style(app.palette.canvas()), area);

    let areas = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(app.composer_height()),
        Constraint::Length(1),
    ])
    .split(area);
    let [body, composer, status] = [areas[0], areas[1], areas[2]];

    draw_transcript(frame, app, body);
    let cursor = draw_composer(frame, app, composer);
    draw_status(frame, app, status);
    // A modal owns the keys while it is open, so the composer cursor would be a
    // promise the screen cannot keep.
    if let Some(modal) = &app.modal {
        modal.draw(frame, area, app.palette, app.glyphs);
    } else if let Some(position) = cursor {
        frame.set_cursor_position(position);
    }
}

fn draw_transcript(frame: &mut Frame, app: &mut App, area: Rect) {
    if area.is_empty() {
        return;
    }
    let width = usize::from(area.width);
    let height = usize::from(area.height);
    let lines = transcript_lines(app, width);
    app.page = height;
    app.scroll.clamp(lines.len(), height);
    let start = app.scroll.start(lines.len(), height);
    let visible: Vec<Line<'static>> = lines.into_iter().skip(start).collect();
    let paragraph = Paragraph::new(visible).style(app.palette.canvas());
    frame.render_widget(paragraph, area);
}

/// Wrap every row into display lines, each led by its marker.
fn transcript_lines(app: &App, width: usize) -> Vec<Line<'static>> {
    let palette = app.palette;
    let marks = app.marks();
    let mut lines: Vec<Line<'static>> = Vec::new();
    for row in app.transcript.rows() {
        let (marker, style) = match row.kind {
            RowKind::User => (format!("{} ", marks.user), palette.accent_style()),
            RowKind::Assistant => (format!("{} ", marks.assistant), palette.body_style()),
            RowKind::ToolCall => (format!("{} ", marks.tool), palette.muted_style()),
            RowKind::ToolResult => (format!("{} ", marks.tool), palette.muted_style()),
            RowKind::Error => ("! ".to_owned(), palette.error_style()),
            RowKind::Notice => ("- ".to_owned(), palette.muted_style()),
        };
        let indent = " ".repeat(niki_render::display_width(&marker));
        let wrap_width = width
            .saturating_sub(niki_render::display_width(&marker))
            .max(1);
        let text = match &row.title {
            Some(title) => format!("{title} {}", row.text),
            None => row.text.clone(),
        };
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        for (index, chunk) in niki_render::wrap(&text, wrap_width).into_iter().enumerate() {
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
    lines
}

fn draw_composer(frame: &mut Frame, app: &mut App, area: Rect) -> Option<Position> {
    if area.is_empty() {
        return None;
    }
    let palette = app.palette;
    let block = Block::bordered()
        .border_set(app.glyphs.border())
        .border_style(palette.border_style())
        .style(palette.panel_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.is_empty() {
        return None;
    }

    let rows = usize::from(inner.height);
    let (visible, offset) = app.composer.window(rows);
    let lines: Vec<Line<'static>> = if app.composer.is_empty() {
        vec![Line::from(Span::styled(PLACEHOLDER, palette.muted_style()))]
    } else {
        visible
            .into_iter()
            .map(|text| Line::from(Span::styled(text.to_owned(), palette.body_style())))
            .collect()
    };
    frame.render_widget(Paragraph::new(lines), inner);

    let (row, column) = app.composer.row_col();
    let last_row = inner.height.saturating_sub(1);
    let last_column = inner.width.saturating_sub(1);
    let x = inner
        .x
        .saturating_add(u16::try_from(column).unwrap_or(last_column))
        .min(inner.x.saturating_add(last_column));
    let y = inner
        .y
        .saturating_add(u16::try_from(row.saturating_sub(offset)).unwrap_or(last_row))
        .min(inner.y.saturating_add(last_row));
    Some(Position::new(x, y))
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    if area.is_empty() {
        return;
    }
    frame.render_widget(Block::default().style(app.palette.panel_style()), area);
    let line = Line::from(Span::styled(app.status_line(), app.palette.panel_style()));
    frame.render_widget(Paragraph::new(line), area);
}
