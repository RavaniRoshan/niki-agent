//! End-to-end check of the W1 exit criterion: `niki -n "prompt"` runs a
//! real turn against a mock provider, writes a file, and prints the answer.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

/// A one-shot OpenAI-compatible SSE server. Each accepted connection answers
/// with the next scripted body and records the request body it received.
fn serve(responses: Vec<String>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);

    std::thread::spawn(move || {
        for response in responses {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let body = read_request(&mut stream);
            recorded.lock().expect("lock").push(body);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    (format!("http://{address}"), seen)
}

/// Read headers and the `Content-Length` body of one HTTP request.
fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(split) = find(&buffer, b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buffer[..split]).to_ascii_lowercase();
            let length = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if buffer.len() >= split + 4 + length {
                break;
            }
        }
    }
    let Some(split) = find(&buffer, b"\r\n\r\n") else {
        return String::new();
    };
    String::from_utf8_lossy(&buffer[split + 4..]).into_owned()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Record `workspace` as trusted in `home`, the grant `niki trust` writes.
fn trust_workspace(home: &Path, workspace: &Path) {
    let key = workspace.canonicalize().expect("canonical workspace");
    std::fs::write(
        home.join("trusted_workspaces.toml"),
        format!(
            "version = 1\n\n[projects.\"{}\"]\ntrusted_at = 0\n",
            key.display()
        ),
    )
    .expect("trust store");
}

fn sse(frames: &[Value]) -> String {
    let mut body = String::new();
    for frame in frames {
        body.push_str(&format!("data: {frame}\n\n"));
    }
    body.push_str("data: [DONE]\n\n");
    body
}

fn tool_call_turn() -> String {
    sse(&[
        json!({"choices": [{"delta": {"tool_calls": [{
            "index": 0,
            "id": "call_1",
            "type": "function",
            "function": {
                "name": "write_file",
                "arguments": "{\"path\":\"hello.txt\",\"content\":\"hello from rust\"}"
            }
        }]}, "finish_reason": null}]}),
        json!({"choices": [{"delta": {}, "finish_reason": "tool_calls"}]}),
    ])
}

fn text_tool_call_turn() -> String {
    sse(&[
        json!({"choices": [{"delta": {"content": "Checking."}, "finish_reason": null}]}),
        json!({"choices": [{"delta": {"tool_calls": [{
            "index": 0,
            "id": "call_1",
            "type": "function",
            "function": {
                "name": "write_file",
                "arguments": "{\"path\":\"hello.txt\",\"content\":\"hello from rust\"}"
            }
        }]}, "finish_reason": null}]}),
        json!({"choices": [{"delta": {}, "finish_reason": "tool_calls"}]}),
    ])
}

fn read_file_turn(path: &str) -> String {
    sse(&[
        json!({"choices": [{"delta": {"tool_calls": [{
            "index": 0,
            "id": "call_1",
            "type": "function",
            "function": {
                "name": "read_file",
                "arguments": format!("{{\"file_path\":\"{path}\"}}")
            }
        }]}, "finish_reason": null}]}),
        json!({"choices": [{"delta": {}, "finish_reason": "tool_calls"}]}),
    ])
}

fn answer_turn() -> String {
    sse(&[
        json!({"choices": [{"delta": {"content": "Wrote hello.txt."}, "finish_reason": null}]}),
        json!({"choices": [{"delta": {}, "finish_reason": "stop"}]}),
        json!({"choices": [], "usage": {"prompt_tokens": 12, "completion_tokens": 5}}),
    ])
}

fn write_config(path: &Path, base_url: &str) {
    std::fs::write(
        path,
        format!(
            "[models]\ndefault = \"mock:mock-model\"\n\n[providers.mock]\nbase_url = \"{base_url}\"\napi_key_env = \"NIKI_TEST_KEY\"\n"
        ),
    )
    .expect("config");
}

#[test]
fn one_prompt_writes_a_file_through_the_turn_loop() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let home = tempfile::tempdir().expect("home");
    let (base_url, seen) = serve(vec![tool_call_turn(), answer_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .arg("-n")
        .arg("create hello.txt")
        .arg("--mode")
        .arg("auto")
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .arg("--quiet")
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout, "Wrote hello.txt.\n");
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("hello.txt")).expect("file"),
        "hello from rust"
    );

    let requests = seen.lock().expect("lock");
    assert_eq!(requests.len(), 2, "one request per model turn");
    assert!(requests[0].contains("\"stream\":true"), "{}", requests[0]);
    assert!(requests[1].contains("\"role\":\"tool\""), "{}", requests[1]);
    assert!(requests[1].contains("hello.txt"), "{}", requests[1]);
}

#[test]
fn discovered_skills_reach_the_request_system_prompt() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let home = tempfile::tempdir().expect("home");
    trust_workspace(home.path(), workspace.path());
    let skill = workspace.path().join(".niki/skills/deploy");
    std::fs::create_dir_all(&skill).expect("skill directory");
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: deploy\ndescription: Ship a release safely.\n---\nbody\n",
    )
    .expect("skill");
    let broken = workspace.path().join(".niki/skills/broken");
    std::fs::create_dir_all(&broken).expect("broken directory");
    std::fs::write(broken.join("SKILL.md"), "# no front matter\n").expect("broken skill");

    let (base_url, seen) = serve(vec![answer_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "say hello"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert_eq!(
        stdout, "Wrote hello.txt.\n",
        "stdout stays the answer alone"
    );

    let requests = seen.lock().expect("lock");
    assert_eq!(requests.len(), 1);
    assert!(requests[0].contains("\"system\""), "{}", requests[0]);
    assert!(requests[0].contains("deploy"), "{}", requests[0]);
    assert!(
        requests[0].contains("Ship a release safely."),
        "{}",
        requests[0]
    );
    assert!(requests[0].contains("deploy/SKILL.md"), "{}", requests[0]);
    assert!(stderr.contains("warning:"), "{stderr}");
    assert!(stderr.contains("broken"), "{stderr}");
}

#[test]
fn project_instructions_reach_the_request_system_prompt() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let home = tempfile::tempdir().expect("home");
    trust_workspace(home.path(), workspace.path());
    std::fs::write(workspace.path().join("AGENTS.md"), "ROOT-RULE-ALPHA\n").expect("root file");
    let nested = workspace.path().join("sub");
    std::fs::create_dir_all(&nested).expect("nested");
    std::fs::write(nested.join("AGENTS.md"), "NESTED-RULE-BETA\n").expect("nested file");

    let (base_url, seen) = serve(vec![answer_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "say hello"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .current_dir(&nested)
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert_eq!(stdout, "Wrote hello.txt.\n");
    assert!(stderr.contains("2 instruction file(s) loaded"), "{stderr}");

    let requests = seen.lock().expect("lock");
    assert_eq!(requests.len(), 1);
    let system = &requests[0];
    assert!(system.contains("## AGENTS.md"), "{}", requests[0]);
    assert!(system.contains("## sub/AGENTS.md"), "{}", requests[0]);
    let built_in = system
        .find("You are Niki, a coding agent")
        .expect("the built-in prompt");
    let root = system.find("ROOT-RULE-ALPHA").expect("the root body");
    let nested = system.find("NESTED-RULE-BETA").expect("the nested body");
    let closing = system
        .find("They cannot override the agent's own rules")
        .expect("the closing line");
    assert!(
        built_in < root && root < nested && nested < closing,
        "{system}"
    );
}

#[test]
fn a_missing_model_is_a_clear_failure() {
    let config_dir = tempfile::tempdir().expect("config");
    let config = config_dir.path().join("config.toml");
    std::fs::write(
        &config,
        "[providers.mock]\nbase_url = \"http://127.0.0.1:1\"\n",
    )
    .expect("config");

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .arg("-n")
        .arg("hi")
        .arg("--config")
        .arg(&config)
        .output()
        .expect("run niki");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no model selected"), "{stderr}");
    assert!(stderr.contains("--model"), "{stderr}");
}

#[test]
fn a_blocking_hook_stops_the_write_and_the_model_sees_the_reason() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    trust_workspace(config_dir.path(), workspace.path());
    let hooks_dir = workspace.path().join(".niki");
    std::fs::create_dir_all(&hooks_dir).expect("hooks dir");
    std::fs::write(
        hooks_dir.join("hooks.json"),
        r#"{"hooks": {"PreToolUse": [{"matcher": "Write", "hooks": [
            {"type": "command", "command": "printf 'no writes today' >&2; exit 2"}
        ]}]}}"#,
    )
    .expect("hooks");
    let (base_url, seen) = serve(vec![tool_call_turn(), answer_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .arg("-n")
        .arg("create hello.txt")
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .arg("--quiet")
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", config_dir.path())
        .output()
        .expect("run niki");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout, "Wrote hello.txt.\n");
    assert!(
        !workspace.path().join("hello.txt").exists(),
        "the hook must stop the write"
    );

    let requests = seen.lock().expect("lock");
    assert_eq!(requests.len(), 2, "the model still gets a second turn");
    assert!(
        requests[1].contains("blocked by hook: no writes today"),
        "{}",
        requests[1]
    );
}

#[test]
fn json_output_prints_one_object_with_the_answer_and_usage() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let (base_url, _seen) = serve(vec![answer_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "say hello", "--output-format", "json", "--quiet"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", config_dir.path())
        .output()
        .expect("run niki");

    assert!(output.status.success(), "exit {:?}", output.status.code());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).expect("one JSON object");
    assert_eq!(value["answer"], "Wrote hello.txt.");
    assert_eq!(value["steps"], 1);
    assert_eq!(value["stop"], "end_turn");
    assert_eq!(value["usage"]["input_tokens"], 12);
    assert!(value["session"].is_string(), "{stdout}");

    let audit = &value["context_audit"];
    let part = |name: &str| {
        audit[name]
            .as_u64()
            .unwrap_or_else(|| panic!("context_audit.{name} is not a number: {stdout}"))
    };
    assert!(part("system") > 0, "{stdout}");
    assert!(part("tool_schemas") > 0, "{stdout}");
    assert!(part("messages") > 0, "{stdout}");
    assert_eq!(
        part("tools"),
        11,
        "the ten built-in tools and the task tool"
    );
    assert_eq!(
        part("total"),
        part("system") + part("tool_schemas") + part("messages"),
        "{stdout}"
    );
}

/// Parse one JSON object per stdout line, failing with the offending line.
fn jsonl_objects(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|error| panic!("not JSON: {line:?} ({error})"))
        })
        .collect()
}

fn jsonl_types(values: &[Value]) -> Vec<&str> {
    values
        .iter()
        .map(|value| {
            value["type"]
                .as_str()
                .unwrap_or_else(|| panic!("every line has a type: {value}"))
        })
        .collect()
}

#[test]
fn jsonl_streams_the_turn_as_one_object_per_event() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let home = tempfile::tempdir().expect("home");
    let (base_url, _seen) = serve(vec![text_tool_call_turn(), answer_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args([
            "-n",
            "create hello.txt",
            "--output-format",
            "jsonl",
            "--mode",
            "auto",
            "--quiet",
        ])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stdout.contains('\u{1b}'),
        "stdout carries no ANSI: {stdout:?}"
    );

    let values = jsonl_objects(&stdout);
    let types = jsonl_types(&values);
    let required = ["start", "text", "tool_call", "tool_result", "usage", "end"];
    let mut next = 0;
    for wanted in required {
        let at = types[next..]
            .iter()
            .position(|kind| *kind == wanted)
            .map(|offset| offset + next)
            .unwrap_or_else(|| panic!("missing {wanted} in {types:?}"));
        next = at + 1;
    }
    assert_eq!(types.first(), Some(&"start"), "{types:?}");
    assert_eq!(types.last(), Some(&"end"), "{types:?}");
    assert_eq!(
        types.iter().filter(|kind| **kind == "end").count(),
        1,
        "end is written once: {types:?}"
    );

    let start = &values[0];
    assert_eq!(start["model"], "mock:mock-model");
    assert!(start["session"].is_string(), "{start}");
    assert!(start["cwd"].is_string(), "{start}");

    let call = values
        .iter()
        .find(|value| value["type"] == "tool_call")
        .expect("tool_call");
    assert_eq!(call["name"], "write_file");
    assert!(call["id"].is_string(), "{call}");
    assert_eq!(call["arguments"]["path"], "hello.txt", "{call}");
    assert_eq!(call["arguments"]["content"], "hello from rust", "{call}");

    let result = values
        .iter()
        .find(|value| value["type"] == "tool_result")
        .expect("tool_result");
    assert_eq!(result["id"], call["id"]);
    assert_eq!(result["name"], "write_file");
    assert_eq!(result["ok"], true);
    assert!(result["truncated"].is_null(), "{result}");

    let usage = values
        .iter()
        .find(|value| value["type"] == "usage")
        .expect("usage");
    assert_eq!(usage["input"], 12);
    assert_eq!(usage["output"], 5);
    assert_eq!(usage["total"], 17);

    let end = values.last().expect("end");
    assert_eq!(end["reason"], "end_turn");
    assert_eq!(end["steps"], 2);
    assert_eq!(end["tool_calls"], 1);
    assert_eq!(end["usage"]["input"], 12);
    assert_eq!(end["usage"]["output"], 5);

    assert_eq!(
        std::fs::read_to_string(workspace.path().join("hello.txt")).expect("file"),
        "hello from rust"
    );
}

#[test]
fn jsonl_truncates_a_tool_result_larger_than_four_kibibytes() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let home = tempfile::tempdir().expect("home");
    let big = "x".repeat(5000);
    std::fs::write(workspace.path().join("big.txt"), &big).expect("big file");
    let (base_url, _seen) = serve(vec![read_file_turn("big.txt"), answer_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args([
            "-n",
            "read big.txt",
            "--output",
            "jsonl",
            "--mode",
            "auto",
            "--quiet",
        ])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {stdout}",
        output.status.code()
    );

    let values = jsonl_objects(&stdout);
    let result = values
        .iter()
        .find(|value| value["type"] == "tool_result")
        .expect("tool_result");
    assert_eq!(result["name"], "read_file");
    assert_eq!(result["truncated"], true, "{result}");
    let text = result["text"].as_str().expect("text is a string");
    assert_eq!(text.len(), 4096, "cut at the 4 KiB budget");
    assert!(text.chars().all(|character| character == 'x'), "{result}");
    assert_eq!(values.last().expect("end")["type"], "end");
}

#[test]
fn jsonl_ends_after_a_failed_turn_with_error_then_end() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let home = tempfile::tempdir().expect("home");
    let config = config_dir.path().join("config.toml");
    write_config(&config, "http://127.0.0.1:1");

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "hi", "--output-format", "jsonl", "--quiet"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains('\u{1b}'),
        "stdout carries no ANSI: {stdout:?}"
    );

    let values = jsonl_objects(&stdout);
    let types = jsonl_types(&values);
    assert_eq!(types.first(), Some(&"start"), "{types:?}");
    assert_eq!(types[types.len() - 2], "error", "{types:?}");
    assert_eq!(types.last(), Some(&"end"), "{types:?}");
    assert_eq!(
        types.iter().filter(|kind| **kind == "end").count(),
        1,
        "end is written once: {types:?}"
    );

    let error = &values[values.len() - 2];
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "{error}"
    );
    assert_eq!(values.last().expect("end")["reason"], "error");
}

#[test]
fn the_step_budget_exits_124() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let (base_url, _seen) = serve(vec![tool_call_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "loop", "--max-steps", "1", "--quiet"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", config_dir.path())
        .output()
        .expect("run niki");

    assert_eq!(output.status.code(), Some(124));
}

#[test]
fn continue_sends_the_saved_turns_back_to_the_model() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let (base_url, seen) = serve(vec![answer_turn(), answer_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let run = |prompt: &str, resume: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_niki"));
        command
            .args(["-n", prompt, "--quiet"])
            .arg("--config")
            .arg(&config)
            .arg("--root")
            .arg(workspace.path())
            .env("NIKI_TEST_KEY", "test-key")
            .env("NIKI_HOME", config_dir.path());
        if resume {
            command.arg("--continue");
        }
        command.output().expect("run niki")
    };

    let first = run("first prompt", false);
    assert!(first.status.success(), "exit {:?}", first.status.code());
    let second = run("second prompt", true);
    assert!(
        second.status.success(),
        "exit {:?}\nstderr: {}",
        second.status.code(),
        String::from_utf8_lossy(&second.stderr)
    );

    let requests = seen.lock().expect("lock");
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1].contains("\"role\":\"assistant\""),
        "the saved answer is replayed: {}",
        requests[1]
    );
    assert!(
        requests[1].contains("Wrote hello.txt."),
        "the saved answer text is replayed: {}",
        requests[1]
    );
    assert!(
        requests[1].contains("first prompt") && requests[1].contains("second prompt"),
        "both prompts are in the request: {}",
        requests[1]
    );
}

#[test]
fn an_unknown_resume_id_is_a_clear_failure() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let config = config_dir.path().join("config.toml");
    write_config(&config, "http://127.0.0.1:1");

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "hi", "--resume", "no-such-session", "--quiet"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", config_dir.path())
        .output()
        .expect("run niki");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no saved session"), "{stderr}");
}

#[test]
fn continue_skips_an_archived_session_while_an_explicit_id_still_resumes_it() {
    let workspace = tempfile::tempdir().expect("workspace");
    let home = tempfile::tempdir().expect("home");
    let (base_url, seen) = serve(vec![answer_turn(), answer_turn()]);
    let config = home.path().join("config.toml");
    write_config(&config, &base_url);

    let store =
        niki_session::SessionStore::open(home.path().join("sessions.db")).expect("session store");
    let older = store.create(workspace.path()).expect("older session");
    store
        .append(&older, "user", "older prompt")
        .expect("older prompt");
    let archived = store.create(workspace.path()).expect("archived session");
    store
        .append(&archived, "user", "archived prompt")
        .expect("archived prompt");
    store.set_archived(&archived, true).expect("archive");
    drop(store);

    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_niki"))
            .args(["-n", "next", "--quiet"])
            .args(args)
            .arg("--config")
            .arg(&config)
            .arg("--root")
            .arg(workspace.path())
            .env("NIKI_TEST_KEY", "test-key")
            .env("NIKI_HOME", home.path())
            .output()
            .expect("run niki")
    };

    let continued = run(&["--continue"]);
    assert!(
        continued.status.success(),
        "exit {:?}\nstderr: {}",
        continued.status.code(),
        String::from_utf8_lossy(&continued.stderr)
    );

    let resumed = run(&["--resume", archived.as_str()]);
    assert!(
        resumed.status.success(),
        "exit {:?}\nstderr: {}",
        resumed.status.code(),
        String::from_utf8_lossy(&resumed.stderr)
    );

    let requests = seen.lock().expect("lock");
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0].contains("older prompt"),
        "the live session supplies the history: {}",
        requests[0]
    );
    assert!(
        !requests[0].contains("archived prompt"),
        "the archived session is skipped: {}",
        requests[0]
    );
    assert!(
        requests[1].contains("archived prompt"),
        "an explicit id still loads it: {}",
        requests[1]
    );
}

#[test]
fn continue_with_only_an_archived_session_fails_as_if_none_existed() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let config = config_dir.path().join("config.toml");
    write_config(&config, "http://127.0.0.1:1");

    let store =
        niki_session::SessionStore::open(config_dir.path().join("sessions.db")).expect("store");
    let archived = store.create(workspace.path()).expect("session");
    store.append(&archived, "user", "old news").expect("append");
    store.set_archived(&archived, true).expect("archive");
    drop(store);

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "hi", "--continue", "--quiet"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", config_dir.path())
        .output()
        .expect("run niki");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no saved session"), "{stderr}");
}

#[cfg(unix)]
fn notify_program(dir: &Path, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("notify.sh");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("notify script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("executable");
    path
}

/// The script used by the record-the-call tests: it appends its arguments and
/// the stdin document to the file named by its first argument.
#[cfg(unix)]
const RECORDING_BODY: &str = "\
out=\"$1\"; shift
{
  printf 'argc=%s\\n' \"$#\"
  for arg in \"$@\"; do printf 'arg=%s\\n' \"$arg\"; done
  cat
  printf '\\nEND\\n'
} >> \"$out\"";

#[cfg(unix)]
fn add_notify(config: &Path, program: &Path, args: &[String]) {
    let mut text = std::fs::read_to_string(config).expect("config");
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    text.push_str(&format!(
        "\n[notify]\nprogram = {:?}\nargs = [{}]\n",
        program.display().to_string(),
        args.join(", ")
    ));
    std::fs::write(config, text).expect("config with notify");
}

#[cfg(unix)]
fn notify_documents(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.starts_with('{'))
        .map(|line| serde_json::from_str(line).expect("JSON payload"))
        .collect()
}

#[cfg(unix)]
fn notify_batches(path: &Path) -> Vec<Vec<Value>> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.split("END")
        .filter_map(|batch| {
            let documents: Vec<Value> = batch
                .lines()
                .filter(|line| line.starts_with('{'))
                .map(|line| serde_json::from_str(line).expect("JSON payload"))
                .collect();
            (!documents.is_empty()).then_some(documents)
        })
        .collect()
}

#[cfg(unix)]
fn wait_for_notify(path: &Path) -> Vec<Value> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let documents = notify_documents(path);
        if !documents.is_empty() {
            return documents;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the notify program did not run"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
fn wait_for_batches(path: &Path, count: usize) -> Vec<Vec<Value>> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let batches = notify_batches(path);
        if batches.len() >= count {
            return batches;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the notify program ran {} time(s), expected {count}",
            batches.len()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
fn text_turn(text: &str) -> String {
    sse(&[
        json!({"choices": [{"delta": {"content": text}, "finish_reason": null}]}),
        json!({"choices": [{"delta": {}, "finish_reason": "stop"}]}),
        json!({"choices": [], "usage": {"prompt_tokens": 12, "completion_tokens": 5}}),
    ])
}

#[cfg(unix)]
fn task_call_turn() -> String {
    let arguments = json!({"description": "child job", "prompt": "do the child job"}).to_string();
    sse(&[
        json!({"choices": [{"delta": {"tool_calls": [{
            "index": 0,
            "id": "call_task",
            "type": "function",
            "function": {"name": "task", "arguments": arguments}
        }]}, "finish_reason": null}]}),
        json!({"choices": [{"delta": {}, "finish_reason": "tool_calls"}]}),
    ])
}

#[cfg(unix)]
#[test]
fn the_notify_program_runs_once_per_turn_with_the_documented_payload() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let home = tempfile::tempdir().expect("home");
    let (base_url, _seen) = serve(vec![answer_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let log = config_dir.path().join("notify.log");
    let script = notify_program(config_dir.path(), RECORDING_BODY);
    add_notify(
        &config,
        &script,
        &[
            log.display().to_string(),
            "--title".to_owned(),
            "Niki".to_owned(),
            "$(echo pwned)".to_owned(),
        ],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "say hello", "--output-format", "json", "--quiet"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    let printed: Value = serde_json::from_str(stdout.trim()).expect("one JSON object");

    let batches = wait_for_batches(&log, 1);
    assert_eq!(batches.len(), 1, "the program runs once per turn");
    let payload = &batches[0][0];
    assert_eq!(payload["event"], "turn_end");
    assert_eq!(payload["session"], printed["session"]);
    assert_eq!(payload["model"], "mock:mock-model");
    assert_eq!(
        payload["cwd"],
        workspace
            .path()
            .canonicalize()
            .expect("canonical")
            .display()
            .to_string()
    );
    assert_eq!(payload["reason"], "end_turn");
    assert_eq!(payload["steps"], 1);
    assert_eq!(payload["tool_calls"], 0);
    assert_eq!(payload["answer"], "Wrote hello.txt.");

    let recorded = std::fs::read_to_string(&log).expect("log");
    assert!(recorded.contains("argc=3\n"), "{recorded}");
    let args: Vec<&str> = recorded
        .lines()
        .filter_map(|line| line.strip_prefix("arg="))
        .collect();
    assert_eq!(
        args,
        vec!["--title", "Niki", "$(echo pwned)"],
        "arguments are passed verbatim"
    );
}

#[cfg(unix)]
#[test]
fn a_configured_program_without_a_program_spawns_nothing() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let home = tempfile::tempdir().expect("home");
    let (base_url, _seen) = serve(vec![answer_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);
    let mut text = std::fs::read_to_string(&config).expect("config");
    text.push_str("\n[notify]\nargs = [\"--title\", \"Niki\"]\n");
    std::fs::write(&config, text).expect("config");

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "say hello", "--quiet"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "exit {:?}", output.status.code());
    assert_eq!(stdout, "Wrote hello.txt.\n");
    assert!(!stderr.contains("notify"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn a_failing_notify_program_warns_on_stderr_and_keeps_the_exit_code() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let home = tempfile::tempdir().expect("home");
    let (base_url, _seen) = serve(vec![answer_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);
    let script = notify_program(config_dir.path(), "exit 7");
    add_notify(&config, &script, &[]);

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "say hello", "--output-format", "json", "--quiet"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "a failing notifier does not change the exit code: {:?}",
        output.status.code()
    );
    let printed: Value = serde_json::from_str(stdout.trim()).expect("stdout stays one JSON object");
    assert_eq!(printed["answer"], "Wrote hello.txt.");
    assert!(!stdout.contains("warning"), "stdout stays clean: {stdout}");
    assert!(stderr.contains("niki: warning:"), "{stderr}");
    assert!(stderr.contains("notify"), "{stderr}");
    assert!(stderr.contains("exited with"), "{stderr}");
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.contains("niki: warning:"))
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn a_missing_or_non_executable_program_is_a_warning_not_an_error() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let home = tempfile::tempdir().expect("home");
    let (base_url, _seen) = serve(vec![answer_turn(), answer_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let base_config = std::fs::read_to_string(&config).expect("config");
    let run = |program: &Path| {
        std::fs::write(
            &config,
            format!(
                "{base_config}\n[notify]\nprogram = {:?}\n",
                program.display().to_string()
            ),
        )
        .expect("config");
        Command::new(env!("CARGO_BIN_EXE_niki"))
            .args(["-n", "say hello", "--quiet"])
            .arg("--config")
            .arg(&config)
            .arg("--root")
            .arg(workspace.path())
            .env("NIKI_TEST_KEY", "test-key")
            .env("NIKI_HOME", home.path())
            .output()
            .expect("run niki")
    };

    let missing = run(Path::new("/nonexistent/niki-notify-test"));
    let stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        missing.status.success(),
        "a missing notifier is not an error: {:?}",
        missing.status.code()
    );
    assert_eq!(
        String::from_utf8_lossy(&missing.stdout),
        "Wrote hello.txt.\n"
    );
    assert!(stderr.contains("could not start"), "{stderr}");

    let not_executable = config_dir.path().join("plain.sh");
    std::fs::write(&not_executable, "#!/bin/sh\nexit 0\n").expect("file");
    let refused = run(&not_executable);
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        refused.status.success(),
        "a non-executable notifier is not an error: {:?}",
        refused.status.code()
    );
    assert_eq!(
        String::from_utf8_lossy(&refused.stdout),
        "Wrote hello.txt.\n"
    );
    assert!(stderr.contains("could not start"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn a_subagent_turn_does_not_notify() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let home = tempfile::tempdir().expect("home");
    let (base_url, seen) = serve(vec![
        task_call_turn(),
        text_turn("Child finished."),
        text_turn("Parent done."),
    ]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let log = config_dir.path().join("notify.log");
    let script = notify_program(config_dir.path(), RECORDING_BODY);
    add_notify(&config, &script, &[log.display().to_string()]);

    let output = Command::new(env!("CARGO_BIN_EXE_niki"))
        .args(["-n", "delegate this", "--quiet"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout, "Parent done.\n");
    assert_eq!(seen.lock().expect("lock").len(), 3, "parent, child, parent");

    let documents = wait_for_notify(&log);
    assert_eq!(
        documents.len(),
        1,
        "only the parent turn notifies: {documents:?}"
    );
    assert_eq!(documents[0]["answer"], "Parent done.");
    assert_eq!(documents[0]["steps"], 2);
    assert_eq!(documents[0]["tool_calls"], 1);
}
