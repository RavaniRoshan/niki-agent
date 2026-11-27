//! `niki doctor` end to end: no terminal, a temp `NIKI_HOME`, and exit codes
//! that tell a usable install from a broken one.

use std::path::Path;
use std::process::{Command, Output};

/// Write a config naming a mock provider and its key variable under `home`.
fn write_config(home: &Path) {
    std::fs::write(
        home.join("config.toml"),
        "[models]\n\
         default = \"mock:mock-model\"\n\
         \n\
         [providers.mock]\n\
         base_url = \"http://127.0.0.1:1\"\n\
         api_key_env = \"NIKI_DOCTOR_KEY\"\n",
    )
    .expect("config");
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn doctor_reports_a_usable_install_without_a_terminal() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    write_config(home.path());

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .arg("doctor")
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_HOME", home.path())
        .env("NIKI_DOCTOR_KEY", "doctor-secret-value")
        .output()
        .expect("run niki doctor");

    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}");
    assert!(
        !text.contains("doctor-secret-value"),
        "the key value must never be printed: {text}"
    );

    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 10, "nine checks and a summary: {text}");
    for line in &lines[..9] {
        assert!(line.starts_with("ok   "), "{line:?} in {text}");
    }
    assert!(
        text.contains(&format!(
            "ok   config: {} exists and parses",
            home.path().join("config.toml").display()
        )),
        "{text}"
    );
    assert!(text.contains("ok   model: mock:mock-model"), "{text}");
    assert!(
        text.contains("ok   credential: mock: NIKI_DOCTOR_KEY is set"),
        "{text}"
    );
    assert!(
        lines[9].starts_with("summary: 9 check(s): 9 ok, 0 warning(s), 0 failure(s)"),
        "{:?}",
        lines[9]
    );

    assert!(
        !home.path().join("sessions.db").exists(),
        "doctor is read-only and must not create the session store"
    );
}

#[test]
fn a_missing_credential_exits_one_and_names_the_variable() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    write_config(home.path());

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .arg("doctor")
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_HOME", home.path())
        .env_remove("NIKI_DOCTOR_KEY")
        .output()
        .expect("run niki doctor");

    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{text}");
    assert!(
        text.contains("fail credential: mock: NIKI_DOCTOR_KEY is unset"),
        "{text}"
    );
    assert!(text.contains("fix the failures above"), "{text}");
}

#[test]
fn a_missing_config_warns_and_a_builtin_provider_still_installs() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");

    // The child inherits this process's environment; supply the key so the
    // credential check is green regardless of the machine.
    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .arg("doctor")
        .arg("--root")
        .arg(workspace.path())
        .arg("--model")
        .arg("anthropic:claude-test")
        .env("NIKI_HOME", home.path())
        .env("ANTHROPIC_API_KEY", "doctor-test-key")
        .output()
        .expect("run niki doctor");
    let text = stdout(&output);

    assert_eq!(output.status.code(), Some(0), "{text}");
    assert!(
        text.contains(&format!(
            "warn config: {} not found; built-in defaults apply",
            home.path().join("config.toml").display()
        )),
        "{text}"
    );
    assert!(
        text.contains("ok   credential: anthropic: ANTHROPIC_API_KEY is set"),
        "{text}"
    );
}

#[test]
fn an_unparseable_config_fails() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    std::fs::write(home.path().join("config.toml"), "not toml [[[\n").expect("config");

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .arg("doctor")
        .arg("--root")
        .arg(workspace.path())
        .arg("--model")
        .arg("anthropic:claude-test")
        .env("NIKI_HOME", home.path())
        .env("ANTHROPIC_API_KEY", "doctor-test-key")
        .output()
        .expect("run niki doctor");

    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{text}");
    assert!(text.contains("fail config:"), "{text}");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.len(),
        10,
        "one line per check plus the summary: {text}"
    );
    assert!(
        lines[..9]
            .iter()
            .any(|line| line.starts_with("fail config:")),
        "{text}"
    );
}
