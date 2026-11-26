//! End-to-end check of the W1 exit criterion: `niki-rs -n "prompt"` runs a
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

    let output = Command::new(env!("CARGO_BIN_EXE_niki-rs"))
        .arg("-n")
        .arg("create hello.txt")
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .arg("--quiet")
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki-rs");

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

    let output = Command::new(env!("CARGO_BIN_EXE_niki-rs"))
        .args(["-n", "say hello"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", home.path())
        .output()
        .expect("run niki-rs");

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
fn a_missing_model_is_a_clear_failure() {
    let config_dir = tempfile::tempdir().expect("config");
    let config = config_dir.path().join("config.toml");
    std::fs::write(
        &config,
        "[providers.mock]\nbase_url = \"http://127.0.0.1:1\"\n",
    )
    .expect("config");

    let output = Command::new(env!("CARGO_BIN_EXE_niki-rs"))
        .arg("-n")
        .arg("hi")
        .arg("--config")
        .arg(&config)
        .output()
        .expect("run niki-rs");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no model selected"), "{stderr}");
    assert!(stderr.contains("--model"), "{stderr}");
}

#[test]
fn a_blocking_hook_stops_the_write_and_the_model_sees_the_reason() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
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

    let output = Command::new(env!("CARGO_BIN_EXE_niki-rs"))
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
        .expect("run niki-rs");

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

    let output = Command::new(env!("CARGO_BIN_EXE_niki-rs"))
        .args(["-n", "say hello", "--output-format", "json", "--quiet"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", config_dir.path())
        .output()
        .expect("run niki-rs");

    assert!(output.status.success(), "exit {:?}", output.status.code());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).expect("one JSON object");
    assert_eq!(value["answer"], "Wrote hello.txt.");
    assert_eq!(value["steps"], 1);
    assert_eq!(value["stop"], "end_turn");
    assert_eq!(value["usage"]["input_tokens"], 12);
    assert!(value["session"].is_string(), "{stdout}");
}

#[test]
fn the_step_budget_exits_124() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config_dir = tempfile::tempdir().expect("config");
    let (base_url, _seen) = serve(vec![tool_call_turn()]);
    let config = config_dir.path().join("config.toml");
    write_config(&config, &base_url);

    let output = Command::new(env!("CARGO_BIN_EXE_niki-rs"))
        .args(["-n", "loop", "--max-steps", "1", "--quiet"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", config_dir.path())
        .output()
        .expect("run niki-rs");

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
        let mut command = Command::new(env!("CARGO_BIN_EXE_niki-rs"));
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
        command.output().expect("run niki-rs")
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

    let output = Command::new(env!("CARGO_BIN_EXE_niki-rs"))
        .args(["-n", "hi", "--resume", "no-such-session", "--quiet"])
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(workspace.path())
        .env("NIKI_TEST_KEY", "test-key")
        .env("NIKI_HOME", config_dir.path())
        .output()
        .expect("run niki-rs");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no saved session"), "{stderr}");
}
