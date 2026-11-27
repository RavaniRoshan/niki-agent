//! Prove that an installed `niki` binary runs one real turn end to end.
//!
//! `niki --version` imports almost nothing, so a build that prints its version
//! can still fail on its first turn. This gate closes that gap without a network
//! beyond loopback: it starts a scripted OpenAI-compatible server, points a
//! throwaway profile at it, and asks the binary to write a file through a tool
//! call. Exit status is 0 only when every assertion holds.
//!
//! ```text
//! cargo run -q -p niki-smoke -- --niki /path/to/bin/niki
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const TARGET_FILE: &str = "smoke_turn.txt";
const EXPECTED_CONTENT: &str = "smoke turn ok";
const API_KEY_ENV: &str = "NIKI_SMOKE_API_KEY";
const API_KEY: &str = "smoke-turn-not-a-secret";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(180);

const PROMPT: &str = "Use the write_file tool to create smoke_turn.txt in the \
current directory with exactly the text \"smoke turn ok\". Then reply with one \
short sentence confirming it.";

fn main() {
    let options = match Options::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("niki-smoke: {message}");
            eprintln!("usage: niki-smoke --niki <path> [--timeout <seconds>] [--keep]");
            std::process::exit(2);
        }
    };

    if let Err(failure) = run(&options) {
        eprintln!("FAIL: {failure}");
        std::process::exit(1);
    }
    println!(
        "PASS: {} ran one tool turn end to end",
        options.binary.display()
    );
}

struct Options {
    binary: PathBuf,
    timeout: Duration,
    keep: bool,
}

impl Options {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut binary = None;
        let mut timeout = DEFAULT_TIMEOUT;
        let mut keep = false;
        let mut args = args.peekable();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--niki" => {
                    binary = Some(PathBuf::from(args.next().ok_or("--niki needs a path")?));
                }
                "--timeout" => {
                    let value = args.next().ok_or("--timeout needs seconds")?;
                    let seconds: u64 = value
                        .parse()
                        .map_err(|_| format!("--timeout is not a number: {value}"))?;
                    timeout = Duration::from_secs(seconds.max(1));
                }
                "--keep" => keep = true,
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        Ok(Self {
            binary: binary.ok_or("--niki is required")?,
            timeout,
            keep,
        })
    }
}

fn run(options: &Options) -> Result<(), String> {
    if !options.binary.is_file() {
        return Err(format!("{} is not a file", options.binary.display()));
    }

    let version = Command::new(&options.binary)
        .arg("--version")
        .output()
        .map_err(|error| format!("cannot run {}: {error}", options.binary.display()))?;
    if !version.status.success() {
        return Err(format!(
            "`--version` exited {:?}: {}",
            version.status.code(),
            String::from_utf8_lossy(&version.stderr).trim()
        ));
    }
    println!(
        "binary: {} ({})",
        options.binary.display(),
        String::from_utf8_lossy(&version.stdout).trim()
    );

    let scratch = Scratch::new()?;
    let workspace = scratch.path.join("workspace");
    let home = scratch.path.join("home");
    std::fs::create_dir_all(&workspace).map_err(io("workspace"))?;
    std::fs::create_dir_all(&home).map_err(io("home"))?;

    let server = Scripted::start()?;
    let config = scratch.path.join("config.toml");
    std::fs::write(&config, profile(&server.base_url)).map_err(io("config"))?;

    let mut child = Command::new(&options.binary)
        .arg("-n")
        .arg(PROMPT)
        .arg("--mode")
        .arg("auto")
        .arg("--config")
        .arg(&config)
        .arg("--root")
        .arg(&workspace)
        .arg("--quiet")
        .env(API_KEY_ENV, API_KEY)
        .env("NIKI_HOME", &home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot spawn {}: {error}", options.binary.display()))?;

    let status = wait(&mut child, options.timeout)?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    if !status.success() {
        return Err(format!(
            "the turn exited {:?}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
            status.code()
        ));
    }

    let written = std::fs::read_to_string(workspace.join(TARGET_FILE)).map_err(|error| {
        format!(
            "the binary never wrote {TARGET_FILE}: {error}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        )
    })?;
    if written != EXPECTED_CONTENT {
        return Err(format!(
            "{TARGET_FILE} holds {:?}, expected {:?}",
            written, EXPECTED_CONTENT
        ));
    }

    let requests = server.requests();
    if requests.len() < 2 {
        return Err(format!(
            "the model saw {} request(s); a tool turn needs two",
            requests.len()
        ));
    }
    if !requests[0].contains("\"stream\":true") {
        return Err("the first request did not ask for a stream".to_string());
    }
    if !requests[1].contains("\"role\":\"tool\"") {
        return Err("the tool result never reached the model".to_string());
    }
    if !requests[1].contains(TARGET_FILE) {
        return Err("the tool result did not name the written file".to_string());
    }

    println!("requests: 2 (tool call, then tool result)");
    println!("wrote: {written:?} to {TARGET_FILE}");

    if options.keep {
        println!("kept: {}", scratch.path.display());
        std::mem::forget(scratch);
    }
    Ok(())
}

fn profile(base_url: &str) -> String {
    format!(
        "# Written by niki-smoke: a throwaway release-gate profile.\n\
         [models]\n\
         default = \"smoke:smoke-model\"\n\
         \n\
         [providers.smoke]\n\
         base_url = \"{base_url}\"\n\
         api_key_env = \"{API_KEY_ENV}\"\n\
         display_name = \"Smoke turn (loopback)\"\n\
         \n\
         models = [\"smoke-model\"]\n"
    )
}

fn wait(child: &mut Child, timeout: Duration) -> Result<std::process::ExitStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("the turn did not finish within {timeout:?}"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => return Err(format!("cannot wait for the binary: {error}")),
        }
    }
}

fn drain<R: Read>(stream: Option<R>) -> String {
    let Some(mut stream) = stream else {
        return String::new();
    };
    let mut buffer = Vec::new();
    let _ = stream.read_to_end(&mut buffer);
    String::from_utf8_lossy(&buffer).into_owned()
}

fn io(what: &'static str) -> impl Fn(std::io::Error) -> String {
    move |error| format!("cannot prepare the {what}: {error}")
}

/// A scripted OpenAI-compatible server: the first request gets a `write_file`
/// tool call, every later request gets a short answer.
struct Scripted {
    base_url: String,
    seen: Arc<Mutex<Vec<String>>>,
}

impl Scripted {
    fn start() -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("cannot bind a loopback port: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("cannot read the bound address: {error}"))?;
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&seen);

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let body = read_request(&mut stream);
                let turn = {
                    let mut seen = recorded.lock().expect("lock");
                    seen.push(body);
                    seen.len()
                };
                let response = if turn == 1 {
                    tool_call_turn()
                } else {
                    answer_turn()
                };
                write_response(&mut stream, &response);
            }
        });

        Ok(Self {
            base_url: format!("http://{address}/v1"),
            seen,
        })
    }

    fn requests(&self) -> Vec<String> {
        self.seen.lock().expect("lock").clone()
    }
}

fn tool_call_turn() -> String {
    sse(&[
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_smoke_turn_1","type":"function","function":{"name":"write_file","arguments":"{\"path\":\"smoke_turn.txt\",\"content\":\"smoke turn ok\"}"}}]},"finish_reason":null}]}"#,
        r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
    ])
}

fn answer_turn() -> String {
    sse(&[
        r#"{"choices":[{"delta":{"content":"Smoke turn complete."},"finish_reason":null}]}"#,
        r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
        r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":4}}"#,
    ])
}

fn sse(frames: &[&str]) -> String {
    let mut body = String::new();
    for frame in frames {
        body.push_str("data: ");
        body.push_str(frame);
        body.push_str("\n\n");
    }
    body.push_str("data: [DONE]\n\n");
    body
}

fn read_request(stream: &mut TcpStream) -> String {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
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
            let body_start = split + 4;
            while buffer.len() < body_start + length {
                let read = stream.read(&mut chunk).unwrap_or(0);
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
            }
            break;
        }
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

fn write_response(stream: &mut TcpStream, body: &str) {
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Throwaway directories, removed on drop unless the run is kept for debugging.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!("niki-smoke-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&path)
            .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
        Ok(Self { path })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
