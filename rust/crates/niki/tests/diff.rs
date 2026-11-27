//! `niki -n /diff` prints the working tree's diff without a config or a model.

use std::process::Command;

fn repo() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    let status = Command::new("git")
        .arg("-C")
        .arg(temp.path())
        .args(["init", "-q"])
        .status()
        .expect("git runs");
    assert!(status.success());
    temp
}

#[test]
fn headless_diff_prints_the_working_tree() {
    let temp = repo();
    std::fs::write(temp.path().join("new.txt"), "hello\n").expect("write");

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "/diff", "--root"])
        .arg(temp.path())
        .output()
        .expect("run niki");

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("untracked"), "{stdout}");
    assert!(stdout.contains("new.txt"), "{stdout}");
    assert!(stdout.contains("+hello"), "{stdout}");
    assert!(stdout.contains("1 file, +1 −0"), "{stdout}");
}

#[test]
fn headless_diff_prints_a_clean_tree_as_no_changes() {
    let temp = repo();
    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "/diff", "--root"])
        .arg(temp.path())
        .output()
        .expect("run niki");

    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "no changes");
}

#[test]
fn headless_diff_reports_a_non_repository_on_one_line() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "/diff", "--root"])
        .arg(temp.path())
        .output()
        .expect("run niki");

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a git repository"), "{stderr}");
    assert_eq!(stderr.trim_end().lines().count(), 1, "{stderr}");
}
