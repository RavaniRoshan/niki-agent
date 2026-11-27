//! Markdown export: the shape, the truncation, and the no-overwrite rule.

use std::path::PathBuf;

use niki_session::SessionStore;
use niki_session::export::{self, ExportError};
use tempfile::TempDir;

fn workspace(temp: &TempDir) -> PathBuf {
    let root = temp.path().join("workspace");
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn store(temp: &TempDir) -> SessionStore {
    SessionStore::open(temp.path().join("sessions.db")).unwrap()
}

#[test]
fn export_writes_the_documented_markdown() {
    let temp = TempDir::new().unwrap();
    let root = workspace(&temp);
    let store = store(&temp);
    let session = store.create(&root).unwrap();
    store.append(&session, "user", "fix the bug").unwrap();
    store
        .append(&session, "tool_call", "read_file\n{\"path\":\"a.rs\"}")
        .unwrap();
    store
        .append(&session, "tool_result", "read_file\nfn main() {}")
        .unwrap();
    store.append(&session, "assistant", "Done.").unwrap();

    let written = export::write(&store, &session, &root, None).unwrap();

    assert!(written.is_absolute(), "{}", written.display());
    assert_eq!(
        written,
        root.canonicalize()
            .unwrap()
            .join(format!("niki-{}.md", session.as_str()))
    );
    assert_eq!(
        std::fs::read_to_string(&written).unwrap(),
        "# fix the bug\n\n## You\n\nfix the bug\n\n```read_file\nfn main() {}\n```\n\n## Niki\n\nDone.\n"
    );
}

#[test]
fn a_title_names_the_export_and_an_empty_session_still_writes() {
    let temp = TempDir::new().unwrap();
    let root = workspace(&temp);
    let store = store(&temp);
    let session = store.create(&root).unwrap();
    store.append(&session, "user", "first prompt").unwrap();
    store.append(&session, "assistant", "answer").unwrap();
    store.set_title(&session, Some("Bug fix")).unwrap();

    let written = export::write(&store, &session, &root, None).unwrap();
    let text = std::fs::read_to_string(&written).unwrap();
    assert!(text.starts_with("# Bug fix\n\n"), "{text}");

    let empty = store.create(&root).unwrap();
    let written = export::write(&store, &empty, &root, None).unwrap();
    assert_eq!(
        std::fs::read_to_string(&written).unwrap(),
        "# Untitled session\n"
    );
}

#[test]
fn a_long_tool_result_is_truncated_with_the_marker() {
    let temp = TempDir::new().unwrap();
    let root = workspace(&temp);
    let store = store(&temp);
    let session = store.create(&root).unwrap();
    store.append(&session, "user", "read the log").unwrap();
    let long = "x".repeat(export::RESULT_LIMIT + 500);
    store
        .append(&session, "tool_result", &format!("read_file\n{long}"))
        .unwrap();

    let written = export::write(&store, &session, &root, None).unwrap();
    let text = std::fs::read_to_string(&written).unwrap();

    assert!(
        text.contains(&format!(
            "{}\n[truncated]\n```",
            "x".repeat(export::RESULT_LIMIT)
        )),
        "{text}"
    );
    assert!(
        !text.contains(&"x".repeat(export::RESULT_LIMIT + 1)),
        "the result stopped at the limit"
    );
    assert_eq!(text.matches(export::TRUNCATED).count(), 1);
}

#[test]
fn exporting_twice_keeps_both_files() {
    let temp = TempDir::new().unwrap();
    let root = workspace(&temp);
    let store = store(&temp);
    let session = store.create(&root).unwrap();
    store.append(&session, "user", "hello").unwrap();

    let first = export::write(&store, &session, &root, None).unwrap();
    let second = export::write(&store, &session, &root, None).unwrap();

    assert_eq!(
        first.file_name().unwrap().to_string_lossy(),
        format!("niki-{}.md", session.as_str())
    );
    assert_eq!(
        second.file_name().unwrap().to_string_lossy(),
        format!("niki-{}-2.md", session.as_str())
    );
    assert!(first.exists() && second.exists());
    assert_eq!(
        std::fs::read_to_string(&first).unwrap(),
        std::fs::read_to_string(&second).unwrap()
    );
}

#[test]
fn an_explicit_path_resolves_inside_the_workspace() {
    let temp = TempDir::new().unwrap();
    let root = workspace(&temp);
    std::fs::create_dir_all(root.join("notes")).unwrap();
    let store = store(&temp);
    let session = store.create(&root).unwrap();
    store.append(&session, "user", "hello").unwrap();

    let written = export::write(&store, &session, &root, Some("notes/today.md")).unwrap();
    assert_eq!(written, root.canonicalize().unwrap().join("notes/today.md"));
    assert!(written.exists());

    let absolute = export::write(
        &store,
        &session,
        &root,
        Some(&root.join("absolute.md").display().to_string()),
    )
    .unwrap();
    assert_eq!(absolute, root.canonicalize().unwrap().join("absolute.md"));
}

#[test]
fn a_path_outside_the_workspace_is_refused() {
    let temp = TempDir::new().unwrap();
    let root = workspace(&temp);
    let store = store(&temp);
    let session = store.create(&root).unwrap();
    store.append(&session, "user", "hello").unwrap();

    let error = export::write(&store, &session, &root, Some("../outside.md")).unwrap_err();
    assert!(matches!(error, ExportError::OutsideRoot { .. }), "{error}");
    assert!(
        error.to_string().contains("outside the workspace"),
        "{error}"
    );
    assert!(!temp.path().join("outside.md").exists());

    let absolute = temp.path().join("outside-absolute.md");
    let error = export::write(
        &store,
        &session,
        &root,
        Some(&absolute.display().to_string()),
    )
    .unwrap_err();
    assert!(matches!(error, ExportError::OutsideRoot { .. }), "{error}");
    assert!(!absolute.exists());
}
