//! The `niki` Rust client binary.
//!
//! Two entry points share one renderer: `replay` folds a committed protocol
//! fixture into a frame with no engine running, and `session` streams the same
//! frames from a live LangGraph engine. `replay` is what the parity tests pin.

use std::io::IsTerminal;

use anyhow::{Context, Result, bail};
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::terminal;
use niki_engine_client::{EngineClient, RunRequest, collect_transcript};
use niki_proto::{Frame, Transcript};
use niki_render::{TranscriptModel, display_width};
use niki_tui::{AppState, BINDINGS, Binding};

/// The Python engine release this client speaks to.
const ENGINE_VERSION: &str = "1.0.0";
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_ENGINE: &str = "http://127.0.0.1:2024";

const USAGE: &str = "\
Niki Agent — Rust client

USAGE:
    niki <command> [options]

COMMANDS:
    replay <fixture>   Render a committed protocol fixture, no engine needed
    session            Run one prompt against a live LangGraph engine
    keys               Print the app-level keymap
    --version          Print the version line
    --help             Print this text

REPLAY OPTIONS:
    --width <N>        Frame width in cells (default: COLUMNS or the terminal)
    --height <N>       Frame height in rows (default: LINES or the terminal)
    --ascii            Use the ASCII glyph registry
    --reader           Print the screen-reader transcript instead of a frame
    --check            Fail when the fixture yields no messages

SESSION OPTIONS:
    --engine <URL>     LangGraph API base (default: NIKI_ENGINE_URL or
                       http://127.0.0.1:2024)
    --assistant <ID>   Assistant id (default: agent)
    --prompt <TEXT>    Prompt to send (required)
    --width, --height, --ascii, --reader
                       As for replay

ENVIRONMENT:
    NIKI_SCREEN_READER=1   Start in reader mode
    NIKI_ASCII=1           Start with ASCII glyphs
    NIKI_ENGINE_URL        Default engine base for `session`
    COLUMNS, LINES         Default frame size when stdout is not a terminal
";

fn main() {
    let code = match run(std::env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("niki: {error:#}");
            1
        }
    };
    std::process::exit(code);
}

fn run(args: Vec<String>) -> Result<i32> {
    let Some(command) = args.first().map(String::as_str) else {
        print!("{USAGE}");
        return Ok(0);
    };
    match command {
        "--version" | "-V" | "version" => {
            println!("Niki Agent {ENGINE_VERSION} (rust {CLIENT_VERSION})");
            Ok(0)
        }
        "--help" | "-h" | "help" => {
            print!("{USAGE}");
            Ok(0)
        }
        "keys" => {
            for binding in BINDINGS.iter() {
                println!("{:<16} {:?}", chord_label(binding), binding.action);
            }
            Ok(0)
        }
        "replay" => replay(&args[1..]),
        "session" => session(&args[1..]),
        other => bail!("unknown command {other:?}; run `niki --help`"),
    }
}

#[derive(Default)]
struct Options {
    positional: Vec<String>,
    width: Option<usize>,
    height: Option<usize>,
    ascii: bool,
    reader: bool,
    check: bool,
    engine: Option<String>,
    assistant: Option<String>,
    prompt: Option<String>,
}

fn parse_options(args: &[String]) -> Result<Options> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(argument) = iter.next() {
        match argument.as_str() {
            "--width" => options.width = Some(size_flag(argument, iter.next())?),
            "--height" => options.height = Some(size_flag(argument, iter.next())?),
            "--ascii" => options.ascii = true,
            "--reader" => options.reader = true,
            "--check" => options.check = true,
            "--engine" => options.engine = Some(value_flag(argument, iter.next())?),
            "--assistant" => options.assistant = Some(value_flag(argument, iter.next())?),
            "--prompt" => options.prompt = Some(value_flag(argument, iter.next())?),
            other if other.starts_with("--") => bail!("unknown option {other}"),
            other => options.positional.push(other.to_owned()),
        }
    }
    Ok(options)
}

fn value_flag(flag: &str, value: Option<&String>) -> Result<String> {
    value
        .cloned()
        .with_context(|| format!("{flag} needs a value"))
}

fn size_flag(flag: &str, value: Option<&String>) -> Result<usize> {
    let raw = value_flag(flag, value)?;
    raw.parse()
        .with_context(|| format!("{flag} needs a number, got {raw:?}"))
}

fn replay(args: &[String]) -> Result<i32> {
    let options = parse_options(args)?;
    let path = options
        .positional
        .first()
        .context("replay needs a fixture path")?;
    if options.positional.len() > 1 {
        bail!("replay takes one fixture path");
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {path}"))?;
    let transcript = Transcript::from_json(&text).with_context(|| format!("parse {path}"))?;
    let model = TranscriptModel::from_transcript(&transcript);
    render(&model, &options, "replay")
}

fn session(args: &[String]) -> Result<i32> {
    let options = parse_options(args)?;
    let prompt = options.prompt.clone().context("session needs --prompt")?;
    let engine = options
        .engine
        .clone()
        .or_else(|| std::env::var("NIKI_ENGINE_URL").ok())
        .unwrap_or_else(|| DEFAULT_ENGINE.to_owned());
    let assistant = options
        .assistant
        .clone()
        .unwrap_or_else(|| "agent".to_owned());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .context("start the async runtime")?;
    let frames = runtime.block_on(run_stream(&engine, assistant, prompt))?;

    let mut model = TranscriptModel::default();
    for frame in &frames {
        model.apply(frame);
    }
    render(&model, &options, "live")
}

async fn run_stream(engine: &str, assistant: String, prompt: String) -> Result<Vec<Frame>> {
    let client = EngineClient::new(engine);
    let thread = client
        .create_thread()
        .await
        .with_context(|| format!("create a thread on {engine}"))?;
    let request = RunRequest::prompt(assistant, prompt);
    let stream = client
        .stream_run(&thread, &request)
        .await
        .context("start the run")?;
    collect_transcript(stream)
        .await
        .context("read the run stream")
}

fn render(model: &TranscriptModel, options: &Options, label: &'static str) -> Result<i32> {
    let state = AppState {
        model: model.clone(),
        reader: options.reader || env_flag("NIKI_SCREEN_READER"),
        ascii: options.ascii || env_flag("NIKI_ASCII"),
        label,
        ..AppState::default()
    };
    let (width, height) = frame_size(options.width, options.height);

    if state.reader {
        print!("{}", state.render_reader(width));
        return Ok(0);
    }

    let frame = state.render(width, height);
    if options.check {
        if state.model.messages.is_empty() {
            bail!("replay produced no messages");
        }
        for (index, line) in frame.lines.iter().enumerate() {
            let cells = display_width(line);
            if cells != width {
                bail!("line {index} is {cells} cells, expected {width}");
            }
        }
    }
    println!("{}", frame.to_text());
    Ok(0)
}

fn frame_size(width: Option<usize>, height: Option<usize>) -> (usize, usize) {
    let env = |name: &str| {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
    };
    let terminal = terminal::size()
        .ok()
        .filter(|_| std::io::stdout().is_terminal())
        .map(|(cols, rows)| (usize::from(cols), usize::from(rows)));
    let (fallback_width, fallback_height) = terminal.unwrap_or((100, 30));
    (
        width
            .or_else(|| env("COLUMNS"))
            .unwrap_or(fallback_width)
            .clamp(8, 500),
        height
            .or_else(|| env("LINES"))
            .unwrap_or(fallback_height)
            .clamp(3, 200),
    )
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

fn chord_label(binding: &Binding) -> String {
    let mut parts = Vec::new();
    if binding.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("ctrl".to_owned());
    }
    if binding.modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("shift".to_owned());
    }
    if binding.modifiers.contains(KeyModifiers::ALT) {
        parts.push("alt".to_owned());
    }
    parts.push(match binding.key {
        KeyCode::Char(' ') => "space".to_owned(),
        KeyCode::Char(character) => character.to_string(),
        KeyCode::Esc => "escape".to_owned(),
        KeyCode::BackTab => "backtab".to_owned(),
        other => format!("{other:?}").to_ascii_lowercase(),
    });
    parts.join("+")
}
