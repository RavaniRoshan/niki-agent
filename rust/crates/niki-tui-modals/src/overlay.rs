use niki_tui_themes::{GlyphMode, Palette};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::approval::{ApprovalModal, ellipsize};
use crate::help::{HelpOverlay, ShortcutGroup};
use crate::model::ModelPicker;
use crate::question::{QuestionModal, QuestionRow};
use crate::settings::SettingsModal;
use crate::status::StatusCard;
use crate::threads::ThreadPicker;
use crate::trust::TrustModal;

/// Widest the overlay grows, so a line of help stays readable on a wide screen.
const MAX_WIDTH: u16 = 76;

/// Width the key map may use: three columns of chords stop fitting at the
/// width a prose panel wants, and the map is the one surface with no prose.
const HELP_WIDTH: u16 = 104;

/// The way out, printed in the key map's last row.
const HELP_FOOTER: &str = "? or escape closes; page up and page down scroll";

/// The same hint without the scroll reminder, for a terminal too narrow for it.
const HELP_FOOTER_SHORT: &str = "? or escape closes";

/// Column the status card's labels reserve, matching the help overlay's chords.
const LABEL_WIDTH: usize = 12;

pub(crate) fn approval(
    frame: &mut Frame,
    area: Rect,
    modal: &ApprovalModal,
    palette: &Palette,
    glyphs: GlyphMode,
) {
    let width = usize::from(area.width.min(MAX_WIDTH).saturating_sub(2));
    let mut lines: Vec<Line> = Vec::new();
    for (index, detail) in modal.body(width).into_iter().enumerate() {
        let style = if index == 0 {
            palette.accent_style()
        } else {
            palette.muted_style()
        };
        lines.push(Line::from(Span::styled(detail, style)));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        ellipsize(
            &format!(
                "the policy in {} mode needs your answer before it runs",
                modal.request().mode.label()
            ),
            width,
        ),
        palette.muted_style(),
    )));
    lines.push(Line::default());
    for (index, (_, label)) in modal.options().into_iter().enumerate() {
        let chosen = index == modal.selected();
        let style = if chosen {
            palette.accent_style()
        } else {
            palette.body_style()
        };
        let marker = if chosen { "> " } else { "  " };
        lines.push(Line::from(Span::styled(format!("{marker}{label}"), style)));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        ellipsize(
            "y/a allow once | s allow session | d/n/escape refuse",
            width,
        ),
        palette.muted_style(),
    )));
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX) + 2;
    render(
        frame,
        area,
        "Approve a command",
        lines,
        palette,
        glyphs,
        height,
    );
}

pub(crate) fn question(
    frame: &mut Frame,
    area: Rect,
    modal: &QuestionModal,
    palette: &Palette,
    glyphs: GlyphMode,
) {
    let width = usize::from(area.width.min(MAX_WIDTH).saturating_sub(2));
    let rows = modal.rows(width);
    let question_lines: Vec<&str> = rows
        .iter()
        .filter_map(|row| match row {
            QuestionRow::Question(line) => Some(line.as_str()),
            _ => None,
        })
        .collect();
    let rest: Vec<&QuestionRow> = rows
        .iter()
        .filter(|row| !matches!(row, QuestionRow::Question(_)))
        .collect();

    let natural = u16::try_from(rows.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2);
    let rect = centered(area, natural, MAX_WIDTH);
    if rect.is_empty() {
        return;
    }
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_set(glyphs.border())
        .border_style(palette.accent_style())
        .style(palette.panel_style())
        .title(Line::from(Span::styled("Question", palette.accent_style())));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.is_empty() {
        return;
    }

    // The choices keep the bottom rows; the question takes whatever is left and
    // scrolls there, so a long question never pushes the options out of reach.
    let visible = usize::from(inner.height);
    // Keep one row for the question when the frame has room for it, so a long
    // question can be read by scrolling instead of being pushed out entirely.
    let reserve = usize::from(visible > 2 && !question_lines.is_empty());
    let pinned = rest.len().min(visible.saturating_sub(reserve));
    let question_height = visible - pinned;
    let question_offset = modal.question_offset(width, question_height);
    let rest_offset = modal.interactive_offset(pinned);

    draw_question_lines(
        frame,
        inner,
        question_height,
        question_lines.iter().copied().skip(question_offset),
        palette,
    );
    draw_question_rest(
        frame,
        inner,
        question_height,
        pinned,
        rest.iter().copied().skip(rest_offset),
        modal,
        palette,
    );
}

fn draw_question_lines<'a>(
    frame: &mut Frame,
    inner: Rect,
    height: usize,
    lines: impl Iterator<Item = &'a str>,
    palette: &Palette,
) {
    if height == 0 {
        return;
    }
    let area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: u16::try_from(height).unwrap_or(u16::MAX),
    };
    let lines: Vec<Line> = lines
        .map(|line| Line::from(Span::styled(line.to_owned(), palette.accent_style())))
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_question_rest<'a>(
    frame: &mut Frame,
    inner: Rect,
    question_height: usize,
    height: usize,
    rows: impl Iterator<Item = &'a QuestionRow>,
    modal: &QuestionModal,
    palette: &Palette,
) {
    if height == 0 {
        return;
    }
    let area = Rect {
        x: inner.x,
        y: inner
            .y
            .saturating_add(u16::try_from(question_height).unwrap_or(u16::MAX)),
        width: inner.width,
        height: u16::try_from(height).unwrap_or(u16::MAX),
    };
    let lines: Vec<Line> = rows
        .map(|row| match row {
            QuestionRow::Count(text) => {
                Line::from(Span::styled(text.clone(), palette.muted_style()))
            }
            QuestionRow::Option { index, text } => {
                let chosen = *index == modal.selected();
                let marker = if chosen { "> " } else { "  " };
                let style = if chosen {
                    palette.accent_style()
                } else {
                    palette.body_style()
                };
                Line::from(Span::styled(format!("{marker}{text}"), style))
            }
            QuestionRow::Freeform { text } => {
                let focused = modal.is_freeform_selected();
                let marker = if focused { "> " } else { "  " };
                let style = if focused {
                    palette.accent_style()
                } else {
                    palette.muted_style()
                };
                Line::from(Span::styled(format!("{marker}{text}"), style))
            }
            QuestionRow::Hint(text) => {
                Line::from(Span::styled(text.clone(), palette.muted_style()))
            }
            QuestionRow::Question(_) => Line::default(),
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

pub(crate) fn models(
    frame: &mut Frame,
    area: Rect,
    modal: &ModelPicker,
    palette: &Palette,
    glyphs: GlyphMode,
) {
    let mut lines = Vec::new();
    if modal.is_empty() {
        lines.push(Line::from(Span::styled(
            "no models are configured; add a [providers.NAME] block and [models].default",
            palette.muted_style(),
        )));
    }
    for (index, choice) in modal.choices().iter().enumerate() {
        let chosen = index == modal.selected();
        let marker = if chosen { "> " } else { "  " };
        let style = if chosen {
            palette.accent_style()
        } else {
            palette.body_style()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{marker}{}", choice.spec), style),
            Span::styled(format!("  {}", choice.provider), palette.muted_style()),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "enter choose | up and down move | escape cancel",
        palette.muted_style(),
    )));
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX) + 2;
    render(frame, area, "Model", lines, palette, glyphs, height);
}

pub(crate) fn connect(
    frame: &mut Frame,
    area: Rect,
    modal: &crate::connect::ConnectModal,
    palette: &Palette,
    glyphs: GlyphMode,
) {
    use crate::connect::Stage;

    let width = usize::from(area.width.min(MAX_WIDTH).saturating_sub(2));
    let mut lines: Vec<Line> = Vec::new();
    match modal.stage() {
        Stage::Choose => {
            for (index, entry) in modal.entries().iter().enumerate() {
                let chosen = index == modal.selected();
                let (lead, style) = if chosen {
                    ("> ", palette.accent_style())
                } else {
                    ("  ", palette.body_style())
                };
                let state = if entry.connected {
                    "connected"
                } else if entry.keyless {
                    "no key needed"
                } else {
                    ""
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{lead}{}", entry.label), style),
                    Span::styled(format!("  {state}"), palette.muted_style()),
                ]));
            }
            lines.push(Line::default());
            lines.push(Line::from(Span::styled(
                "enter choose | up and down move | escape cancel".to_owned(),
                palette.muted_style(),
            )));
        }
        stage => {
            let entry = modal.highlighted();
            let (heading, prompt): (String, String) = match stage {
                Stage::BaseUrl => (
                    "Endpoint".to_owned(),
                    "the base URL, for example https://example.test/v1".to_owned(),
                ),
                Stage::Key => {
                    let hint = entry
                        .and_then(|entry| entry.key_var.as_deref())
                        .map(|var| format!("paste the key, or press enter to use {var}"))
                        .unwrap_or_else(|| "paste the key".to_owned());
                    ("API key".to_owned(), hint)
                }
                Stage::Model => (
                    "Model".to_owned(),
                    "the model id this provider exposes".to_owned(),
                ),
                Stage::Choose => unreachable!("handled above"),
            };
            if let Some(entry) = entry {
                lines.push(Line::from(Span::styled(
                    format!("{} ({})", entry.label, entry.base_url),
                    palette.body_style(),
                )));
                lines.push(Line::default());
            }
            lines.push(Line::from(Span::styled(heading, palette.muted_style())));
            let draft = modal.draft();
            let shown = if stage == Stage::Key && !draft.is_empty() {
                "*".repeat(draft.chars().count())
            } else {
                draft.to_owned()
            };
            let (text, style) = if shown.is_empty() {
                (
                    if stage == Stage::Model {
                        format!("  {}", modal.placeholder())
                    } else {
                        format!("  {prompt}")
                    },
                    palette.muted_style(),
                )
            } else {
                (format!("  {shown}"), palette.accent_style())
            };
            lines.push(Line::from(Span::styled(ellipsize(&text, width), style)));
            if let Some(error) = modal.error() {
                lines.push(Line::default());
                lines.push(Line::from(Span::styled(
                    ellipsize(error, width),
                    palette.error_style(),
                )));
            }
            lines.push(Line::default());
            lines.push(Line::from(Span::styled(
                "enter continue | escape back".to_owned(),
                palette.muted_style(),
            )));
        }
    }
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX) + 2;
    render(
        frame,
        area,
        "Connect a provider",
        lines,
        palette,
        glyphs,
        height,
    );
}

pub(crate) fn threads(
    frame: &mut Frame,
    area: Rect,
    modal: &ThreadPicker,
    palette: &Palette,
    glyphs: GlyphMode,
) {
    let mut lines = Vec::new();
    if !modal.filter().is_empty() {
        lines.push(Line::from(Span::styled(
            format!("filter: {}", modal.filter()),
            palette.accent_style(),
        )));
    }
    if modal.is_empty() {
        lines.push(Line::from(Span::styled(
            "no sessions are saved yet",
            palette.muted_style(),
        )));
    } else if modal.is_filtered_empty() {
        lines.push(Line::from(Span::styled(
            "no session matches that filter",
            palette.muted_style(),
        )));
    }
    for index in modal.window() {
        let Some(entry) = modal.row(index) else {
            continue;
        };
        let chosen = index == modal.selected();
        let lead = if chosen { "> " } else { "  " };
        let style = if chosen {
            palette.accent_style()
        } else {
            palette.body_style()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{lead}{}", entry.preview), style),
            Span::styled(
                format!("  {}  {}", entry.message_label(), entry.cwd),
                palette.muted_style(),
            ),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "enter open | type to filter | up and down move | escape cancel",
        palette.muted_style(),
    )));
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX) + 2;
    render(frame, area, "Sessions", lines, palette, glyphs, height);
}

pub(crate) fn help(
    frame: &mut Frame,
    area: Rect,
    modal: &HelpOverlay,
    palette: &Palette,
    glyphs: GlyphMode,
) {
    let width = usize::from(area.width.min(HELP_WIDTH).saturating_sub(2));
    let mut lines: Vec<Line> = vec![Line::default()];
    lines.extend(column_lines(HelpOverlay::groups(), width, palette));
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("Commands", palette.accent_style())));
    for (name, summary) in HelpOverlay::command_entries() {
        lines.push(Line::from(vec![
            Span::styled(format!("{name:<12}"), palette.accent_style()),
            Span::styled(summary, palette.body_style()),
        ]));
    }
    let lines: Vec<Line> = lines.into_iter().skip(modal.offset()).collect();
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX) + 3;
    let footer = if width >= niki_render::display_width(HELP_FOOTER) {
        HELP_FOOTER
    } else {
        HELP_FOOTER_SHORT
    };
    let footer = ellipsize(footer, width);
    render_with_footer(
        frame, area, "Keys", lines, palette, glyphs, height, HELP_WIDTH, &footer,
    );
}

/// The columns a gap of four cells separates.
const COLUMN_GAP: usize = 4;

/// The groups as columns: three when they fit, then two, then one.
///
/// The narrow fallbacks matter more than they look. At 80 columns a terminal is
/// wide enough for three, at 60 only two, and over ssh on a phone one — and in
/// every case the reader should see whole chords rather than a truncated one.
fn column_lines(groups: &[ShortcutGroup], width: usize, palette: &Palette) -> Vec<Line<'static>> {
    let columns: Vec<(Vec<Line<'static>>, usize)> = groups
        .iter()
        .map(|group| {
            let key_width = group
                .entries
                .iter()
                .map(|(key, _)| niki_render::display_width(key))
                .max()
                .unwrap_or(0);
            let action_width = group
                .entries
                .iter()
                .map(|(_, action)| niki_render::display_width(action))
                .max()
                .unwrap_or(0);
            let mut lines = vec![Line::from(Span::styled(
                group.title,
                palette.accent_style(),
            ))];
            for (key, action) in group.entries {
                let padding = key_width.saturating_sub(niki_render::display_width(key)) + 2;
                lines.push(Line::from(vec![
                    Span::styled((*key).to_owned(), palette.body_style()),
                    Span::raw(" ".repeat(padding)),
                    Span::styled(*action, palette.muted_style()),
                ]));
            }
            (lines, key_width + 2 + action_width)
        })
        .collect();

    let fitted: Vec<(Vec<Line<'static>>, usize)> = columns;
    let total: usize = fitted.iter().map(|(_, width)| *width).sum();
    if fitted.len() == 3 && total + COLUMN_GAP * 2 <= width {
        return side_by_side(&fitted);
    }
    if fitted.len() == 3 && width >= 40 {
        let right_width = fitted[1].1.max(fitted[2].1);
        if fitted[0].1 + COLUMN_GAP + right_width <= width {
            let mut right = fitted[1].0.clone();
            right.push(Line::default());
            right.extend(fitted[2].0.clone());
            return side_by_side(&[(fitted[0].0.clone(), fitted[0].1), (right, right_width)]);
        }
    }
    let mut stacked = Vec::new();
    for (index, (lines, _)) in fitted.iter().enumerate() {
        if index > 0 {
            stacked.push(Line::default());
        }
        stacked.extend(lines.clone());
    }
    stacked
}

fn side_by_side(columns: &[(Vec<Line<'static>>, usize)]) -> Vec<Line<'static>> {
    let height = columns
        .iter()
        .map(|(lines, _)| lines.len())
        .max()
        .unwrap_or(0);
    (0..height)
        .map(|row| {
            let mut line = Line::default();
            for (column, (lines, width)) in columns.iter().enumerate() {
                let entry = lines.get(row).cloned().unwrap_or_default();
                let used: usize = entry
                    .spans
                    .iter()
                    .map(|span| niki_render::display_width(&span.content))
                    .sum();
                line.extend(entry.spans);
                if column + 1 < columns.len() {
                    line.push_span(ratatui::text::Span::raw(
                        " ".repeat(width.saturating_sub(used) + COLUMN_GAP),
                    ));
                }
            }
            line
        })
        .collect()
}

pub(crate) fn settings(
    frame: &mut Frame,
    area: Rect,
    modal: &SettingsModal,
    palette: &Palette,
    glyphs: GlyphMode,
) {
    let rows = modal.rows();
    let label_width = rows
        .iter()
        .map(|(label, _)| niki_render::display_width(label))
        .max()
        .unwrap_or(0);
    let width = usize::from(area.width.min(MAX_WIDTH).saturating_sub(2));
    let mut lines = Vec::new();
    for (index, (label, value)) in rows.iter().enumerate() {
        let chosen = index == modal.selected();
        let lead = if chosen { "> " } else { "  " };
        let style = if chosen {
            palette.accent_style()
        } else {
            palette.body_style()
        };
        let pad = " ".repeat(label_width.saturating_sub(niki_render::display_width(label)) + 2);
        lines.push(Line::from(vec![
            Span::styled(format!("{lead}{label}{pad}"), style),
            Span::styled(value.clone(), palette.muted_style()),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        ellipsize(
            "enter or space changes | up and down move | escape closes",
            width,
        ),
        palette.muted_style(),
    )));
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX) + 2;
    render(frame, area, "Settings", lines, palette, glyphs, height);
}

pub(crate) fn status(
    frame: &mut Frame,
    area: Rect,
    modal: &StatusCard,
    palette: &Palette,
    glyphs: GlyphMode,
) {
    let width = usize::from(area.width.min(MAX_WIDTH).saturating_sub(2));
    let label_width = LABEL_WIDTH.min(width);
    let value_width = width.saturating_sub(label_width);
    let natural = u16::try_from(modal.fields().len())
        .unwrap_or(u16::MAX)
        .saturating_add(2);
    // A card that fits is always drawn from its first line; only a terminal
    // too short for every line honours the scroll offset.
    let offset = if natural > area.height {
        modal.offset()
    } else {
        0
    };
    let lines: Vec<Line> = modal
        .fields()
        .iter()
        .skip(offset)
        .map(|field| {
            let label = ellipsize(&field.label, label_width);
            Line::from(vec![
                Span::styled(format!("{label:<label_width$}"), palette.accent_style()),
                Span::styled(ellipsize(&field.value, value_width), palette.body_style()),
            ])
        })
        .collect();
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX) + 2;
    render(frame, area, "Status", lines, palette, glyphs, height);
}

pub(crate) fn trust(
    frame: &mut Frame,
    area: Rect,
    modal: &TrustModal,
    palette: &Palette,
    glyphs: GlyphMode,
) {
    let width = usize::from(area.width.min(MAX_WIDTH).saturating_sub(2));
    let mut lines: Vec<Line> = Vec::new();
    for (index, detail) in modal.body(width).into_iter().enumerate() {
        let style = if index == 0 {
            palette.accent_style()
        } else {
            palette.muted_style()
        };
        lines.push(Line::from(Span::styled(detail, style)));
    }
    lines.push(Line::default());
    for (index, label) in modal.options().into_iter().enumerate() {
        let chosen = index == modal.selected();
        let style = if chosen {
            palette.accent_style()
        } else {
            palette.body_style()
        };
        let marker = if chosen { "> " } else { "  " };
        lines.push(Line::from(Span::styled(format!("{marker}{label}"), style)));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        ellipsize(
            "y trust | n/escape not now | enter takes the highlight",
            width,
        ),
        palette.muted_style(),
    )));
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX) + 2;
    render(
        frame,
        area,
        "Trust this workspace",
        lines,
        palette,
        glyphs,
        height,
    );
}

fn render(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
    palette: &Palette,
    glyphs: GlyphMode,
    height: u16,
) {
    render_with(
        frame, area, title, lines, palette, glyphs, height, MAX_WIDTH,
    );
}

/// A box whose last inner row is a fixed hint, so the way out stays visible
/// when the body scrolls past the bottom of the terminal.
#[expect(
    clippy::too_many_arguments,
    reason = "the frame, the box and the paint are all already named at the call site"
)]
fn render_with_footer(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
    palette: &Palette,
    glyphs: GlyphMode,
    height: u16,
    max_width: u16,
    footer: &str,
) {
    let rect = centered(area, height, max_width);
    if rect.is_empty() {
        return;
    }
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_set(glyphs.border())
        .border_style(palette.accent_style())
        .style(palette.panel_style())
        .title(Line::from(Span::styled(
            title.to_owned(),
            palette.accent_style(),
        )));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.height == 0 {
        return;
    }
    let body = Rect {
        height: inner.height.saturating_sub(1),
        ..inner
    };
    frame.render_widget(
        Paragraph::new(lines)
            .style(palette.panel_style())
            .wrap(Wrap { trim: false }),
        body,
    );
    let hint = Rect {
        y: inner.y + inner.height - 1,
        height: 1,
        ..inner
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            footer.to_owned(),
            palette.muted_style(),
        )))
        .style(palette.panel_style()),
        hint,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "the frame, the box and the paint are all already named at the call site"
)]
fn render_with(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
    palette: &Palette,
    glyphs: GlyphMode,
    height: u16,
    max_width: u16,
) {
    render_in(
        frame,
        centered(area, height, max_width),
        title,
        lines,
        palette,
        glyphs,
    );
}

fn render_in(
    frame: &mut Frame,
    rect: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
    palette: &Palette,
    glyphs: GlyphMode,
) {
    if rect.is_empty() {
        return;
    }
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_set(glyphs.border())
        .border_style(palette.accent_style())
        .style(palette.panel_style())
        .title(Line::from(Span::styled(
            title.to_owned(),
            palette.accent_style(),
        )));
    let paragraph = Paragraph::new(lines)
        .style(palette.panel_style())
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, rect);
}

/// A box of `height` rows, as wide as `max_width` allows, centred in `area`.
fn centered(area: Rect, height: u16, max_width: u16) -> Rect {
    let width = area.width.min(max_width);
    let height = height.min(area.height);
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(ratatui::layout::Flex::Center)
        .split(area)[0];
    Layout::horizontal([Constraint::Length(width)])
        .flex(ratatui::layout::Flex::Center)
        .split(vertical)[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::{StatusCard, StatusField};
    use niki_tui_themes::NIKI;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    static PALETTE: Palette = NIKI;

    fn card() -> StatusCard {
        StatusCard::new(vec![
            StatusField::new("model", "mock:mock-model"),
            StatusField::new("provider", "mock"),
            StatusField::new("mode", "manual"),
            StatusField::new("session", "abc123 new"),
            StatusField::new("session file", "unknown"),
            StatusField::new("workspace", "/work"),
            StatusField::new("tokens", "not measured yet"),
            StatusField::new("skills", "2"),
            StatusField::new("client", "niki 2.1.1"),
        ])
    }

    fn render(card: &StatusCard, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                status(frame, area, card, &PALETTE, GlyphMode::Unicode);
            })
            .expect("draw");
        terminal.backend().buffer().clone()
    }

    fn row(buffer: &Buffer, y: u16) -> String {
        let mut text = String::new();
        for x in 0..buffer.area.width {
            text.push_str(buffer.cell((x, y)).map_or(" ", |cell| cell.symbol()));
        }
        text
    }

    fn text(buffer: &Buffer) -> String {
        (0..buffer.area.height)
            .map(|y| row(buffer, y))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_help(width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                help(
                    frame,
                    area,
                    &HelpOverlay::new(),
                    &PALETTE,
                    GlyphMode::Unicode,
                );
            })
            .expect("draw");
        terminal.backend().buffer().clone()
    }

    fn headings(buffer: &Buffer) -> String {
        (0..buffer.area.height)
            .map(|y| row(buffer, y))
            .find(|line| line.contains("Compose"))
            .unwrap_or_default()
    }

    #[test]
    fn the_key_map_folds_from_three_columns_to_one() {
        for (width, expected) in [(120u16, 3usize), (70, 2), (40, 1)] {
            let buffer = render_help(width, 40);
            let header = headings(&buffer);
            let found = ["Compose", "Session", "Transcript"]
                .iter()
                .filter(|needle| header.contains(*needle))
                .count();
            assert_eq!(found, expected, "at {width} columns: {header:?}");
        }
    }

    #[test]
    fn a_narrow_key_map_keeps_the_way_out_whole() {
        let buffer = render_help(40, 16);
        let screen = text(&buffer);
        assert!(screen.contains("? or escape closes"), "{screen}");
        assert!(
            !screen.contains("page d"),
            "the hint must not be cut mid-word:\n{screen}"
        );
    }

    #[test]
    fn the_way_out_stays_visible_when_the_body_overflows() {
        let buffer = render_help(120, 16);
        let screen = text(&buffer);
        assert!(
            screen.contains("? or escape closes"),
            "the hint must survive a short terminal:\n{screen}"
        );
    }

    #[test]
    fn every_labelled_line_is_drawn_with_its_value() {
        let buffer = render(&card(), 60, 14);
        let screen = text(&buffer);
        assert!(screen.contains("Status"), "{screen}");
        for field in card().fields() {
            assert!(
                screen.contains(&field.label),
                "{} missing from {screen}",
                field.label
            );
            assert!(
                screen.contains(&field.value),
                "{} missing from {screen}",
                field.value
            );
        }
    }

    #[test]
    fn a_narrow_terminal_clips_every_line_to_its_width() {
        let card = StatusCard::new(vec![
            StatusField::new("model", "provider:".to_owned() + &"m".repeat(80)),
            StatusField::new("workspace", "/a/very/long/workspace/path/that/keeps/going"),
        ]);
        let buffer = render(&card, 20, 10);
        let screen = text(&buffer);
        for line in screen.lines() {
            assert!(
                niki_render::display_width(line) <= 20,
                "line wider than the terminal: {line:?}"
            );
        }
        assert!(
            screen.contains('…'),
            "a clipped value is marked: {screen:?}"
        );
    }

    #[test]
    fn a_twenty_by_six_terminal_draws_without_panicking() {
        let buffer = render(&card(), 20, 6);
        assert_eq!(buffer.area.width, 20);
        assert_eq!(buffer.area.height, 6);
        assert!(text(&buffer).contains("Status"));
    }
}
