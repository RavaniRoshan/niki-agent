//! What the screen draws, checked against a `TestBackend` buffer.
//!
//! No terminal is opened: the layout, the markers, the palette and the cursor
//! are all read back out of the buffer the widget tree painted.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use niki_agent::{AgentEvent, Usage};
use niki_policy::Mode;
use niki_tui_live::{App, Command, PLACEHOLDER, SessionInfo, TurnState, draw};
use niki_tui_themes::{GlyphMode, MONO, Palette};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

fn app() -> App {
    App::new(SessionInfo {
        model: "mock:mock-model".to_owned(),
        mode: Mode::Manual,
        cwd: "/work".to_owned().into(),
    })
}

fn render(app: &mut App, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| draw(frame, app)).expect("draw");
    terminal.backend().buffer().clone()
}

fn row(buffer: &Buffer, y: u16) -> String {
    let mut text = String::new();
    for x in 0..buffer.area.width {
        text.push_str(buffer.cell((x, y)).map_or(" ", |cell| cell.symbol()));
    }
    text.trim_end().to_owned()
}

fn text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| row(buffer, y))
        .collect::<Vec<_>>()
        .join("\n")
}

fn stroke(app: &mut App, code: KeyCode) {
    let _ = app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn type_text(app: &mut App, text: &str) {
    for character in text.chars() {
        stroke(app, KeyCode::Char(character));
    }
}

#[test]
fn the_composer_sits_above_the_status_bar() {
    let mut app = app();
    let buffer = render(&mut app, 60, 12);
    // Twelve rows: nine of transcript, two of composer frame plus one of text,
    // and the status bar.
    assert_eq!(row(&buffer, 0), "");
    assert!(row(&buffer, 8).starts_with('╭'), "{}", row(&buffer, 8));
    assert!(row(&buffer, 9).contains(PLACEHOLDER), "{}", row(&buffer, 9));
    assert!(row(&buffer, 10).starts_with('╰'), "{}", row(&buffer, 10));
    let status = row(&buffer, 11);
    for field in ["mock:mock-model", "manual", "idle", "/work"] {
        assert!(status.contains(field), "{field} missing from {status:?}");
    }
    assert_eq!(app.status.turn, TurnState::Idle);
}

#[test]
fn a_prompt_and_its_answer_are_drawn_in_the_transcript() {
    let mut app = app();
    type_text(&mut app, "hi");
    assert_eq!(
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Some(Command::Submit("hi".to_owned()))
    );
    app.apply(AgentEvent::Text("Hello ".to_owned()));
    app.apply(AgentEvent::Text("there.".to_owned()));
    let buffer = render(&mut app, 60, 12);
    let screen = text(&buffer);
    assert!(screen.contains("› hi"), "{screen}");
    assert!(screen.contains("⏺ Hello there."), "{screen}");
    assert!(screen.contains("working"), "{screen}");
}

#[test]
fn tool_rows_and_failures_carry_their_markers() {
    let mut app = app();
    app.apply(AgentEvent::ToolStart {
        name: "shell".to_owned(),
        arguments: "{\"command\":\"ls\"}".to_owned(),
    });
    app.apply(AgentEvent::ToolEnd {
        name: "shell".to_owned(),
        output: "Cargo.toml".to_owned(),
        is_error: false,
    });
    app.apply(AgentEvent::ToolEnd {
        name: "shell".to_owned(),
        output: "refused by policy".to_owned(),
        is_error: true,
    });
    let buffer = render(&mut app, 60, 14);
    let screen = text(&buffer);
    assert!(screen.contains("⚙ shell {\"command\":\"ls\"}"), "{screen}");
    assert!(screen.contains("⚙ shell Cargo.toml"), "{screen}");
    assert!(screen.contains("! shell refused by policy"), "{screen}");
}

#[test]
fn the_cursor_rests_after_the_typed_text() {
    let mut app = app();
    type_text(&mut app, "abc");
    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| draw(frame, &mut app)).expect("draw");
    // The composer frame starts at row 8, so its text starts at row 9 and the
    // border occupies column 0.
    terminal.backend_mut().assert_cursor_position((4, 9));
}

#[test]
fn the_palette_colours_the_body_and_the_bar() {
    let mut app = app();
    app.apply(AgentEvent::Text("coloured".to_owned()));
    let buffer = render(&mut app, 40, 10);
    let body_cell = buffer.cell((2, 0)).expect("first answer letter");
    assert_eq!(body_cell.fg, app.palette.body.color());
    let status_y = buffer.area.height - 1;
    let status_cell = buffer.cell((0, status_y)).expect("status bar");
    assert_eq!(status_cell.bg, app.palette.panel.color());

    static MONO_PALETTE: Palette = MONO;
    app.set_palette(&MONO_PALETTE);
    let buffer = render(&mut app, 40, 10);
    assert_eq!(
        buffer.cell((2, 0)).expect("first answer letter").fg,
        MONO_PALETTE.body.color()
    );
}

#[test]
fn an_ascii_terminal_is_drawn_with_no_unicode_at_all() {
    let mut app = app();
    app.set_glyphs(GlyphMode::Ascii);
    app.apply(AgentEvent::Text("plain".to_owned()));
    let buffer = render(&mut app, 60, 12);
    let screen = text(&buffer);
    assert!(
        screen.contains("> plain") || screen.contains("* plain"),
        "{screen}"
    );
    for line in screen.lines() {
        assert!(line.is_ascii(), "non-ASCII in {line:?}");
    }
}

#[test]
fn the_composer_border_can_be_drawn_in_ascii() {
    let mut app = app();
    app.set_glyphs(GlyphMode::Ascii);
    let buffer = render(&mut app, 40, 8);
    // Eight rows: four of transcript, the three-row composer frame and the bar.
    let frame = row(&buffer, 4);
    assert!(frame.starts_with('+'), "{frame}");
    assert!(frame.is_ascii(), "{frame}");
}

#[test]
fn page_up_shows_older_rows_and_page_down_returns_to_the_newest() {
    let mut app = app();
    for line in 0..60 {
        app.apply(AgentEvent::Text(format!("line {line}\n")));
    }
    let buffer = render(&mut app, 40, 12);
    let newest = text(&buffer);
    assert!(newest.contains("line 59"), "{newest}");
    assert!(!newest.contains("line 0\n"), "{newest}");

    stroke(&mut app, KeyCode::PageUp);
    stroke(&mut app, KeyCode::PageUp);
    let scrolled = text(&render(&mut app, 40, 12));
    assert_ne!(scrolled, newest);
    assert!(app.scroll.offset() > 0);

    for _ in 0..10 {
        stroke(&mut app, KeyCode::PageDown);
    }
    let back = text(&render(&mut app, 40, 12));
    assert_eq!(back, newest);
    assert_eq!(app.scroll.offset(), 0);
}

#[test]
fn the_status_bar_reports_tokens_once_the_provider_does() {
    let mut app = app();
    app.apply(AgentEvent::Usage(Usage {
        input_tokens: 40,
        output_tokens: 8,
    }));
    let buffer = render(&mut app, 80, 10);
    let status = row(&buffer, 9);
    assert!(status.contains("40 in / 8 out"), "{status}");
}

#[test]
fn a_narrow_terminal_truncates_without_panicking() {
    let mut app = app();
    app.apply(AgentEvent::Text("a rather long answer ".repeat(20)));
    let buffer = render(&mut app, 12, 6);
    assert_eq!(buffer.area.width, 12);
    assert_eq!(buffer.area.height, 6);
    assert!(!row(&buffer, 5).is_empty(), "the status bar keeps its row");
}

#[test]
fn a_tiny_terminal_still_draws() {
    // A 1x1 terminal leaves no room for a composer, a viewport and a bar; the
    // layout must not panic on it.
    let mut app = app();
    let buffer = render(&mut app, 1, 1);
    assert_eq!(buffer.area.height, 1);
}
