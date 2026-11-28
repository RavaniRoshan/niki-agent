//! `/settings`: the `[ui]` switches, applied live and written to the file.

use std::fs;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use niki_config::Config;
use niki_policy::Mode;
use niki_tui_live::{App, Command, SessionInfo};
use niki_tui_modals::Modal;

fn app(config_path: &std::path::Path) -> App {
    let mut app = App::new(SessionInfo {
        model: "mock:mock-model".to_owned(),
        mode: Mode::Manual,
        cwd: "/work".to_owned().into(),
    });
    app.set_config(Config::default(), config_path.to_path_buf());
    app
}

fn press(app: &mut App, code: KeyCode) -> Option<Command> {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn type_text(app: &mut App, text: &str) {
    for character in text.chars() {
        let _ = press(app, KeyCode::Char(character));
    }
}

fn open(app: &mut App) {
    type_text(app, "/settings");
    assert!(press(app, KeyCode::Enter).is_none());
    assert!(
        matches!(app.modal, Some(Modal::Settings(_))),
        "the list opened"
    );
}

#[test]
fn the_list_starts_on_what_the_session_is_using() {
    let directory = tempfile::tempdir().expect("tempdir");
    let mut app = app(&directory.path().join("config.toml"));
    open(&mut app);
    let Some(Modal::Settings(modal)) = &app.modal else {
        panic!("settings modal");
    };
    let rows = modal.rows();
    assert_eq!(rows[0], ("theme", "niki".to_owned()));
    assert_eq!(rows[1], ("motion", "full".to_owned()));
    assert_eq!(rows[3], ("welcome", "on".to_owned()));
}

#[test]
fn changing_the_theme_switches_the_palette_and_writes_the_key() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("config.toml");
    fs::write(&path, "[models]\ndefault = \"mock:mock-model\"\n").expect("seed");
    let mut app = app(&path);
    open(&mut app);

    assert!(press(&mut app, KeyCode::Enter).is_none());
    let text = fs::read_to_string(&path).expect("written");
    assert!(text.contains("[ui]\ntheme = \"high-contrast\""), "{text}");
    assert!(
        text.contains("[models]\ndefault = \"mock:mock-model\"\n"),
        "the model table survives: {text}"
    );
}

#[test]
fn a_switch_is_applied_to_the_screen_and_written_as_a_bool() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("config.toml");
    fs::write(&path, "[ui]\ntheme = \"niki\"\n").expect("seed");
    let mut app = app(&path);
    open(&mut app);

    for _ in 0..3 {
        press(&mut app, KeyCode::Down);
    }
    assert!(press(&mut app, KeyCode::Enter).is_none());
    assert!(!app.effects.welcome, "the block is gone for this session");
    let text = fs::read_to_string(&path).expect("written");
    assert!(text.contains("welcome = false"), "{text}");
    assert!(
        text.contains("theme = \"niki\""),
        "the earlier key is untouched: {text}"
    );
}

#[test]
fn without_a_configuration_file_the_change_still_applies() {
    let mut app = App::new(SessionInfo {
        model: "mock:mock-model".to_owned(),
        mode: Mode::Manual,
        cwd: "/work".to_owned().into(),
    });
    open(&mut app);
    press(&mut app, KeyCode::Down);
    assert!(press(&mut app, KeyCode::Enter).is_none());
    assert!(
        app.transcript
            .rows()
            .iter()
            .all(|row| row.kind != niki_tui_live::RowKind::Error)
    );
}
