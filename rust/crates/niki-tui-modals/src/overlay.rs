use niki_tui_themes::{GlyphMode, Palette};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::approval::{ApprovalModal, ellipsize};
use crate::help::HelpOverlay;
use crate::model::ModelPicker;
use crate::status::StatusCard;
use crate::threads::ThreadPicker;

/// Widest the overlay grows, so a line of help stays readable on a wide screen.
const MAX_WIDTH: u16 = 76;

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
    let entries = HelpOverlay::entries();
    let mut lines: Vec<Line> = entries
        .iter()
        .map(|(chord, description)| {
            Line::from(vec![
                Span::styled(format!("{chord:<12}"), palette.accent_style()),
                Span::styled(*description, palette.body_style()),
            ])
        })
        .collect();
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "enter sends | alt+enter adds a line | escape clears the draft",
        palette.muted_style(),
    )));
    lines.push(Line::from(Span::styled(
        "escape closes this list | page up and page down scroll it",
        palette.muted_style(),
    )));
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("composer", palette.accent_style())));
    for (chord, description) in HelpOverlay::composer_entries() {
        lines.push(Line::from(vec![
            Span::styled(format!("{chord:<12}"), palette.accent_style()),
            Span::styled(*description, palette.body_style()),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("commands", palette.accent_style())));
    for (name, summary) in HelpOverlay::command_entries() {
        lines.push(Line::from(vec![
            Span::styled(format!("{name:<12}"), palette.accent_style()),
            Span::styled(summary, palette.body_style()),
        ]));
    }
    let lines: Vec<Line> = lines.into_iter().skip(modal.offset()).collect();
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX) + 2;
    render(frame, area, "Keys", lines, palette, glyphs, height);
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

fn render(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
    palette: &Palette,
    glyphs: GlyphMode,
    height: u16,
) {
    let rect = centered(area, height);
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

/// A box of `height` rows, as wide as the screen allows, centred in `area`.
fn centered(area: Rect, height: u16) -> Rect {
    let width = area.width.min(MAX_WIDTH);
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
