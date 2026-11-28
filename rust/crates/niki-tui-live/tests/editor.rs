//! The composer's three everyday interactions: completing a command,
//! recalling a prompt, and copying the answer.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use niki_agent::AgentEvent;
use niki_policy::Mode;
use niki_tui_live::clipboard;
use niki_tui_live::{App, Command, History, RowKind, SessionInfo, TurnState};

fn app() -> App {
    App::new(SessionInfo {
        model: "mock:mock-model".to_owned(),
        mode: Mode::Manual,
        cwd: "/work".to_owned().into(),
    })
}

fn press(app: &mut App, code: KeyCode) -> Option<Command> {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn stroke(app: &mut App, code: KeyCode) {
    let _ = press(app, code);
}

fn type_text(app: &mut App, text: &str) {
    for character in text.chars() {
        stroke(app, KeyCode::Char(character));
    }
}

fn control(app: &mut App, character: char) -> Option<Command> {
    app.handle_key(KeyEvent::new(
        KeyCode::Char(character),
        KeyModifiers::CONTROL,
    ))
}

fn names(app: &App) -> Vec<&'static str> {
    app.completion()
        .map(|completion| {
            completion
                .commands()
                .iter()
                .map(|command| command.name)
                .collect()
        })
        .unwrap_or_default()
}

fn highlighted(app: &App) -> &'static str {
    app.completion()
        .map(|completion| completion.selected().name)
        .unwrap_or_default()
}

/// Send a prompt as the screen does, then let the next one in.
fn send(app: &mut App, prompt: &str) -> Option<Command> {
    type_text(app, prompt);
    let command = press(app, KeyCode::Enter);
    app.status.turn = TurnState::Idle;
    command
}

#[test]
fn the_list_puts_prefix_matches_before_substring_matches() {
    let mut app = app();
    type_text(&mut app, "/t");
    assert_eq!(
        names(&app),
        [
            "/theme", "/threads", "/tools", "/auto", "/compact", "/connect"
        ]
    );
    assert_eq!(
        highlighted(&app),
        "/theme",
        "a list opened from `/` starts on its first row, and the narrowed list does too"
    );

    let mut kept = self::app();
    type_text(&mut kept, "/t");
    stroke(&mut kept, KeyCode::Down);
    assert_eq!(highlighted(&kept), "/threads");
    type_text(&mut kept, "h");
    assert_eq!(
        highlighted(&kept),
        "/threads",
        "a highlight the person moved stays on its command while it is listed"
    );
}

#[test]
fn the_list_is_capped_at_six_commands_in_registry_order() {
    let mut app = app();
    type_text(&mut app, "/");
    assert_eq!(
        names(&app),
        [
            "/auto",
            "/clear",
            "/compact",
            "/connect",
            "/context",
            "/context-doctor"
        ]
    );
    assert_eq!(app.completion_height(), 7, "six names and the description");
}

#[test]
fn matching_ignores_the_slash_and_the_case_of_the_draft() {
    let mut app = app();
    type_text(&mut app, "/HE");
    assert_eq!(names(&app), ["/help", "/theme", "/threads"]);

    let mut app = self::app();
    type_text(&mut app, "/le");
    assert_eq!(names(&app), ["/clear"]);
}

#[test]
fn enter_runs_the_command_the_list_highlights() {
    let mut app = app();
    type_text(&mut app, "/vers");
    assert_eq!(press(&mut app, KeyCode::Enter), None);
    assert_eq!(
        app.composer.text(),
        "",
        "the command ran rather than being sent"
    );
    let last = app.transcript.rows().last().expect("a notice");
    assert!(
        last.text.starts_with("Niki Agent "),
        "the version line ran: {}",
        last.text
    );

    let mut app = self::app();
    type_text(&mut app, "/hel");
    assert_eq!(press(&mut app, KeyCode::Enter), None);
    assert!(matches!(app.modal, Some(niki_tui_live::Modal::Help(_))));
}

#[test]
fn a_space_or_plain_text_closes_the_list() {
    let mut app = app();
    type_text(&mut app, "/theme");
    assert_eq!(names(&app), ["/theme"]);
    type_text(&mut app, " mono");
    assert!(app.completion().is_none());

    let mut app = self::app();
    type_text(&mut app, "list the files");
    assert!(app.completion().is_none());
}

#[test]
fn tab_completes_the_highlighted_name_and_a_space() {
    let mut app = app();
    type_text(&mut app, "/he");
    assert_eq!(press(&mut app, KeyCode::Tab), None);
    assert_eq!(app.composer.text(), "/help ");
    assert!(app.completion().is_none());

    let mut app = self::app();
    type_text(&mut app, "/he");
    stroke(&mut app, KeyCode::Down);
    assert_eq!(highlighted(&app), "/theme");
    stroke(&mut app, KeyCode::Tab);
    assert_eq!(app.composer.text(), "/theme ");
}

#[test]
fn the_arrows_move_the_highlight_while_the_list_is_open() {
    let mut app = app();
    type_text(&mut app, "/");
    stroke(&mut app, KeyCode::Up);
    assert_eq!(highlighted(&app), "/auto", "the first row is the top");

    stroke(&mut app, KeyCode::Down);
    assert_eq!(highlighted(&app), "/clear");
    for _ in 0..10 {
        stroke(&mut app, KeyCode::Down);
    }
    assert_eq!(
        highlighted(&app),
        "/context-doctor",
        "the last row is the bottom"
    );
    assert_eq!(app.composer.text(), "/", "the draft is untouched");
}

#[test]
fn escape_closes_the_list_and_keeps_the_draft() {
    let mut app = app();
    type_text(&mut app, "/c");
    assert!(app.completion().is_some());
    assert_eq!(press(&mut app, KeyCode::Esc), None);
    assert!(app.completion().is_none());
    assert_eq!(app.composer.text(), "/c");

    type_text(&mut app, "l");
    assert_eq!(
        names(&app),
        ["/clear"],
        "typing reopens the list for the new draft"
    );

    assert_eq!(press(&mut app, KeyCode::Esc), None);
    assert_eq!(press(&mut app, KeyCode::Esc), None);
    assert!(app.composer.is_empty(), "the second press clears the draft");
}

#[test]
fn enter_submits_the_draft_as_typed() {
    let mut app = app();
    // A word the list cannot match is sent as typed and reported.
    type_text(&mut app, "/zzz");
    assert!(app.completion().is_none());
    assert_eq!(press(&mut app, KeyCode::Enter), None);
    assert!(app.composer.is_empty());
    let last = app.transcript.rows().last().expect("a notice");
    assert_eq!(last.kind, RowKind::Notice);
    assert!(last.text.contains("/zzz"), "{}", last.text);

    let mut app = self::app();
    type_text(&mut app, "/help");
    assert!(app.completion().is_some());
    assert_eq!(press(&mut app, KeyCode::Enter), None);
    assert!(app.modal.is_some(), "the complete command ran");
}

#[test]
fn arrows_still_move_the_cursor_through_a_multiline_draft() {
    let mut app = app();
    type_text(&mut app, "one");
    let _ = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT));
    type_text(&mut app, "two");
    assert_eq!(app.composer.row_col(), (1, 3));
    stroke(&mut app, KeyCode::Up);
    assert_eq!(app.composer.row_col(), (0, 3));
    stroke(&mut app, KeyCode::Down);
    assert_eq!(app.composer.row_col(), (1, 3));
    assert!(app.completion().is_none());
}

#[test]
fn up_recalls_prompts_newest_first_and_down_returns_to_the_draft() {
    let mut app = app();
    assert_eq!(
        send(&mut app, "first prompt"),
        Some(Command::Submit("first prompt".to_owned()))
    );
    assert_eq!(
        send(&mut app, "second prompt"),
        Some(Command::Submit("second prompt".to_owned()))
    );

    stroke(&mut app, KeyCode::Up);
    assert_eq!(app.composer.text(), "second prompt");
    assert!(app.history().is_browsing());
    stroke(&mut app, KeyCode::Up);
    assert_eq!(app.composer.text(), "first prompt");
    stroke(&mut app, KeyCode::Up);
    assert_eq!(app.composer.text(), "first prompt", "the oldest is last");
    stroke(&mut app, KeyCode::Down);
    assert_eq!(app.composer.text(), "second prompt");
    stroke(&mut app, KeyCode::Down);
    assert!(app.composer.is_empty(), "down ends on the draft");
    assert!(!app.history().is_browsing());
}

#[test]
fn a_non_empty_draft_keeps_its_arrow_keys() {
    let mut app = app();
    assert_eq!(
        send(&mut app, "an earlier prompt"),
        Some(Command::Submit("an earlier prompt".to_owned()))
    );

    type_text(&mut app, "draft");
    stroke(&mut app, KeyCode::Up);
    assert_eq!(app.composer.text(), "draft");
    assert_eq!(
        app.composer.row_col(),
        (0, 0),
        "the cursor moved, not history"
    );
    assert!(!app.history().is_browsing());
}

#[test]
fn the_history_file_round_trips_and_skips_a_corrupt_line() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("missing/niki/history.jsonl");
    let mut app = app();
    app.attach_history(History::open(&path));
    send(&mut app, "first");
    send(&mut app, "second");

    let stored = std::fs::read_to_string(&path).expect("the history file");
    assert_eq!(stored.lines().count(), 2, "{stored:?}");

    let mut relaunched = self::app();
    relaunched.attach_history(History::open(&path));
    stroke(&mut relaunched, KeyCode::Up);
    assert_eq!(relaunched.composer.text(), "second");
    stroke(&mut relaunched, KeyCode::Up);
    assert_eq!(relaunched.composer.text(), "first");

    std::fs::write(&path, format!("{stored}\n\"third\"\nnot json\n")).expect("corrupt line");
    let reloaded = History::open(&path);
    assert_eq!(reloaded.entries(), ["first", "second", "third"]);
}

#[test]
fn blanks_and_an_immediate_repeat_are_not_recorded() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("history.jsonl");
    let mut app = app();
    app.attach_history(History::open(&path));

    assert_eq!(send(&mut app, "   "), None);
    assert!(app.history().entries().is_empty());
    app.composer.clear();

    send(&mut app, "same");
    send(&mut app, "same");
    send(&mut app, "other");
    assert_eq!(app.history().entries(), ["same", "other"]);
    assert_eq!(
        std::fs::read_to_string(&path)
            .expect("history file")
            .lines()
            .count(),
        2
    );
}

#[test]
fn ctrl_y_copies_the_latest_answer_once() {
    let mut app = app();
    app.set_clipboard(true);
    app.apply(AgentEvent::Text("first answer".to_owned()));
    app.apply(AgentEvent::ToolStart {
        name: "shell".to_owned(),
        arguments: "{\"command\":\"ls\"}".to_owned(),
    });
    app.apply(AgentEvent::ToolEnd {
        name: "shell".to_owned(),
        output: "a.txt".to_owned(),
        is_error: false,
    });
    app.apply(AgentEvent::Text("the answer".to_owned()));

    let command = control(&mut app, 'y');
    let Some(Command::Copy(text)) = command else {
        panic!("expected a copy, got {command:?}");
    };
    assert_eq!(text, "the answer");
    assert_eq!(
        app.composer.text(),
        "",
        "ctrl+y is not typed into the draft"
    );

    let mut out = Vec::new();
    clipboard::copy(&mut out, &text).expect("write the escape");
    let sequence = String::from_utf8(out).expect("utf-8");
    assert_eq!(sequence, "\x1b]52;c;dGhlIGFuc3dlcg==\x1b\\");
    assert_eq!(
        sequence.matches("\x1b]52;c;").count(),
        1,
        "one escape per copy"
    );

    let last = app.transcript.rows().last().expect("a notice");
    assert_eq!(last.kind, RowKind::Notice);
    assert_eq!(last.text, "copy: 10 characters");
}

#[test]
fn a_disabled_clipboard_reports_the_length_and_writes_nothing() {
    let mut app = app();
    app.set_clipboard(false);
    app.apply(AgentEvent::Text("hello".to_owned()));
    assert_eq!(control(&mut app, 'y'), None);
    let last = app.transcript.rows().last().expect("a notice");
    assert_eq!(last.kind, RowKind::Notice);
    assert_eq!(last.text, "copy: 5 characters (clipboard disabled)");
}

#[test]
fn copying_with_no_answer_says_so() {
    let mut app = app();
    assert_eq!(control(&mut app, 'y'), None);
    let last = app.transcript.rows().last().expect("a notice");
    assert_eq!(last.text, "no answer to copy yet");
}

#[test]
fn a_bang_line_is_not_a_slash_command() {
    let mut app = app();
    type_text(&mut app, "!echo hi");
    assert!(
        app.completion().is_none(),
        "completion must not offer commands for a shell line: {:?}",
        app.completion()
    );

    let mut app = self::app();
    type_text(&mut app, "!");
    assert!(app.completion().is_none(), "{:?}", app.completion());
}

#[test]
fn ctrl_n_starts_a_fresh_session_and_drops_the_old_view() {
    let mut app = app();
    app.transcript.push_notice("an old row");
    assert_eq!(control(&mut app, 'n'), Some(Command::ClearApprovals));
    let texts: Vec<&str> = app
        .transcript
        .rows()
        .iter()
        .map(|row| row.text.as_str())
        .collect();
    assert_eq!(texts, ["started a fresh session"]);
    assert_eq!(app.composer.text(), "", "the chord is not typed");
}

#[test]
fn ctrl_r_opens_the_session_browser() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store =
        niki_session::SessionStore::open(temp.path().join("sessions.db")).expect("session store");
    let mut app = app();
    app.attach_sessions(store);
    assert_eq!(control(&mut app, 'r'), None);
    assert!(
        matches!(app.modal, Some(niki_tui_live::Modal::Threads(_))),
        "ctrl+r must open the picker"
    );
}

#[test]
fn ctrl_r_without_a_store_says_sessions_are_unavailable() {
    let mut app = app();
    assert_eq!(control(&mut app, 'r'), None);
    let last = app.transcript.rows().last().expect("a notice");
    assert_eq!(last.text, "no sessions are saved in this build");
}
