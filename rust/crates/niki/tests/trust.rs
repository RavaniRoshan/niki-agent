//! `niki trust` end to end: a temp `NIKI_HOME`, a workspace on disk, and the
//! store the gate reads.

use std::path::Path;
use std::process::{Command, Output};

fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(args)
        .env("NIKI_HOME", home)
        .env("NIKI_TRUST_TEST_KEY", "test-key")
        .output()
        .expect("run niki")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A config naming one unreachable provider, so a run with a built-in command
/// never needs a socket but still resolves a model.
fn write_config(home: &Path) {
    std::fs::write(
        home.join("config.toml"),
        "[models]\ndefault = \"mock:mock-model\"\n\n\
         [providers.mock]\nbase_url = \"http://127.0.0.1:1\"\n\
         api_key_env = \"NIKI_TRUST_TEST_KEY\"\n",
    )
    .expect("config");
}

#[test]
fn trusting_a_workspace_lists_it_and_untrusting_removes_it() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path().canonicalize().expect("canonical");

    let listing = run(home.path(), &["trust", "--list"]);
    assert_eq!(listing.status.code(), Some(0), "{}", stderr(&listing));
    assert_eq!(stdout(&listing), "no trusted workspaces\n");

    let trusted = run(
        home.path(),
        &["trust", "--root", &root.display().to_string()],
    );
    assert_eq!(trusted.status.code(), Some(0), "{}", stderr(&trusted));
    assert_eq!(stdout(&trusted), format!("trusted: {}\n", root.display()));

    let listing = run(home.path(), &["trust", "--list"]);
    assert_eq!(stdout(&listing), format!("{}\n", root.display()));

    let removed = run(
        home.path(),
        &["untrust", "--root", &root.display().to_string()],
    );
    assert_eq!(removed.status.code(), Some(0), "{}", stderr(&removed));
    assert_eq!(
        stdout(&removed),
        format!("no longer trusted: {}\n", root.display())
    );

    let listing = run(home.path(), &["trust", "--list"]);
    assert_eq!(stdout(&listing), "no trusted workspaces\n");
}

#[test]
fn untrusting_a_workspace_that_was_never_trusted_says_so() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path().canonicalize().expect("canonical");

    let output = run(
        home.path(),
        &["untrust", "--root", &root.display().to_string()],
    );

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!("not trusted: {}\n", root.display())
    );
    assert!(
        !home.path().join("trusted_workspaces.toml").exists(),
        "a no-op removal must not create the store"
    );
}

#[test]
fn a_trailing_slash_and_a_relative_path_name_the_same_workspace() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path().canonicalize().expect("canonical");
    let spelled = format!("{}/", root.display());

    let trusted = run(home.path(), &["trust", "--root", &spelled]);
    assert_eq!(trusted.status.code(), Some(0), "{}", stderr(&trusted));

    let listing = run(home.path(), &["trust", "--list"]);
    assert_eq!(stdout(&listing), format!("{}\n", root.display()));
}

#[test]
fn a_missing_path_is_an_error() {
    let home = tempfile::tempdir().expect("home");
    let missing = home.path().join("not-there");

    let output = run(
        home.path(),
        &["trust", "--root", &missing.display().to_string()],
    );

    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    assert!(
        stderr(&output).contains("resolve"),
        "stderr should name the path: {}",
        stderr(&output)
    );
}

#[test]
fn trust_does_not_combine_with_a_headless_run() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");

    let output = run(
        home.path(),
        &[
            "trust",
            "-n",
            "hi",
            "--root",
            &workspace.path().display().to_string(),
        ],
    );

    assert_eq!(output.status.code(), Some(2), "{}", stdout(&output));
    assert!(
        stderr(&output).contains("does not combine"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn an_untrusted_workspace_announces_that_project_files_are_not_loaded() {
    let home = tempfile::tempdir().expect("home");
    write_config(home.path());
    let workspace = tempfile::tempdir().expect("workspace");

    let output = run(
        home.path(),
        &[
            "-n",
            "/version",
            "--root",
            &workspace.path().display().to_string(),
        ],
    );

    let errors = stderr(&output);
    assert!(
        errors.contains("is not trusted; run niki trust"),
        "the hint should be printed before the turn runs: {errors}"
    );
    assert!(
        errors.contains("provider mock failed"),
        "the run still reaches the provider: {errors}"
    );
}

#[test]
fn a_trusted_workspace_does_not_announce_the_notice() {
    let home = tempfile::tempdir().expect("home");
    write_config(home.path());
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path().canonicalize().expect("canonical");
    let trusted = run(
        home.path(),
        &["trust", "--root", &root.display().to_string()],
    );
    assert_eq!(trusted.status.code(), Some(0), "{}", stderr(&trusted));

    let output = run(
        home.path(),
        &["-n", "/version", "--root", &root.display().to_string()],
    );

    let errors = stderr(&output);
    assert!(
        !errors.contains("run niki trust"),
        "a trusted workspace prints no trust hint: {errors}"
    );
}
