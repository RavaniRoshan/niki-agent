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
