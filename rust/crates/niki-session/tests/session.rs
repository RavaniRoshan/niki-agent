//! Session persistence: create, append, load, checkpoints, and reopening.

use std::path::{Path, PathBuf};

use niki_session::{SessionError, SessionStore};
use tempfile::TempDir;

fn scratch(temp: &TempDir) -> PathBuf {
    temp.path().join("sessions.db")
}

fn open(path: &Path) -> SessionStore {
    SessionStore::open(path).unwrap()
}

#[test]
fn create_and_list_a_session() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let cwd = temp.path().join("project");

    let session = store.create(&cwd).unwrap();
    let listed = store.list().unwrap();

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, session);
    assert_eq!(listed[0].cwd, cwd.to_string_lossy());
    assert_eq!(listed[0].messages, 0);
    assert_eq!(listed[0].title, None);
    assert!(listed[0].created_at > 0);
}

#[test]
fn append_and_load_keep_the_write_order() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let session = store.create(temp.path()).unwrap();

    let first = store.append(&session, "user", "hello").unwrap();
    let second = store.append(&session, "assistant", "hi").unwrap();
    let third = store.append(&session, "tool", "read 3 lines").unwrap();

    assert!(first < second && second < third);
    let messages = store.load(&session).unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(
        messages
            .iter()
            .map(|message| (message.id, message.role.as_str(), message.content.as_str()))
            .collect::<Vec<_>>(),
        [
            (first, "user", "hello"),
            (second, "assistant", "hi"),
            (third, "tool", "read 3 lines"),
        ]
    );
    assert!(messages.iter().all(|message| message.session_id == session));
    assert!(messages.iter().all(|message| message.created_at > 0));
    assert_eq!(store.list().unwrap()[0].messages, 3);
}

#[test]
fn resume_replays_the_transcript_up_to_the_checkpoint() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let session = store.create(temp.path()).unwrap();
    store.append(&session, "user", "plan").unwrap();
    store.append(&session, "assistant", "planned").unwrap();

    let watermark = store.checkpoint(&session, "before-refactor").unwrap();
    assert_eq!(watermark, 2);

    store.append(&session, "user", "go ahead").unwrap();
    assert_eq!(store.load(&session).unwrap().len(), 3);

    let resumed = store.resume(&session, "before-refactor").unwrap();
    assert_eq!(resumed.len(), 2);
    assert_eq!(resumed[1].content, "planned");
}

#[test]
fn a_reused_label_moves_the_checkpoint_forward() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let session = store.create(temp.path()).unwrap();
    store.append(&session, "user", "one").unwrap();

    assert_eq!(store.checkpoint(&session, "step").unwrap(), 1);
    store.append(&session, "assistant", "two").unwrap();
    assert_eq!(store.checkpoint(&session, "step").unwrap(), 2);
    assert_eq!(store.resume(&session, "step").unwrap().len(), 2);
}

#[test]
fn a_checkpoint_before_any_message_resumes_empty() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let session = store.create(temp.path()).unwrap();

    assert_eq!(store.checkpoint(&session, "start").unwrap(), 0);
    assert!(store.resume(&session, "start").unwrap().is_empty());
}

#[test]
fn reopening_the_file_sees_the_earlier_session() {
    let temp = TempDir::new().unwrap();
    let path = scratch(&temp);

    let (session, message_id) = {
        let store = open(&path);
        let session = store.create(temp.path()).unwrap();
        let message_id = store.append(&session, "user", "survives").unwrap();
        (session, message_id)
    };

    let reopened = open(&path);
    let listed = reopened.list().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, session);

    let messages = reopened.load(&session).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].id, message_id);
    assert_eq!(messages[0].content, "survives");
}

#[test]
fn list_is_newest_first() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let older = store.create(temp.path()).unwrap();
    let newer = store.create(temp.path()).unwrap();

    let listed = store.list().unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, newer);
    assert_eq!(listed[1].id, older);
}

#[test]
fn list_for_keeps_only_the_named_workspace() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    let first_session = store.create(&first).unwrap();
    let second_session = store.create(&second).unwrap();

    let listed = store.list_for(&first).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, first_session);
    assert_eq!(listed[0].cwd, first.to_string_lossy());
    assert_ne!(first_session, second_session);

    assert!(
        store
            .list_for(temp.path().join("absent"))
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.list().unwrap().len(), 2, "the plain list sees both");
}

#[test]
fn archived_sessions_are_left_out_of_the_active_lists_but_found_by_id() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let cwd = temp.path().join("project");
    let older = store.create(&cwd).unwrap();
    let newer = store.create(&cwd).unwrap();
    store.append(&newer, "user", "old news").unwrap();
    store.set_archived(&newer, true).unwrap();

    let active = store.list_active().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, older);
    assert_eq!(store.list_for_active(&cwd).unwrap().len(), 1);
    assert_eq!(store.list_for_active(&cwd).unwrap()[0].id, older);
    assert_eq!(
        store.latest_active(&cwd).unwrap().unwrap().id,
        older,
        "the newest live session stands in for the workspace"
    );

    let all = store.list().unwrap();
    assert_eq!(
        all.len(),
        2,
        "the archived session is still listed by --all"
    );
    assert!(
        all.iter()
            .any(|summary| summary.id == newer && summary.archived)
    );
    assert_eq!(store.list_for(&cwd).unwrap().len(), 2);

    let found = store.find(newer.as_str()).unwrap().expect("found by id");
    assert!(found.archived);
    assert_eq!(
        store.load(&newer).unwrap().len(),
        1,
        "its transcript is intact"
    );

    store.set_archived(&newer, false).unwrap();
    assert_eq!(store.latest_active(&cwd).unwrap().unwrap().id, newer);
}

#[test]
fn a_workspace_with_only_archived_sessions_has_no_latest_active() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let cwd = temp.path().join("project");
    let only = store.create(&cwd).unwrap();
    let elsewhere = store.create(temp.path().join("elsewhere")).unwrap();

    store.set_archived(&only, true).unwrap();

    assert!(store.latest_active(&cwd).unwrap().is_none());
    assert!(store.list_for_active(&cwd).unwrap().is_empty());
    assert_eq!(
        store.list_active().unwrap()[0].id,
        elsewhere,
        "another workspace's live session is not drawn in"
    );
    assert_eq!(store.list().unwrap().len(), 2);
}

#[test]
fn updated_at_is_the_latest_stored_write() {
    let temp = TempDir::new().unwrap();
    let path = scratch(&temp);
    let store = open(&path);
    let session = store.create(temp.path()).unwrap();
    store.append(&session, "user", "later").unwrap();

    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute("UPDATE sessions SET created_at = 100", [])
            .unwrap();
        connection
            .execute("UPDATE messages SET created_at = 400", [])
            .unwrap();
    }

    let summary = store
        .list()
        .unwrap()
        .into_iter()
        .find(|summary| summary.id == session)
        .unwrap();
    assert_eq!(summary.created_at, 100);
    assert_eq!(summary.updated_at, 400, "the message is the later write");
    assert_eq!(
        store.find(session.as_str()).unwrap().unwrap().updated_at,
        400
    );
    assert_eq!(
        store
            .latest_active(temp.path())
            .unwrap()
            .unwrap()
            .updated_at,
        400
    );

    let empty = store.create(temp.path().join("empty")).unwrap();
    let summary = store
        .list()
        .unwrap()
        .into_iter()
        .find(|summary| summary.id == empty)
        .unwrap();
    assert_eq!(
        summary.updated_at, summary.created_at,
        "a session without messages was last written when it was created"
    );
}

#[test]
fn a_session_from_another_file_is_unknown() {
    let temp = TempDir::new().unwrap();
    let first = open(&temp.path().join("first.db"));
    let second = open(&temp.path().join("second.db"));
    let foreign = first.create(temp.path()).unwrap();

    assert!(matches!(
        second.load(&foreign),
        Err(SessionError::UnknownSession { .. })
    ));
    assert!(matches!(
        second.append(&foreign, "user", "no"),
        Err(SessionError::UnknownSession { .. })
    ));
    assert!(matches!(
        second.checkpoint(&foreign, "step"),
        Err(SessionError::UnknownSession { .. })
    ));
}

#[test]
fn an_unknown_checkpoint_label_is_reported() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let session = store.create(temp.path()).unwrap();

    assert!(matches!(
        store.resume(&session, "never-taken"),
        Err(SessionError::UnknownCheckpoint { .. })
    ));
}

#[test]
fn a_summary_previews_the_first_user_prompt() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let session = store.create(temp.path()).unwrap();

    assert_eq!(
        store.list().unwrap()[0].first_prompt,
        None,
        "a session with no messages has no prompt to preview"
    );

    store.append(&session, "assistant", "Welcome.").unwrap();
    assert_eq!(
        store.list().unwrap()[0].first_prompt,
        None,
        "an assistant message is not a prompt"
    );

    store.append(&session, "user", "plan the port").unwrap();
    store.append(&session, "user", "a later prompt").unwrap();
    let listed = store.list().unwrap();
    assert_eq!(listed[0].first_prompt.as_deref(), Some("plan the port"));
    assert_eq!(listed[0].messages, 3);

    assert_eq!(
        store.find(session.as_str()).unwrap().unwrap().first_prompt,
        Some("plan the port".to_owned())
    );
    assert_eq!(
        store
            .latest_active(temp.path())
            .unwrap()
            .unwrap()
            .first_prompt,
        Some("plan the port".to_owned())
    );
}

#[test]
fn a_title_round_trips_through_list_find_and_latest_active() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let cwd = temp.path().join("project");
    let session = store.create(&cwd).unwrap();

    assert_eq!(store.title(&session).unwrap(), None);
    assert_eq!(
        store.set_title(&session, Some("  deploy notes  ")).unwrap(),
        Some("deploy notes".to_owned()),
        "a title is trimmed"
    );
    assert_eq!(
        store.title(&session).unwrap().as_deref(),
        Some("deploy notes")
    );
    assert_eq!(
        store.list().unwrap()[0].title.as_deref(),
        Some("deploy notes")
    );
    assert_eq!(
        store
            .find(session.as_str())
            .unwrap()
            .unwrap()
            .title
            .as_deref(),
        Some("deploy notes")
    );
    assert_eq!(
        store.latest_active(&cwd).unwrap().unwrap().title.as_deref(),
        Some("deploy notes")
    );
}

#[test]
fn an_empty_title_clears_it() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let session = store.create(temp.path()).unwrap();

    store.set_title(&session, Some("named")).unwrap();
    assert_eq!(store.set_title(&session, Some("   ")).unwrap(), None);
    assert_eq!(store.list().unwrap()[0].title, None);
    assert_eq!(store.title(&session).unwrap(), None);

    store.set_title(&session, Some("named again")).unwrap();
    assert_eq!(store.set_title(&session, None).unwrap(), None);
    assert_eq!(store.title(&session).unwrap(), None);
}

#[test]
fn a_fresh_database_has_the_title_column() {
    let temp = TempDir::new().unwrap();
    let store = open(&scratch(&temp));
    let session = store.create(temp.path()).unwrap();

    assert_eq!(store.title(&session).unwrap(), None);
    store.set_title(&session, Some("fresh")).unwrap();
    assert_eq!(store.title(&session).unwrap().as_deref(), Some("fresh"));
}

#[test]
fn a_database_created_before_the_title_column_gains_it() {
    let temp = TempDir::new().unwrap();
    let path = scratch(&temp);
    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE sessions (
                     id TEXT PRIMARY KEY,
                     created_at INTEGER NOT NULL,
                     cwd TEXT NOT NULL
                 );
                 CREATE TABLE messages (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     session_id TEXT NOT NULL REFERENCES sessions(id),
                     role TEXT NOT NULL,
                     content TEXT NOT NULL,
                     created_at INTEGER NOT NULL
                 );
                 CREATE INDEX messages_session ON messages(session_id, id);
                 CREATE TABLE checkpoints (
                     session_id TEXT NOT NULL REFERENCES sessions(id),
                     label TEXT NOT NULL,
                     message_id INTEGER NOT NULL,
                     created_at INTEGER NOT NULL,
                     PRIMARY KEY (session_id, label)
                 );
                 INSERT INTO sessions (id, created_at, cwd)
                 VALUES ('older-session', 1, '/work');",
            )
            .unwrap();
    }

    let store = open(&path);
    let listed = store.list().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, None, "the migrated column starts empty");

    let session = listed[0].id.clone();
    store.set_title(&session, Some("migrated")).unwrap();
    assert_eq!(
        store
            .find(session.as_str())
            .unwrap()
            .unwrap()
            .title
            .as_deref(),
        Some("migrated")
    );
    store.append(&session, "user", "still writes").unwrap();
    assert_eq!(store.load(&session).unwrap().len(), 1);
}
