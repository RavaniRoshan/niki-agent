use niki_tui_themes::{GlyphMode, Palette};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::approval::ApprovalModal;
use crate::help::HelpOverlay;
use crate::model::ModelPicker;

/// Widest the overlay grows, so a line of help stays readable on a wide screen.
const MAX_WIDTH: u16 = 76;

pub(crate) fn approval(
    frame: &mut Frame,
    area: Rect,
    modal: &ApprovalModal,
    palette: &Palette,
    glyphs: GlyphMode,
) {
    let mut lines = vec![
        Line::from(Span::styled(
            format!("`{}`", modal.request().command),
            palette.accent_style(),
        )),
        Line::from(Span::styled(
            format!(
                "the policy in {} mode needs your answer before it runs",
                modal.request().mode.label()
            ),
            palette.muted_style(),
        )),
        Line::default(),
    ];
    for (index, (_, label)) in modal.options().into_iter().enumerate() {
        let chosen = index == modal.selected();
        let marker = if chosen { "> " } else { "  " };
        let style = if chosen {
            palette.accent_style()
        } else {
            palette.body_style()
        };
        lines.push(Line::from(Span::styled(format!("{marker}{label}"), style)));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "y allow once | a always | n refuse | enter choose | escape refuse",
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
