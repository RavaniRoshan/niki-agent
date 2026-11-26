//! Behaviour of the hook runner against real child processes. Every hook is a
//! `/bin/sh` command in a temporary workspace, so nothing here needs a network.

use std::path::Path;
use std::time::{Duration, Instant};

use niki_hooks::{
    Event, HookRunner, Hooks, Outcome, PostToolUse, PreToolUse, ToolCall, UserPrompt,
};
use serde_json::{Value, json};

struct Workspace {
    dir: tempfile::TempDir,
}

impl Workspace {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.dir.path().join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, text).expect("write");
    }

    fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(relative)).expect("read")
    }

    fn exists(&self, relative: &str) -> bool {
        self.dir.path().join(relative).exists()
    }

    fn load(&self, document: &str) -> (Hooks, Vec<String>) {
        self.write(".niki/hooks.json", document);
        let loaded = Hooks::load_from(
            &self.dir.path().join(".niki/hooks.json"),
            &self.dir.path().join("home/hooks.json"),
        );
        (loaded.hooks, loaded.warnings)
    }

    fn pre(&self, hooks: &Hooks, name: &str, input: Value) -> Outcome {
        hooks.pre_tool_use(&PreToolUse {
            tool: ToolCall {
                name,
                input: &input,
                id: "call-1",
            },
            cwd: self.path(),
            session_id: Some("session-1"),
        })
    }
}

fn handler(command: &str) -> String {
    json!({"type": "command", "command": command}).to_string()
}

#[test]
fn matchers_select_the_tool_and_every_matching_group_runs() {
    let workspace = Workspace::new();
    let (hooks, warnings) = workspace.load(&format!(
        r#"{{"hooks": {{
            "PreToolUse": [
                {{"matcher": "Bash", "hooks": [{}]}},
                {{"matcher": "Write|Edit", "hooks": [{}]}},
                {{"hooks": [{}]}}
            ]
        }}}}"#,
        handler("printf bash >> log"),
        handler("printf write >> log"),
        handler("printf any >> log"),
    ));
    assert!(warnings.is_empty(), "{warnings:?}");

    workspace.pre(&hooks, "shell", json!({"command": "ls"}));
    assert_eq!(workspace.read("log"), "bashany");

    workspace.pre(
        &hooks,
        "write_file",
        json!({"path": "a.txt", "content": "hi"}),
    );
    assert_eq!(workspace.read("log"), "bashanywriteany");

    workspace.pre(&hooks, "grep", json!({"pattern": "x"}));
    assert_eq!(workspace.read("log"), "bashanywriteanyany");
}

#[test]
fn a_non_matching_tool_runs_no_handler() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(&format!(
        r#"{{"hooks": {{"PreToolUse": [{{"matcher": "Write", "hooks": [{}]}}]}}}}"#,
        handler("printf ran > log")
    ));

    let outcome = workspace.pre(&hooks, "read_file", json!({"path": "a.txt"}));

    assert_eq!(outcome, Outcome::default());
    assert!(!workspace.exists("log"));
}

#[test]
fn the_payload_on_stdin_carries_the_event_the_tool_and_the_session() {
    let workspace = Workspace::new();
    let (hooks, warnings) = workspace.load(&format!(
        r#"{{"hooks": {{"PreToolUse": [{{"matcher": "Write", "hooks": [{}]}}]}}}}"#,
        handler("cat > payload.json")
    ));
    assert!(warnings.is_empty(), "{warnings:?}");

    let outcome = workspace.pre(
        &hooks,
        "write_file",
        json!({"path": "note.txt", "content": "hello"}),
    );
    assert!(!outcome.is_blocked(), "{outcome:?}");

    let payload: Value =
        serde_json::from_str(&workspace.read("payload.json")).expect("payload JSON");
    assert_eq!(payload["hook_event_name"], "PreToolUse");
    assert_eq!(payload["tool_name"], "Write");
    assert_eq!(
        payload["tool_input"],
        json!({"file_path": "note.txt", "content": "hello"})
    );
    assert_eq!(payload["tool_use_id"], "call-1");
    assert_eq!(payload["session_id"], "session-1");
    assert_eq!(payload["cwd"], workspace.path().display().to_string());
}

#[test]
fn a_clean_exit_passes_the_tool_through() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(&format!(
        r#"{{"hooks": {{"PreToolUse": [{{"hooks": [{}]}}]}}}}"#,
        handler("true")
    ));

    let outcome = workspace.pre(&hooks, "shell", json!({"command": "ls"}));

    assert_eq!(outcome, Outcome::default());
}

#[test]
fn exit_code_two_blocks_with_stderr_as_the_reason() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(&format!(
        r#"{{"hooks": {{"PreToolUse": [{{"matcher": "Write", "hooks": [{}]}}]}}}}"#,
        handler("printf 'refusing the write' >&2; exit 2")
    ));

    let outcome = workspace.pre(&hooks, "write_file", json!({"path": "a.txt"}));

    assert_eq!(
        outcome.blocked.as_deref(),
        Some("Write blocked by hook: refusing the write")
    );
    assert!(outcome.warnings.is_empty(), "{:?}", outcome.warnings);
}

#[test]
fn exit_code_two_without_stderr_uses_the_client_wording() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(&format!(
        r#"{{"hooks": {{"PreToolUse": [{{"hooks": [{}]}}]}}}}"#,
        handler("exit 2")
    ));

    let outcome = workspace.pre(&hooks, "shell", json!({"command": "ls"}));

    assert_eq!(
        outcome.blocked.as_deref(),
        Some("Bash blocked by hook: Hook blocked the operation")
    );
}

#[test]
fn a_json_block_decision_blocks_the_tool() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(&format!(
        r#"{{"hooks": {{"PreToolUse": [{{"matcher": "Bash", "hooks": [{}]}}]}}}}"#,
        handler(r#"printf '{"decision":"block","reason":"policy says no"}'"#)
    ));

    let outcome = workspace.pre(&hooks, "shell", json!({"command": "rm -rf /"}));

    assert_eq!(
        outcome.blocked.as_deref(),
        Some("Bash blocked by hook: policy says no")
    );
}

#[test]
fn a_permission_deny_blocks_the_tool() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(&format!(
        r#"{{"hooks": {{"PreToolUse": [{{"matcher": "Write", "hooks": [{}]}}]}}}}"#,
        handler(
            r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"read only"}}'"#
        )
    ));

    let outcome = workspace.pre(&hooks, "write_file", json!({"path": "a.txt"}));

    assert_eq!(
        outcome.blocked.as_deref(),
        Some("Write blocked by hook: read only")
    );
}

#[test]
fn additional_context_is_carried_but_does_not_block() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(&format!(
        r#"{{"hooks": {{"PreToolUse": [{{"hooks": [{}]}}]}}}}"#,
        handler(
            r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","additionalContext":"be careful"}}'"#
        )
    ));

    let outcome = workspace.pre(&hooks, "shell", json!({"command": "ls"}));

    assert!(!outcome.is_blocked());
    assert_eq!(outcome.context, vec!["be careful"]);
}

#[test]
fn a_post_tool_block_is_feedback_not_a_refusal() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(&format!(
        r#"{{"hooks": {{"PostToolUse": [{{"matcher": "Write", "hooks": [{}]}}]}}}}"#,
        handler("printf 'run the formatter' >&2; exit 2")
    ));

    let response = Value::String("wrote a.txt".to_owned());
    let outcome = hooks.post_tool_use(&PostToolUse {
        tool: ToolCall {
            name: "write_file",
            input: &json!({"path": "a.txt", "content": "hi"}),
            id: "call-2",
        },
        cwd: workspace.path(),
        session_id: None,
        response: &response,
        duration_ms: 12,
    });

    assert!(!outcome.is_blocked());
    assert_eq!(outcome.context, vec!["run the formatter"]);
}

#[test]
fn post_tool_payload_carries_the_response_and_duration() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(&format!(
        r#"{{"hooks": {{"PostToolUse": [{{"hooks": [{}]}}]}}}}"#,
        handler("cat > post.json")
    ));

    let response = Value::String("12 bytes".to_owned());
    hooks.post_tool_use(&PostToolUse {
        tool: ToolCall {
            name: "write_file",
            input: &json!({"path": "a.txt", "content": "hi"}),
            id: "call-2",
        },
        cwd: workspace.path(),
        session_id: None,
        response: &response,
        duration_ms: 12,
    });

    let payload: Value = serde_json::from_str(&workspace.read("post.json")).expect("JSON");
    assert_eq!(payload["hook_event_name"], "PostToolUse");
    assert_eq!(payload["tool_response"], "12 bytes");
    assert_eq!(payload["duration_ms"], 12);
    assert!(payload.get("session_id").is_none());
}

#[test]
fn a_timeout_kills_the_child_and_does_not_hang() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(
        r#"{"hooks": {"PreToolUse": [{"hooks": [{"type":"command","command":"sleep 5","timeout":0.2}]}]}}"#,
    );

    let started = Instant::now();
    let outcome = workspace.pre(&hooks, "shell", json!({"command": "ls"}));
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(3),
        "the runner waited {elapsed:?}"
    );
    assert!(!outcome.is_blocked(), "{outcome:?}");
    assert_eq!(outcome.warnings.len(), 1, "{:?}", outcome.warnings);
    assert!(
        outcome.warnings[0].contains("timeout"),
        "{}",
        outcome.warnings[0]
    );
}

#[test]
fn a_nonzero_exit_other_than_two_is_a_warning() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(&format!(
        r#"{{"hooks": {{"PreToolUse": [{{"hooks": [{}]}}]}}}}"#,
        handler("printf 'boom' >&2; exit 3")
    ));

    let outcome = workspace.pre(&hooks, "shell", json!({"command": "ls"}));

    assert!(!outcome.is_blocked());
    assert_eq!(outcome.warnings.len(), 1, "{:?}", outcome.warnings);
    assert!(
        outcome.warnings[0].contains("exited with status 3"),
        "{}",
        outcome.warnings[0]
    );
}

#[test]
fn a_malformed_hooks_file_is_a_warning_and_the_session_starts_empty() {
    let workspace = Workspace::new();
    let (hooks, warnings) = workspace.load("{ this is not json");

    assert!(!hooks.has_handlers(Event::PreToolUse));
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("not valid JSON"), "{}", warnings[0]);
}

#[test]
fn an_unsupported_event_is_ignored_with_a_warning() {
    let workspace = Workspace::new();
    let (hooks, warnings) = workspace.load(&format!(
        r#"{{"hooks": {{"SessionStart": [{{"hooks": [{}]}}]}}}}"#,
        handler("true")
    ));

    assert!(!hooks.has_handlers(Event::PreToolUse));
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("SessionStart"), "{}", warnings[0]);
}

#[test]
fn the_project_file_wins_per_event_and_the_user_file_fills_the_rest() {
    let workspace = Workspace::new();
    workspace.write(
        ".niki/hooks.json",
        &format!(
            r#"{{"hooks": {{"PreToolUse": [{{"hooks": [{}]}}]}}}}"#,
            handler("printf project >> log")
        ),
    );
    workspace.write(
        "home/hooks.json",
        &format!(
            r#"{{"hooks": {{
                "PreToolUse": [{{"hooks": [{}]}}],
                "PostToolUse": [{{"matcher": "Write", "hooks": [{}]}}]
            }}}}"#,
            handler("printf user >> log"),
            handler("printf post >> log")
        ),
    );
    let loaded = Hooks::load_from(
        &workspace.path().join(".niki/hooks.json"),
        &workspace.path().join("home/hooks.json"),
    );
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);

    workspace.pre(&loaded.hooks, "shell", json!({"command": "ls"}));
    assert_eq!(workspace.read("log"), "project");

    let response = Value::String("ok".to_owned());
    loaded.hooks.post_tool_use(&PostToolUse {
        tool: ToolCall {
            name: "write_file",
            input: &json!({"path": "a.txt", "content": "hi"}),
            id: "call-3",
        },
        cwd: workspace.path(),
        session_id: None,
        response: &response,
        duration_ms: 1,
    });
    assert_eq!(workspace.read("log"), "projectpost");
}

#[test]
fn load_reads_the_project_and_user_locations() {
    let workspace = Workspace::new();
    workspace.write(
        ".niki/hooks.json",
        &format!(
            r#"{{"hooks": {{"UserPromptSubmit": [{{"hooks": [{}]}}]}}}}"#,
            handler("printf prompted >> log")
        ),
    );
    let loaded = Hooks::load(workspace.path(), &workspace.path().join("home"));

    let outcome = loaded.hooks.user_prompt_submit(&UserPrompt {
        prompt: "hello",
        cwd: workspace.path(),
        session_id: None,
    });

    assert!(!outcome.is_blocked());
    assert_eq!(workspace.read("log"), "prompted");
}

#[test]
fn a_blocked_prompt_reports_the_stop_reason() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(&format!(
        r#"{{"hooks": {{"UserPromptSubmit": [{{"hooks": [{}]}}]}}}}"#,
        handler("printf 'not now' >&2; exit 2")
    ));

    let outcome = hooks.user_prompt_submit(&UserPrompt {
        prompt: "hello",
        cwd: workspace.path(),
        session_id: None,
    });

    assert_eq!(outcome.blocked.as_deref(), Some("not now"));
}

#[test]
fn plain_prompt_stdout_becomes_context() {
    let workspace = Workspace::new();
    let (hooks, _) = workspace.load(&format!(
        r#"{{"hooks": {{"UserPromptSubmit": [{{"hooks": [{}]}}]}}}}"#,
        handler("printf 'working tree is clean'")
    ));

    let outcome = hooks.user_prompt_submit(&UserPrompt {
        prompt: "hello",
        cwd: workspace.path(),
        session_id: None,
    });

    assert_eq!(outcome.context, vec!["working tree is clean"]);
}
