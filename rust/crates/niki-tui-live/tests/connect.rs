//! `/connect`: the walk, the files it writes, and the model it switches to.

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

#[test]
fn a_fragment_of_the_command_opens_the_walk() {
    let directory = tempfile::tempdir().expect("tempdir");
    let mut app = app(&directory.path().join("config.toml"));
    type_text(&mut app, "/con");
    assert!(press(&mut app, KeyCode::Enter).is_none());
    assert!(
        matches!(app.modal, Some(Modal::Connect(_))),
        "enter adopted the highlighted /connect"
    );
}

#[test]
fn the_first_provider_writes_the_key_and_the_default_model() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("config.toml");
    let mut app = app(&path);

    type_text(&mut app, "/connect");
    assert!(press(&mut app, KeyCode::Enter).is_none());
    assert!(
        matches!(app.modal, Some(Modal::Connect(_))),
        "the walk opened"
    );

    // The catalog starts on Anthropic: choose it, paste a key, name a model.
    assert!(press(&mut app, KeyCode::Enter).is_none());
    type_text(&mut app, "sk-ant-test");
    assert!(press(&mut app, KeyCode::Enter).is_none());
    type_text(&mut app, "claude-sonnet-4-5");
    let command = press(&mut app, KeyCode::Enter);

    assert_eq!(
        command,
        Some(Command::SelectModel(
            "anthropic:claude-sonnet-4-5".to_owned()
        ))
    );
    assert!(app.modal.is_none(), "the walk closed");

    let credentials =
        fs::read_to_string(directory.path().join("credentials.toml")).expect("credentials written");
    assert!(credentials.contains("sk-ant-test"), "{credentials}");

    let config = fs::read_to_string(&path).expect("config written");
    assert!(
        config.contains("default = \"anthropic:claude-sonnet-4-5\""),
        "{config}"
    );
    assert!(
        !config.contains("sk-ant-test"),
        "the key never lands in config.toml: {config}"
    );
}

#[test]
fn a_custom_endpoint_adds_its_block() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("config.toml");
    let mut app = app(&path);

    type_text(&mut app, "/connect");
    press(&mut app, KeyCode::Enter);
    // The custom row is last.
    let _ = press(&mut app, KeyCode::End);
    press(&mut app, KeyCode::Enter);
    type_text(&mut app, "https://example.test/v1");
    press(&mut app, KeyCode::Enter);
    // Enter on an empty key marks the endpoint keyless.
    press(&mut app, KeyCode::Enter);
    type_text(&mut app, "my-model");
    let command = press(&mut app, KeyCode::Enter);

    assert_eq!(
        command,
        Some(Command::SelectModel("custom:my-model".to_owned()))
    );
    let config = fs::read_to_string(&path).expect("config written");
    assert!(config.contains("[providers.custom]"), "{config}");
    assert!(config.contains("https://example.test/v1"), "{config}");
    assert!(config.contains("keyless = true"), "{config}");
    assert!(config.contains("default = \"custom:my-model\""), "{config}");
}

#[test]
fn an_existing_default_is_replaced_not_duplicated() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("config.toml");
    fs::write(&path, "[models]\ndefault = \"old:model\"\n# keep me\n").expect("seed");
    let mut app = app(&path);

    type_text(&mut app, "/connect");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    // An empty key means "use the environment variable".
    press(&mut app, KeyCode::Enter);
    type_text(&mut app, "m");
    press(&mut app, KeyCode::Enter);

    let config = fs::read_to_string(&path).expect("config written");
    assert_eq!(
        config.matches("default =").count(),
        1,
        "one default line: {config}"
    );
    assert!(config.contains("# keep me"), "comments survive: {config}");
    assert!(!config.contains("old:model"), "{config}");
}
