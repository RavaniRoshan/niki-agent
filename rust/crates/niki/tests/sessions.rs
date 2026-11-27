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

fn run_sessions(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_niki"))
        .arg("sessions")
        .args(args)
        .env("NIKI_HOME", home)
        .output()
        .expect("run niki sessions")
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
    assert!(!row["archived"].as_bool().expect("archived flag"));
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

#[test]
fn fork_copies_the_messages_and_prints_the_new_id() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let source = seed(home.path(), workspace.path(), Some("port the parser"), 3);

    let output = run_sessions(home.path(), &["fork", source.as_str()]);
    let text = stdout(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{text}{stderr}");
    let new_id = text.trim().to_owned();
    assert_ne!(new_id, source.as_str(), "{text}");

    let store = SessionStore::open(home.path().join("sessions.db")).expect("store");
    let original = store.load(&source).expect("load the source");
    let forked = store.find(&new_id).expect("find").expect("the fork exists");
    let copied = store.load(&forked.id).expect("load the fork");

    assert_eq!(original.len(), 3, "the source keeps every message");
    assert_eq!(copied.len(), original.len());
    assert_eq!(
        copied
            .iter()
            .map(|message| (message.role.as_str(), message.content.as_str()))
            .collect::<Vec<_>>(),
        original
            .iter()
            .map(|message| (message.role.as_str(), message.content.as_str()))
            .collect::<Vec<_>>(),
        "order, roles and bodies are copied"
    );
    assert_eq!(forked.title.as_deref(), Some("port the parser (fork)"));
    assert_eq!(
        store.title(&source).expect("title").as_deref(),
        Some("port the parser")
    );
    assert_eq!(
        store.list().expect("list").len(),
        2,
        "the fork is a second session"
    );
}

#[test]
fn fork_prints_the_two_ids_as_json() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let source = seed(home.path(), workspace.path(), None, 1);

    let output = run_sessions(home.path(), &["fork", source.as_str(), "--json"]);
    let text = stdout(&output);

    assert_eq!(output.status.code(), Some(0), "{text}");
    let value: Value = serde_json::from_str(text.trim()).expect("JSON object");
    assert_eq!(value["from"], source.as_str());
    assert_ne!(value["id"], source.as_str());
    assert!(value["id"].as_str().is_some_and(|id| !id.is_empty()));
}

#[test]
fn fork_of_a_missing_id_exits_1_and_says_so() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    seed(home.path(), workspace.path(), Some("kept"), 1);

    let output = run_sessions(home.path(), &["fork", "no-such-session"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("no saved session"), "{stderr}");
    assert!(stderr.contains("no-such-session"), "{stderr}");
    assert_eq!(stdout(&output), "");
}

#[test]
fn fork_of_a_missing_store_exits_1_without_creating_one() {
    let home = tempfile::tempdir().expect("home");

    let output = run_sessions(home.path(), &["fork", "no-such-session"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no saved session"),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!home.path().join("sessions.db").exists());
}

#[test]
fn archive_hides_the_session_from_the_default_list_and_all_shows_it() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let session = seed(home.path(), workspace.path(), Some("old news"), 2);

    let output = run_sessions(home.path(), &["archive", session.as_str()]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "", "archive prints nothing on success");

    let default = stdout(&run(home.path(), &[]));
    assert_eq!(default, "no saved sessions\n", "{default}");
    assert!(!default.contains(session.as_str()));

    let all = stdout(&run(home.path(), &["--all"]));
    assert!(all.contains(session.as_str()), "{all}");
    assert!(all.contains("[archived]"), "{all}");
    assert!(all.contains("old news"), "{all}");

    let rows: Value =
        serde_json::from_str(stdout(&run(home.path(), &["--json"])).trim()).expect("array");
    assert_eq!(rows.as_array().expect("array").len(), 0);

    let rows: Value = serde_json::from_str(stdout(&run(home.path(), &["--all", "--json"])).trim())
        .expect("array");
    let rows = rows.as_array().expect("array");
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["id"], session.as_str());
    assert!(rows[0]["archived"].as_bool().expect("archived flag"));
}

#[test]
fn archive_undo_restores_the_session() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let session = seed(home.path(), workspace.path(), Some("back again"), 1);

    assert_eq!(
        run_sessions(home.path(), &["archive", session.as_str()])
            .status
            .code(),
        Some(0)
    );
    assert_eq!(stdout(&run(home.path(), &[])), "no saved sessions\n");

    let output = run_sessions(home.path(), &["archive", "--undo", session.as_str()]);
    assert_eq!(output.status.code(), Some(0));

    let text = stdout(&run(home.path(), &[]));
    assert!(text.contains(session.as_str()), "{text}");
    assert!(!text.contains("[archived]"), "{text}");
    let rows: Value =
        serde_json::from_str(stdout(&run(home.path(), &["--json"])).trim()).expect("array");
    assert!(!rows[0]["archived"].as_bool().expect("archived flag"));
}

#[test]
fn archiving_a_missing_id_exits_1_and_says_so() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    seed(home.path(), workspace.path(), Some("kept"), 0);

    let output = run_sessions(home.path(), &["archive", "no-such-session"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("no saved session"), "{stderr}");
    assert!(stderr.contains("no-such-session"), "{stderr}");
}

#[test]
fn a_verb_without_an_id_is_a_usage_error() {
    let home = tempfile::tempdir().expect("home");

    for verb in ["fork", "archive"] {
        let output = run_sessions(home.path(), &[verb]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{verb}: {stderr}");
        assert!(stderr.contains("needs a session id"), "{verb}: {stderr}");
    }
}

#[test]
fn a_flag_on_the_wrong_verb_is_a_usage_error() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let session = seed(home.path(), workspace.path(), Some("kept"), 0);

    let list = run_sessions(home.path(), &["list", "--undo"]);
    assert_eq!(list.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&list.stderr).contains("--undo"),
        "{:?}",
        String::from_utf8_lossy(&list.stderr)
    );

    let fork = run_sessions(home.path(), &["fork", session.as_str(), "--all"]);
    assert_eq!(fork.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&fork.stderr).contains("--all"),
        "{:?}",
        String::from_utf8_lossy(&fork.stderr)
    );
}

#[test]
fn help_names_every_sessions_verb() {
    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .arg("--help")
        .output()
        .expect("run niki --help");
    let text = stdout(&output);

    assert_eq!(output.status.code(), Some(0), "{text}");
    for line in ["sessions list", "sessions fork", "sessions archive"] {
        assert!(text.contains(line), "help omits {line:?}: {text}");
    }
    for flag in ["--all", "--undo", "--json"] {
        assert!(text.contains(flag), "help omits {flag}: {text}");
    }
}
