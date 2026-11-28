//! What the screen draws, checked against a `TestBackend` buffer.
//!
//! No terminal is opened: the layout, the markers, the palette and the cursor
//! are all read back out of the buffer the widget tree painted.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use niki_agent::{AgentEvent, Usage};
use niki_policy::Mode;
use niki_tools::{AskOption, AskRequest};
use niki_tui_live::{App, Command, PLACEHOLDER, SessionInfo, TurnState, draw};
use niki_tui_themes::{Effects, GlyphMode, MONO, Palette};
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

fn question(allow_freeform: bool) -> AskRequest {
    AskRequest {
        question: "Which store should the new service use?".to_owned(),
        options: vec![
            AskOption {
                label: "SQLite".to_owned(),
                description: Some("one file, no server".to_owned()),
            },
            AskOption {
                label: "JSON".to_owned(),
                description: Some("a file per record".to_owned()),
            },
        ],
        allow_freeform,
    }
}

fn type_text(app: &mut App, text: &str) {
    for character in text.chars() {
        stroke(app, KeyCode::Char(character));
    }
}

#[test]
fn the_composer_rule_sits_above_the_two_line_footer() {
    let mut app = app();
    let buffer = render(&mut app, 60, 12);
    // Twelve rows: eight of transcript, the rule and one line of composer, then
    // the two-line footer. A fresh session fills the body with the welcome.
    let screen = text(&buffer);
    assert!(screen.contains("NIKI AGENT"), "{screen}");
    assert!(row(&buffer, 8).starts_with('─'), "{}", row(&buffer, 8));
    assert!(row(&buffer, 9).contains('›'), "{}", row(&buffer, 9));
    assert!(row(&buffer, 9).contains(PLACEHOLDER), "{}", row(&buffer, 9));
    assert!(
        row(&buffer, 11).contains("commands"),
        "{}",
        row(&buffer, 11)
    );
    let status = row(&buffer, 10);
    for field in ["mock:mock-model", "manual", "idle", "/work"] {
        assert!(status.contains(field), "{field} missing from {status:?}");
    }
    assert_eq!(app.status.turn, TurnState::Idle);
}

#[test]
fn a_fresh_session_shows_the_welcome_block() {
    let mut app = app();
    let buffer = render(&mut app, 80, 24);
    let screen = text(&buffer);
    for needle in [
        ">_ ",
        "NIKI AGENT",
        concat!("(v", env!("CARGO_PKG_VERSION"), ")"),
        "/work",
        "To get started, describe a task or try one of these commands:",
        "/init - write an AGENTS.md",
        "/status - model, mode, session and token usage",
        "/model - choose a model",
        "/theme - choose a palette",
        "/help - every command and key binding",
    ] {
        assert!(screen.contains(needle), "{needle} missing from\n{screen}");
    }
}

#[test]
fn a_narrow_terminal_keeps_whole_words() {
    let mut app = app();
    let buffer = render(&mut app, 40, 20);
    let screen = text(&buffer);
    // The intro wraps at word boundaries, so the closing word survives instead
    // of the line being cut mid-word at the right edge.
    assert!(screen.contains("commands:"), "{screen}");
    // A command summary that cannot fit ends in an ellipsis, not mid-word.
    let summary = (0..buffer.area.height)
        .map(|y| row(&buffer, y))
        .find(|line| line.starts_with("  /status"))
        .expect("/status row");
    assert!(summary.ends_with('…'), "{summary:?}");
}

#[test]
fn the_same_app_redraws_at_a_new_size() {
    let mut app = app();
    let wide = render(&mut app, 80, 24);
    assert!(
        text(&wide).contains("/help - every command and key binding"),
        "{}",
        text(&wide)
    );
    let narrow = render(&mut app, 40, 14);
    let screen = text(&narrow);
    assert!(screen.contains("NIKI AGENT"), "{screen}");
    assert!(screen.contains(&format!("› {PLACEHOLDER}")), "{screen}");
}

#[test]
fn the_welcome_block_yields_to_the_first_prompt() {
    let mut app = app();
    type_text(&mut app, "hi");
    let _ = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.apply(AgentEvent::Text("Hello.".to_owned()));
    let buffer = render(&mut app, 80, 24);
    let screen = text(&buffer);
    assert!(!screen.contains("To get started"), "{screen}");
    assert!(screen.contains("› hi"), "{screen}");
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
    assert!(screen.contains("• Working ("), "{screen}");
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
    assert!(screen.contains("✓ shell Cargo.toml"), "{screen}");
    assert!(screen.contains("× shell refused by policy"), "{screen}");
}

#[test]
fn the_cursor_rests_after_the_typed_text() {
    let mut app = app();
    type_text(&mut app, "abc");
    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| draw(frame, &mut app)).expect("draw");
    // The rule is row 8, the draft is row 9, and the `› ` prefix spends the
    // first two columns before the three typed characters.
    terminal.backend_mut().assert_cursor_position((5, 9));
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
fn the_composer_rule_can_be_drawn_in_ascii() {
    let mut app = app();
    app.set_glyphs(GlyphMode::Ascii);
    let buffer = render(&mut app, 40, 8);
    // Eight rows: four of transcript, the rule and draft, then the footer.
    let rule = row(&buffer, 4);
    assert!(rule.starts_with('-'), "{rule}");
    assert!(rule.is_ascii(), "{rule}");
    assert!(row(&buffer, 5).starts_with('>'), "{}", row(&buffer, 5));
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
    assert!(status.contains("48 tok"), "{status}");
}

#[test]
fn a_narrow_terminal_truncates_without_panicking() {
    let mut app = app();
    app.apply(AgentEvent::Text("a rather long answer ".repeat(20)));
    let buffer = render(&mut app, 12, 6);
    assert_eq!(buffer.area.width, 12);
    assert_eq!(buffer.area.height, 6);
    assert!(!row(&buffer, 4).is_empty(), "the state line keeps its row");
}

#[test]
fn a_tiny_terminal_still_draws() {
    // A 1x1 terminal leaves no room for a composer, a viewport and a bar; the
    // layout must not panic on it.
    let mut app = app();
    let buffer = render(&mut app, 1, 1);
    assert_eq!(buffer.area.height, 1);
}

#[test]
fn an_approval_prompt_covers_the_screen_and_names_the_command() {
    let mut app = app();
    app.request_approval(niki_agent::ApprovalRequest::command(
        "cargo publish --dry-run",
        Mode::Manual,
    ));
    let buffer = render(&mut app, 80, 20);
    let screen = text(&buffer);
    assert!(screen.contains("Approve a command"), "{screen}");
    assert!(screen.contains("cargo publish --dry-run"), "{screen}");
    assert!(screen.contains("> Allow this once"), "{screen}");
    assert!(screen.contains("Allow for this session"), "{screen}");
    assert!(screen.contains("Refuse it"), "{screen}");
    assert!(screen.contains("waiting for you"), "{screen}");
}

#[test]
fn an_approval_write_body_names_the_absolute_path_and_boundary() {
    let mut app = app();
    app.request_approval(niki_agent::ApprovalRequest::write(
        "write_file",
        "/outside/secret.txt".into(),
        false,
        Mode::Manual,
    ));
    let buffer = render(&mut app, 80, 20);
    let screen = text(&buffer);
    assert!(screen.contains("/outside/secret.txt"), "{screen}");
    assert!(
        screen.contains("resolves outside the workspace"),
        "{screen}"
    );
    assert!(screen.contains("Allow for this session"), "{screen}");
}

#[test]
fn a_question_prompt_shows_the_question_the_count_and_the_options() {
    let mut app = app();
    app.request_question(question(true));
    let buffer = render(&mut app, 80, 20);
    let screen = text(&buffer);
    assert!(screen.contains("Question"), "{screen}");
    assert!(
        screen.contains("Which store should the new service use?"),
        "{screen}"
    );
    assert!(screen.contains("2 options · single answer"), "{screen}");
    assert!(screen.contains("> SQLite"), "{screen}");
    assert!(screen.contains("one file, no server"), "{screen}");
    assert!(screen.contains("JSON"), "{screen}");
    assert!(screen.contains("type your own answer"), "{screen}");
    assert!(screen.contains("waiting for you"), "{screen}");
}

#[test]
fn a_question_highlight_follows_the_arrow_keys() {
    let mut app = app();
    app.request_question(question(false));
    stroke(&mut app, KeyCode::Down);
    let screen = text(&render(&mut app, 80, 20));
    assert!(screen.contains("> JSON"), "{screen}");
    assert!(!screen.contains("> SQLite"), "{screen}");
}

#[test]
fn a_long_question_on_a_twenty_by_six_terminal_draws_without_panicking() {
    let mut app = app();
    let mut request = question(true);
    request.question = "Why should this service use a new database at all? ".repeat(40);
    app.request_question(request);
    let buffer = render(&mut app, 20, 6);
    assert_eq!(buffer.area.width, 20);
    assert_eq!(buffer.area.height, 6);
    let screen = text(&buffer);
    assert!(!screen.trim().is_empty(), "the prompt still draws");
    assert!(screen.contains("Why"), "the question gets a row: {screen}");
    for line in screen.lines() {
        assert!(
            niki_render::display_width(line) <= 20,
            "line wider than the terminal: {line:?}"
        );
    }
}

#[test]
fn the_keymap_overlay_lists_the_shared_bindings() {
    let mut app = app();
    let _ = app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    let buffer = render(&mut app, 90, 30);
    let screen = text(&buffer);
    for needle in ["Keys", "Compose", "Session", "Transcript", "ctrl+c", "Quit"] {
        assert!(screen.contains(needle), "{needle} missing from\n{screen}");
    }
}

#[test]
fn the_model_picker_lists_the_configured_models() {
    let mut app = app();
    let config = niki_config::Config::from_str(
        "[models]\ndefault = \"mock:mock-model\"\n\n\
         [providers.mock]\ndisplay_name = \"Mock\"\nmodels = [\"mock-model\", \"mock-large\"]\n",
    )
    .expect("config");
    app.set_models(niki_tui_live::ModelPicker::from_config(&config));
    app.open_models();
    let buffer = render(&mut app, 80, 20);
    let screen = text(&buffer);
    assert!(screen.contains("> mock:mock-model"), "{screen}");
    assert!(screen.contains("mock:mock-large"), "{screen}");
    assert!(screen.contains("Mock"), "{screen}");
}

#[test]
fn the_session_browser_lists_a_preview_count_and_cwd() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store =
        niki_session::SessionStore::open(temp.path().join("sessions.db")).expect("session store");
    let session = store.create("/work").expect("session");
    store
        .append(&session, "user", "deploy the site")
        .expect("append");
    store.append(&session, "assistant", "done").expect("append");

    let mut app = app();
    app.attach_sessions(store);
    app.open_threads();

    let first = text(&render(&mut app, 80, 20));
    assert!(first.contains("Sessions"), "{first}");
    assert!(
        first.contains("(no prompt)"),
        "the session the launch opened has no prompt: {first}"
    );

    type_text(&mut app, "DEPLOY");
    let screen = text(&render(&mut app, 80, 20));
    assert!(screen.contains("filter: DEPLOY"), "{screen}");
    assert!(screen.contains("> deploy the site"), "{screen}");
    assert!(screen.contains("2 messages"), "{screen}");
    assert!(screen.contains("/work"), "{screen}");
    assert!(
        !screen.contains("(no prompt)"),
        "the filter leaves the prompted session: {screen}"
    );
}

#[test]
fn an_ascii_terminal_draws_the_overlay_without_box_drawing() {
    let mut app = app();
    app.set_glyphs(GlyphMode::Ascii);
    app.open_help();
    let buffer = render(&mut app, 90, 30);
    for line in text(&buffer).lines() {
        assert!(line.is_ascii(), "non-ASCII in {line:?}");
    }
}

#[test]
fn a_notice_sits_under_the_welcome_instead_of_hiding_it() {
    let mut app = app();
    app.apply(AgentEvent::Notice(
        "warning: this project is not trusted; run niki trust".to_owned(),
    ));
    let buffer = render(&mut app, 80, 30);
    let screen = text(&buffer);
    assert!(
        screen.contains("NIKI AGENT"),
        "the welcome survives a warning:\n{screen}"
    );
    assert!(
        screen.contains("- warning: this project is not trusted"),
        "the warning still shows:\n{screen}"
    );
    assert!(!app.started(), "a warning is not the start of a session");
}

#[test]
fn the_footer_hints_degrade_one_key_at_a_time() {
    let mut app = app();
    let wide = render(&mut app, 80, 16);
    let wide_row = row(&wide, 15);
    for hint in ["/ commands", "? keys", "ctrl+t transcript", "ctrl+d quit"] {
        assert!(wide_row.contains(hint), "{hint} missing from {wide_row:?}");
    }

    let narrow = render(&mut app, 40, 16);
    let narrow_row = row(&narrow, 15);
    assert!(
        niki_render::display_width(&narrow_row) <= 40,
        "{narrow_row:?}"
    );
    assert!(narrow_row.contains("/ commands"), "{narrow_row:?}");
    assert!(!narrow_row.contains("ctrl+d"), "{narrow_row:?}");
}

#[test]
fn the_footer_keeps_the_hints_and_puts_the_counts_on_the_right() {
    let mut app = app();
    type_text(&mut app, "hi");
    let _ = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.apply(AgentEvent::Usage(Usage {
        input_tokens: 4,
        output_tokens: 2,
    }));
    let buffer = render(&mut app, 80, 12);
    assert!(
        row(&buffer, 7).contains("Working"),
        "the live row carries the state: {:?}",
        row(&buffer, 7)
    );
    assert!(
        !row(&buffer, 10).contains("working"),
        "the state line does not repeat it: {:?}",
        row(&buffer, 10)
    );
    let footer = row(&buffer, 11);
    assert!(footer.contains("commands"), "the hints stay: {footer:?}");
    assert!(footer.contains("6 tok"), "{footer:?}");
    assert!(
        footer.trim_end().ends_with("6 tok"),
        "the counts sit on the right: {footer:?}"
    );
    assert!(
        footer.starts_with("/ commands"),
        "the hints sit on the left: {footer:?}"
    );
}

#[test]
fn a_narrow_footer_gives_the_row_to_the_counts() {
    let mut app = app();
    type_text(&mut app, "hi");
    let _ = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.apply(AgentEvent::Usage(Usage {
        input_tokens: 4,
        output_tokens: 2,
    }));
    let buffer = render(&mut app, 14, 12);
    let footer = row(&buffer, 11);
    assert_eq!(footer.trim(), "6 tok", "{footer:?}");
    assert!(!footer.contains("commands"), "{footer:?}");
}

#[test]
fn a_running_tool_call_carries_the_activity_marker() {
    let mut app = app();
    app.status.turn = TurnState::Working;
    app.apply(AgentEvent::ToolStart {
        name: "shell".to_owned(),
        arguments: "{\"command\":\"ls\"}".to_owned(),
    });
    let screen = text(&render(&mut app, 60, 12));
    assert!(screen.contains("• shell"), "{screen}");

    for _ in 0..7 {
        app.tick();
    }
    let dim = text(&render(&mut app, 60, 12));
    assert!(dim.contains("◦ shell"), "the marker dims: {dim}");
}

#[test]
fn the_welcome_effect_can_be_switched_off_without_losing_the_state() {
    let mut app = app();
    app.set_effects(Effects {
        welcome: false,
        ..Effects::ALL
    });
    let screen = text(&render(&mut app, 80, 24));
    assert!(!screen.contains("NIKI AGENT"), "{screen}");
    assert!(
        screen.contains("mock:mock-model"),
        "the footer still carries the state: {screen}"
    );
}

#[test]
fn the_command_list_is_drawn_under_the_composer() {
    let mut app = app();
    type_text(&mut app, "/t");
    let buffer = render(&mut app, 60, 20);
    let screen = text(&buffer);
    for name in ["/theme", "/threads", "/tools", "/compact", "/context"] {
        assert!(screen.contains(name), "{name} missing from {screen}");
    }
    assert!(
        screen.contains("Choose a palette, or cycle to the next one"),
        "the highlighted command's description: {screen}"
    );

    let lines: Vec<&str> = screen.lines().collect();
    let composer_top = lines
        .iter()
        .position(|line| line.starts_with('─'))
        .expect("the composer rule");
    let first_match = lines
        .iter()
        .position(|line| line.contains("/theme"))
        .expect("the list");
    assert!(
        first_match > composer_top,
        "the list sits under the composer: {screen}"
    );

    stroke(&mut app, KeyCode::Tab);
    let after = text(&render(&mut app, 60, 20));
    assert!(!after.contains("Choose a palette"), "{after}");
    assert!(after.contains("/theme "), "the completed draft: {after}");
}

#[test]
fn a_tiny_terminal_with_the_command_list_still_draws() {
    let mut app = app();
    type_text(&mut app, "/t");
    let buffer = render(&mut app, 12, 6);
    assert_eq!(buffer.area.width, 12);
    assert_eq!(buffer.area.height, 6);
}

#[test]
fn the_theme_command_repaints_with_the_new_palette() {
    let mut app = app();
    app.apply(AgentEvent::Text("recoloured".to_owned()));
    type_text(&mut app, "/theme mono");
    stroke(&mut app, KeyCode::Enter);
    assert_eq!(app.palette.name, "mono");
    let buffer = render(&mut app, 40, 10);
    assert_eq!(
        buffer.cell((2, 0)).expect("answer text").fg,
        niki_tui_themes::MONO.body.color()
    );
    let status = row(&buffer, 8);
    assert!(!status.is_empty());
}

#[test]
fn a_queued_prompt_is_drawn_dim_under_the_transcript_and_counted_in_the_bar() {
    let mut app = app();
    app.status.turn = TurnState::Working;
    type_text(&mut app, "second prompt");
    stroke(&mut app, KeyCode::Enter);

    let buffer = render(&mut app, 60, 14);
    let screen = text(&buffer);
    assert!(screen.contains("queued: second prompt"), "{screen}");
    assert!(
        row(&buffer, 12).contains("queued 1"),
        "the state line counts the queue: {}",
        row(&buffer, 12)
    );
    let cell = buffer.cell((0, 0)).expect("the queued marker");
    assert_eq!(cell.fg, app.palette.muted.color(), "it is drawn muted");
    assert!(
        screen
            .lines()
            .next()
            .is_some_and(|line| line.starts_with("queued: ")),
        "the row sits under an empty transcript: {screen}"
    );
}

fn ctrl_t(app: &mut App) {
    let _ = app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
}

/// A conversation longer than any viewport these tests use, so the top and the
/// bottom of it can never be on screen at the same time.
fn conversation(app: &mut App) {
    app.transcript.push_user("first prompt");
    for index in 0..20 {
        app.transcript
            .push_assistant(&format!("answer line {index}\n"));
        app.transcript.end_stream();
    }
    app.transcript.push_user("last prompt");
}

#[test]
fn ctrl_t_opens_at_the_newest_line_and_home_reaches_the_first_row() {
    let mut app = app();
    conversation(&mut app);

    ctrl_t(&mut app);
    assert!(app.transcript_overlay.is_some());

    let newest = text(&render(&mut app, 40, 12));
    assert!(newest.contains("transcript"), "{newest}");
    assert!(newest.contains("last prompt"), "{newest}");
    assert!(
        !newest.contains("first prompt"),
        "the view starts at the bottom: {newest}"
    );
    assert!(newest.contains("Esc close"), "the footer hint: {newest}");

    stroke(&mut app, KeyCode::Home);
    let top = text(&render(&mut app, 40, 12));
    assert!(top.contains("first prompt"), "{top}");
    assert!(top.contains("line 1/"), "the header names the top: {top}");
    assert!(!top.contains("last prompt"), "{top}");
}

#[test]
fn end_returns_to_the_last_row() {
    let mut app = app();
    conversation(&mut app);
    ctrl_t(&mut app);
    let newest = text(&render(&mut app, 40, 12));

    stroke(&mut app, KeyCode::Home);
    let top = text(&render(&mut app, 40, 12));
    assert_ne!(top, newest);

    stroke(&mut app, KeyCode::End);
    let back = text(&render(&mut app, 40, 12));
    assert_eq!(back, newest, "end shows the newest content again");
    assert!(back.contains("last prompt"), "{back}");
}

#[test]
fn escape_closes_the_overlay_and_restores_the_screen_and_state() {
    let mut app = app();
    conversation(&mut app);
    type_text(&mut app, "half typed");
    let before = text(&render(&mut app, 60, 16));
    let state = (
        app.composer.text().to_owned(),
        app.composer.row_col(),
        app.transcript.rows().to_vec(),
        app.status_line(),
        app.scroll.offset(),
        app.queued().len(),
        app.status.mode,
        app.status.turn,
        app.palette.name,
        app.completion().is_none(),
        app.history().is_browsing(),
    );

    ctrl_t(&mut app);
    let open = text(&render(&mut app, 60, 16));
    assert_ne!(open, before, "the overlay covers the screen");

    stroke(&mut app, KeyCode::Esc);
    assert!(app.transcript_overlay.is_none(), "escape closes it");
    let after = text(&render(&mut app, 60, 16));
    assert_eq!(after, before, "the screen comes back unchanged");
    assert_eq!(
        (
            app.composer.text().to_owned(),
            app.composer.row_col(),
            app.transcript.rows().to_vec(),
            app.status_line(),
            app.scroll.offset(),
            app.queued().len(),
            app.status.mode,
            app.status.turn,
            app.palette.name,
            app.completion().is_none(),
            app.history().is_browsing(),
        ),
        state,
        "only the overlay flag changed"
    );
}

#[test]
fn keys_while_the_overlay_is_open_never_reach_the_composer_or_send() {
    let mut app = app();
    conversation(&mut app);
    type_text(&mut app, "keep me");
    ctrl_t(&mut app);
    let rows = app.transcript.rows().to_vec();

    type_text(&mut app, "hello");
    assert_eq!(app.composer.text(), "keep me", "typing is dropped");
    assert_eq!(press(&mut app, KeyCode::Enter), None, "enter sends nothing");
    assert_eq!(app.composer.text(), "keep me");
    assert_eq!(app.transcript.rows(), rows.as_slice());
    assert!(app.transcript_overlay.is_some(), "the overlay stays open");
    assert_eq!(app.queued().len(), 0, "nothing was queued");

    assert_eq!(press(&mut app, KeyCode::Esc), None);
    assert!(app.transcript_overlay.is_none());
    assert_eq!(app.composer.text(), "keep me", "the draft survives");
}

#[test]
fn an_empty_transcript_shows_a_placeholder() {
    let mut app = app();
    let before = text(&render(&mut app, 40, 10));
    ctrl_t(&mut app);
    let screen = text(&render(&mut app, 40, 10));
    assert!(screen.contains("transcript"), "{screen}");
    assert!(screen.contains("nothing yet"), "{screen}");
    assert!(screen.contains("Esc close"), "{screen}");

    stroke(&mut app, KeyCode::Esc);
    assert_eq!(text(&render(&mut app, 40, 10)), before);
}

#[test]
fn the_overlay_renders_an_answer_with_the_markdown_renderer() {
    let mut app = app();
    app.transcript.push_user("plan it");
    app.transcript
        .push_assistant("## Plan\n\n**first** step and `code`");
    ctrl_t(&mut app);

    let screen = text(&render(&mut app, 60, 12));
    assert!(screen.contains("Plan"), "{screen}");
    assert!(screen.contains("first step and code"), "{screen}");
    assert!(
        !screen.contains("## Plan"),
        "the heading is rendered: {screen}"
    );
    assert!(!screen.contains("**first**"), "{screen}");
}

#[test]
fn the_status_card_draws_over_the_screen_and_fits_a_twenty_by_six_terminal() {
    let mut app = app();
    app.set_session_file("/home/somebody/.niki/sessions.db");
    app.open_status();
    let buffer = render(&mut app, 20, 6);
    assert_eq!(buffer.area.width, 20);
    assert_eq!(buffer.area.height, 6);
    let screen = text(&buffer);
    assert!(screen.contains("Status"), "{screen}");
    assert!(screen.contains("model"), "{screen}");

    app.open_status();
    assert!(app.modal.is_none(), "a second open closes the card");
    let after = text(&render(&mut app, 20, 6));
    assert!(!after.contains("Status"), "{after}");
}

fn press(app: &mut App, code: KeyCode) -> Option<Command> {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn a_busy_turn_gets_a_clocked_row_above_the_composer() {
    let mut app = app();
    app.status.turn = TurnState::Working;
    let buffer = render(&mut app, 60, 12);
    // Twelve rows: seven of transcript, the live row, the rule and one line of
    // composer, then the two-line footer.
    let live = row(&buffer, 7);
    assert!(live.contains("Working (0s"), "{live:?}");
    assert!(live.contains("esc to interrupt"), "{live:?}");
    assert!(row(&buffer, 8).starts_with('─'), "{}", row(&buffer, 8));

    app.status.turn = TurnState::Idle;
    let buffer = render(&mut app, 60, 12);
    assert!(
        !row(&buffer, 7).contains("esc to interrupt"),
        "the row goes away with the turn: {:?}",
        row(&buffer, 7)
    );
}

#[test]
fn the_help_overlay_opens_in_three_columns_on_a_wide_screen() {
    let mut app = app();
    let _ = app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    let buffer = render(&mut app, 120, 40);
    let screen = text(&buffer);
    let header = (0..buffer.area.height)
        .map(|y| row(&buffer, y))
        .find(|line| line.contains("Compose"))
        .unwrap_or_default();
    for needle in ["Compose", "Session", "Transcript"] {
        assert!(header.contains(needle), "{needle} not in header {header:?}");
    }
    assert!(screen.contains("Commands"), "{screen}");
}
