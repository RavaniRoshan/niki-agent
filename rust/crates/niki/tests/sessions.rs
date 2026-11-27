//! `niki sessions list` end to end: no terminal, a temp `NIKI_HOME`, and a
//! store seeded directly through `niki-session`.

use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

use niki_session::{SessionId, SessionStore};
use serde_json::Value;

fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["sessions", "list"])
        .args(args)
        .env("NIKI_HOME", home)
        .output()
        .expect("run niki sessions list")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn seed(home: &Path, workspace: &Path, title: Option<&str>, messages: usize) -> SessionId {
    let store = SessionStore::open(home.join("sessions.db")).expect("store");
    let session = store.create(workspace).expect("create");
    if let Some(title) = title {
        store.set_title(&session, Some(title)).expect("title");
    }
    for index in 0..messages {
        store
            .append(&session, "user", &format!("message {index}"))
            .expect("append");
    }
    session
}

#[test]
fn an_empty_store_prints_the_empty_form_and_creates_nothing() {
    let home = tempfile::tempdir().expect("home");

    let output = run(home.path(), &[]);
    let text = stdout(&output);

    assert_eq!(output.status.code(), Some(0), "{text}");
    assert_eq!(text, "no saved sessions\n");
    assert!(
        !home.path().join("sessions.db").exists(),
        "listing must not create the store"
    );
}

#[test]
fn an_empty_store_prints_an_empty_json_array() {
    let home = tempfile::tempdir().expect("home");

    let output = run(home.path(), &["--json"]);
    let text = stdout(&output);

    assert_eq!(output.status.code(), Some(0), "{text}");
    assert_eq!(text.trim(), "[]");
    assert!(!home.path().join("sessions.db").exists());
}

#[test]
fn a_missing_home_is_empty_and_is_not_created() {
    let parent = tempfile::tempdir().expect("parent");
    let home = parent.path().join("missing-home");

    let output = run(&home, &[]);

    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    assert_eq!(stdout(&output), "no saved sessions\n");
    assert!(!home.exists(), "listing must not create the config home");
}

#[test]
fn a_saved_session_shows_its_id_title_and_message_count() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let session = seed(home.path(), workspace.path(), Some("port the parser"), 2);

    let output = run(home.path(), &[]);
    let text = stdout(&output);

    assert_eq!(output.status.code(), Some(0), "{text}");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 1, "{text}");
    let fields: Vec<&str> = lines[0].split("  ").collect();
    assert_eq!(fields.len(), 4, "{:?}", lines[0]);
    assert_eq!(fields[0], session.as_str());
    assert!(
        fields[1].parse::<i64>().expect("Unix seconds") > 0,
        "{}",
        fields[1]
    );
    assert_eq!(fields[2], "port the parser");
    assert_eq!(fields[3], "2 messages");
}

#[test]
fn a_session_without_a_title_says_untitled() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    seed(home.path(), workspace.path(), None, 0);

    let text = stdout(&run(home.path(), &[]));

    assert!(text.contains("(untitled)  0 messages"), "{text}");
}

#[test]
fn the_json_form_has_the_documented_fields() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let session = seed(home.path(), workspace.path(), Some("port the parser"), 3);

    let output = run(home.path(), &["--json"]);
    let text = stdout(&output);

    assert_eq!(output.status.code(), Some(0), "{text}");
    let rows: Value = serde_json::from_str(text.trim()).expect("JSON array");
    let rows = rows.as_array().expect("array");
    assert_eq!(rows.len(), 1, "{text}");
    let row = &rows[0];
    assert_eq!(row["id"], session.as_str());
    assert_eq!(row["title"], "port the parser");
    assert_eq!(row["messages"], 3);
    assert_eq!(
        row["workspace"].as_str(),
        workspace.path().to_str(),
        "{row}"
    );
    assert!(
        row["updated_at"].as_i64().expect("Unix seconds") > 0,
        "{row}"
    );
}

#[test]
fn an_untitled_session_is_null_in_json() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    seed(home.path(), workspace.path(), None, 1);

    let text = stdout(&run(home.path(), &["--json"]));
    let rows: Value = serde_json::from_str(text.trim()).expect("JSON array");

    assert!(rows[0]["title"].is_null(), "{text}");
}

#[test]
fn root_keeps_only_that_workspace() {
    let home = tempfile::tempdir().expect("home");
    let first = tempfile::tempdir().expect("first workspace");
    let second = tempfile::tempdir().expect("second workspace");
    let first_session = seed(home.path(), first.path(), Some("first session"), 1);
    let second_session = seed(home.path(), second.path(), Some("second session"), 1);
    let root = first.path().to_str().expect("UTF-8 workspace");

    let output = run(home.path(), &["--root", root]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}");
    assert!(text.contains(first_session.as_str()), "{text}");
    assert!(text.contains("first session"), "{text}");
    assert!(!text.contains(second_session.as_str()), "{text}");
    assert!(!text.contains("second session"), "{text}");

    let text = stdout(&run(home.path(), &["--root", root, "--json"]));
    let rows: Value = serde_json::from_str(text.trim()).expect("JSON array");
    let rows = rows.as_array().expect("array");
    assert_eq!(rows.len(), 1, "{text}");
    assert_eq!(rows[0]["id"], first_session.as_str());
}

#[test]
fn the_session_written_most_recently_comes_first() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let older = seed(home.path(), workspace.path(), Some("older"), 0);
    let newer = seed(home.path(), workspace.path(), Some("newer"), 0);

    // A later message is a later write, even though the session was created
    // first. Sleeping past the one-second clock keeps the two stamps distinct.
    std::thread::sleep(Duration::from_millis(1100));
    let store = SessionStore::open(home.path().join("sessions.db")).expect("store");
    store
        .append(&older, "user", "bump the transcript")
        .expect("append");

    let text = stdout(&run(home.path(), &[]));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].starts_with(older.as_str()), "{text}");
    assert!(lines[1].starts_with(newer.as_str()), "{text}");
}

#[test]
fn sessions_without_list_is_a_usage_error() {
    let home = tempfile::tempdir().expect("home");

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .arg("sessions")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki sessions");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("sessions list"),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}
