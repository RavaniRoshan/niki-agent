//! Drawing the screen.
//!
//! One layout, four regions: the transcript viewport, the composer rule and its
//! text, the command list, and the two-line footer. Nothing here reads a
//! terminal or an event; [`draw`] takes the same [`App`] a test can build, which
//! is what lets the whole view be checked against a `TestBackend` buffer.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use niki_render::{RenderedLine, SpanStyle};

use crate::app::App;
use crate::transcript::RowKind;

/// The composer's hint when nothing has been typed.
///
/// One short sentence with no key list: the keys live on the footer's hint row,
/// one line down, where they are legible at every width.
pub const PLACEHOLDER: &str = "Ask Niki to do anything";

/// `cwd` with the home directory collapsed to `~`.
///
/// The welcome block prints this instead of the absolute path so a checkout six
/// directories deep still leaves the command list on one screen.
fn display_dir(cwd: &str) -> String {
    display_dir_in(cwd, std::env::var("HOME").ok().as_deref())
}

/// [`display_dir`] with the home directory passed in, so a test does not have
/// to move the process environment.
fn display_dir_in(cwd: &str, home: Option<&str>) -> String {
    let path = std::path::Path::new(cwd);
    let Some(home) = home else {
        return cwd.to_owned();
    };
    let Ok(rest) = path.strip_prefix(home) else {
        return cwd.to_owned();
    };
    let rest = rest.display().to_string();
    if rest.is_empty() {
        "~".to_owned()
    } else {
        format!("~{}{rest}", std::path::MAIN_SEPARATOR)
    }
}

/// Paint the whole screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    frame.render_widget(Block::default().style(app.palette.canvas()), area);
    app.sync_turn_clock();

    let working = app.working_line();
    let areas = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(u16::from(working.is_some())),
        Constraint::Length(app.composer_height()),
        Constraint::Length(app.completion_height()),
        Constraint::Length(2),
    ])
    .split(area);
    let [body, live, composer, completion, footer] =
        [areas[0], areas[1], areas[2], areas[3], areas[4]];

    draw_transcript_if_any(frame, app, body);
    if let Some(line) = working {
        draw_working(frame, app, live, &line);
    }
    let cursor = draw_composer(frame, app, composer);
    draw_completion(frame, app, completion);
    draw_footer(frame, app, footer);
    if let Some(overlay) = app.transcript_overlay.as_mut() {
        crate::transcript_overlay::draw(
            frame,
            area,
            overlay,
            app.transcript.rows(),
            app.palette,
            app.glyphs,
        );
    }
    // A modal owns the keys while it is open, so the composer cursor would be a
    // promise the screen cannot keep. The transcript overlay hides it too,
    // because the composer is not on screen.
    if let Some(modal) = &app.modal {
        modal.draw(frame, area, app.palette, app.glyphs);
    } else if app.transcript_overlay.is_none() {
        if let Some(position) = cursor {
            frame.set_cursor_position(position);
        }
    }
}

/// The live turn row: an animated bullet, the heading, the clock and the way
/// out.
///
/// It sits directly above the composer, so the two things a person reaches for
/// while a turn runs — reading that it is alive and stopping it — are the same
/// place they are looking.
fn draw_working(frame: &mut Frame, app: &App, area: Rect, line: &str) {
    if area.is_empty() {
        return;
    }
    let palette = app.palette;
    let width = usize::from(area.width);
    let (glyph, rest) = line.split_at(line.chars().next().map_or(0, char::len_utf8));
    let (heading, clock) = match rest.split_once('(') {
        Some((heading, tail)) => (heading, Some(format!("({tail}"))),
        None => (rest, None),
    };
    let mut spans = vec![
        Span::styled(clip(glyph, width).0, palette.accent_style()),
        Span::styled(clip(heading, width).0, palette.body_style()),
    ];
    if let Some(clock) = clock {
        spans.push(Span::styled(clip(&clock, width).0, palette.muted_style()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_transcript_if_any(frame: &mut Frame, app: &mut App, area: Rect) {
    if area.is_empty() {
        return;
    }
    let height = usize::from(area.height);
    app.page = height;
    if !app.transcript.has_conversation() && app.status.turn == crate::status::TurnState::Idle {
        draw_welcome(frame, app, area);
        return;
    }
    draw_transcript(frame, app, area);
}

/// The first screen in a fresh session: what this is, where it is rooted, and
/// the commands worth knowing first.
///
/// The shape is the one a person already knows from other terminal agents: a
/// `>_` mark, the product and its version, the home-relative directory, then a
/// single sentence introducing the command list. No tagline — the footer and
/// the site carry the positioning, and prose there would push the commands off
/// a small screen.
///
/// Notices — a trust warning, say — are drawn beneath the welcome instead of
/// replacing it. With the `welcome` effect off the notices stand alone; the
/// footer still carries the model, mode and directory.
fn draw_welcome(frame: &mut Frame, app: &App, area: Rect) {
    let palette = app.palette;
    let width = usize::from(area.width);
    let height = usize::from(area.height);
    if !app.effects.welcome {
        let lines = notice_lines(app, width);
        frame.render_widget(Paragraph::new(lines).style(palette.canvas()), area);
        return;
    }
    let mut lines: Vec<Line<'static>> = Vec::new();

    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled(clip("  >_ ", width).0, palette.accent_style()),
        Span::styled(clip("NIKI AGENT", width).0, palette.body_style()),
        Span::styled(
            clip(&format!(" (v{})", env!("CARGO_PKG_VERSION")), width).0,
            palette.muted_style(),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        clip(&format!("     {}", display_dir(&app.status.cwd)), width).0,
        palette.muted_style(),
    )));
    if app.status.mode == niki_policy::Mode::Yolo {
        lines.push(Line::from(vec![
            Span::styled(clip("  permissions: ", width).0, palette.muted_style()),
            Span::styled(clip("YOLO mode", width).0, palette.error_style()),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        clip(
            "  To get started, describe a task or try one of these commands:",
            width,
        )
        .0,
        palette.muted_style(),
    )));
    lines.push(Line::default());
    for (command, summary) in [
        ("/init", "write an AGENTS.md for this repository"),
        ("/status", "model, mode, session and token usage"),
        ("/model", "choose a model"),
        ("/theme", "choose a palette"),
        ("/help", "every command and key binding"),
    ] {
        lines.push(Line::from(vec![
            Span::styled(
                clip(&format!("  {command}"), width).0,
                palette.accent_style(),
            ),
            Span::styled(
                clip(&format!(" - {summary}"), width).0,
                palette.muted_style(),
            ),
        ]));
    }

    // A notice is the app talking, not the person, so it sits under the welcome
    // rather than replacing it: a trust warning must not be the first screen.
    let notices = notice_lines(app, width);
    if !notices.is_empty() {
        lines.push(Line::default());
        lines.extend(notices);
    }

    lines.truncate(height);
    frame.render_widget(Paragraph::new(lines).style(palette.canvas()), area);
}

/// The `- ` notice rows, wrapped to `width`.
fn notice_lines(app: &App, width: usize) -> Vec<Line<'static>> {
    let palette = app.palette;
    let mut lines = Vec::new();
    for row in app.transcript.rows() {
        if row.kind != RowKind::Notice {
            continue;
        }
        for (index, chunk) in niki_render::wrap(&row.text, width.saturating_sub(2).max(1))
            .into_iter()
            .enumerate()
        {
            let lead = if index == 0 { "- " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(lead.to_owned(), palette.muted_style()),
                Span::styled(chunk, palette.muted_style()),
            ]));
        }
    }
    lines
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
    let rows = app.transcript.rows();
    let last = rows.len().saturating_sub(1);
    let mut lines: Vec<Line<'static>> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if row.kind == RowKind::Diff {
            if !lines.is_empty() {
                lines.push(Line::default());
            }
            let title = row.title.clone().unwrap_or_default();
            lines.push(Line::from(Span::styled(
                clip(&title, width).0,
                palette.accent_style(),
            )));
            for line in row.rendered.iter().flatten() {
                lines.push(rendered_line(line, width));
            }
            continue;
        }
        let (marker, style) = match row.kind {
            RowKind::User => (format!("{} ", marks.user), palette.accent_style()),
            RowKind::Assistant => (format!("{} ", marks.assistant), palette.body_style()),
            RowKind::ToolCall => {
                // A call that is still running carries the activity marker, so
                // a long step reads as alive rather than frozen.
                let running = index == last && app.status.turn == crate::status::TurnState::Working;
                let glyph = if running {
                    app.activity_glyph().to_string()
                } else {
                    marks.tool.to_owned()
                };
                (format!("{glyph} "), palette.muted_style())
            }
            RowKind::ToolResult => (format!("{} ", marks.ok), palette.ok_style()),
            RowKind::Error => (format!("{} ", marks.failed), palette.error_style()),
            RowKind::Notice => ("- ".to_owned(), palette.muted_style()),
            RowKind::Diff => continue,
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
    for prompt in app.queued() {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        let marker = "queued: ";
        let indent = " ".repeat(marker.len());
        let wrap_width = width.saturating_sub(marker.len()).max(1);
        for (index, chunk) in niki_render::wrap(prompt, wrap_width)
            .into_iter()
            .enumerate()
        {
            let lead = if index == 0 {
                marker.to_owned()
            } else {
                indent.clone()
            };
            lines.push(Line::from(vec![
                Span::styled(lead, palette.muted_style()),
                Span::styled(chunk, palette.muted_style()),
            ]));
        }
    }
    lines
}

/// One pre-rendered line, clipped to the viewport rather than wrapped.
pub(crate) fn rendered_line(line: &RenderedLine, width: usize) -> Line<'static> {
    let mut spans = Vec::new();
    let mut used = 0;
    for span in &line.spans {
        if used >= width {
            break;
        }
        let (text, cells) = clip(&span.text, width - used);
        if !text.is_empty() {
            spans.push(Span::styled(text, span_style(span.style)));
        }
        used += cells;
    }
    Line::from(spans)
}

fn span_style(style: SpanStyle) -> Style {
    let mut paint = Style::new();
    if let Some(color) = style.color {
        paint = paint.fg(Color::Rgb(color.r, color.g, color.b));
    }
    if style.bold {
        paint = paint.add_modifier(Modifier::BOLD);
    }
    if style.italic {
        paint = paint.add_modifier(Modifier::ITALIC);
    }
    if style.underline {
        paint = paint.add_modifier(Modifier::UNDERLINED);
    }
    paint
}

/// The longest prefix of `text` that fits in `width` cells, and its width.
fn clip(text: &str, width: usize) -> (String, usize) {
    let total = niki_render::display_width(text);
    if total <= width {
        return (text.to_owned(), total);
    }
    let mut clipped = String::new();
    let mut used = 0;
    for character in text.chars() {
        let cells = niki_render::display_width(&character.to_string());
        if used + cells > width {
            break;
        }
        clipped.push(character);
        used += cells;
    }
    (clipped, used)
}

/// The composer: one hairline rule, then the draft with a `›` where input
/// begins.
///
/// A frame would spend four rows and a border colour on one line of text. The
/// rule marks the input region with less ink, and the glyph survives the ASCII
/// mode as `>`.
fn draw_composer(frame: &mut Frame, app: &mut App, area: Rect) -> Option<Position> {
    if area.is_empty() {
        return None;
    }
    let palette = app.palette;
    let rule = app.glyphs.border().horizontal_top;
    let rule_row = Rect { height: 1, ..area };
    let rule_line = Line::from(Span::styled(
        rule.repeat(usize::from(area.width)),
        palette.border_style(),
    ));
    frame.render_widget(Paragraph::new(rule_line), rule_row);
    if area.height < 2 {
        return None;
    }
    let inner = Rect {
        y: area.y + 1,
        height: area.height - 1,
        ..area
    };

    let mark = app.marks().user;
    let prefix = niki_render::display_width(mark) + 1;
    let indent = " ".repeat(prefix);
    let rows = usize::from(inner.height);
    let (visible, offset) = app.composer.window(rows);
    let lines: Vec<Line<'static>> = if app.composer.is_empty() {
        vec![Line::from(vec![
            Span::styled(format!("{mark} "), palette.accent_style()),
            Span::styled(PLACEHOLDER, palette.muted_style()),
        ])]
    } else {
        visible
            .into_iter()
            .enumerate()
            .map(|(index, text)| {
                let lead = if index == 0 {
                    format!("{mark} ")
                } else {
                    indent.clone()
                };
                Line::from(vec![
                    Span::styled(lead, palette.accent_style()),
                    Span::styled(text.to_owned(), palette.body_style()),
                ])
            })
            .collect()
    };
    frame.render_widget(Paragraph::new(lines), inner);

    let (row, column) = app.composer.row_col();
    let last_row = inner.height.saturating_sub(1);
    let last_column = inner.width.saturating_sub(1);
    let x = inner
        .x
        .saturating_add(u16::try_from(prefix).unwrap_or(last_column))
        .saturating_add(u16::try_from(column).unwrap_or(last_column))
        .min(inner.x.saturating_add(last_column));
    let y = inner
        .y
        .saturating_add(u16::try_from(row.saturating_sub(offset)).unwrap_or(last_row))
        .min(inner.y.saturating_add(last_row));
    Some(Position::new(x, y))
}

fn draw_completion(frame: &mut Frame, app: &App, area: Rect) {
    let Some(completion) = app.completion() else {
        return;
    };
    if area.is_empty() {
        return;
    }
    let palette = app.palette;
    let marks = app.marks();
    let mut lines = Vec::with_capacity(completion.commands().len() + 1);
    for (index, command) in completion.commands().iter().enumerate() {
        let chosen = index == completion.selected_index();
        let lead = if chosen {
            format!("{} ", marks.user)
        } else {
            "  ".to_owned()
        };
        let style = if chosen {
            palette.accent_style()
        } else {
            palette.muted_style()
        };
        lines.push(Line::from(vec![
            Span::styled(lead, style),
            Span::styled(command.name, style),
        ]));
    }
    lines.push(Line::from(Span::styled(
        format!("  {}", completion.selected().summary),
        palette.muted_style(),
    )));
    frame.render_widget(Paragraph::new(lines), area);
}

/// The footer: the state line, then keys on the left and counts on the right.
///
/// The hint row never goes away. A person who has run a hundred turns still
/// forgets a chord, and the row costs nothing once the counts are right-aligned
/// beside it. The two groups degrade independently: the hints drop whole
/// entries until they fit, and if even one entry will not share the row the
/// counts get it alone. Both rows share the panel colour so the footer reads as
/// one band.
fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    if area.is_empty() {
        return;
    }
    frame.render_widget(Block::default().style(app.palette.panel_style()), area);
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);

    let state = Line::from(Span::styled(app.status_line(), app.palette.panel_style()));
    frame.render_widget(Paragraph::new(state), rows[0]);

    let width = usize::from(rows[1].width);
    let accounting = app.accounting_line().unwrap_or_default();
    let accounting_width = niki_render::display_width(&accounting);
    // One blank column between the groups, and at least a dozen for keys: below
    // that the counts are the more useful half and take the row alone.
    let room = width.saturating_sub(accounting_width + 1);
    let hints = if accounting.is_empty() {
        app.hints(width)
    } else if room >= 12 {
        app.hints(room)
    } else {
        String::new()
    };

    let mut spans = Vec::new();
    if !hints.is_empty() {
        spans.push(Span::styled(hints.clone(), app.palette.muted_style()));
    }
    if !accounting.is_empty() {
        let pad = width.saturating_sub(
            niki_render::display_width(&hints) + niki_render::display_width(&accounting),
        );
        if pad > 0 {
            spans.push(Span::raw(" ".repeat(pad)));
        }
        spans.push(Span::styled(accounting, app.palette.muted_style()));
    }
    if !spans.is_empty() {
        frame.render_widget(Paragraph::new(Line::from(spans)), rows[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::display_dir_in;

    #[test]
    fn the_home_directory_collapses_to_a_tilde() {
        assert_eq!(
            display_dir_in("/home/dev/project", Some("/home/dev")),
            "~/project"
        );
        assert_eq!(display_dir_in("/home/dev", Some("/home/dev")), "~");
        assert_eq!(display_dir_in("/srv/app", Some("/home/dev")), "/srv/app");
        assert_eq!(display_dir_in("/srv/app", None), "/srv/app");
    }
}
