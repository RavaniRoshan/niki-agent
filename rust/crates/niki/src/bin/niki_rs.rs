//! `niki-rs` — the headless entry point.
//!
//! Reads the same `config.toml` the Python product reads, builds the provider
//! the `[models].default` spec names, and runs one prompt through the turn loop
//! in the current workspace. Named `niki-rs` until cutover (RULES.md 2).

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use niki_agent::{Agent, AgentConfig, AgentEvent, AllowList, CancelHandle, Mode};
use niki_config::{Config, Protocol, config_home};
use niki_provider::{AnthropicProvider, OpenAiProvider, RetryPolicy};
use niki_tools::ToolContext;

const USAGE: &str = "\
niki-rs — headless Niki agent (Rust)

USAGE:
    niki-rs -n <PROMPT> [options]

OPTIONS:
    -n, --prompt <TEXT>   Prompt to run (required)
    --model <PROVIDER:MODEL>
                          Model to use; defaults to [models].default in config
    --config <PATH>       Config file (default: $NIKI_HOME/config.toml, else
                          ~/.niki/config.toml)
    --root <PATH>         Workspace root (default: the current directory)
    --mode <MODE>         manual, auto or yolo (default: manual)
    --allow <SPEC>        Comma-separated shell allow-list, e.g. \"git*,cargo*\"
    --max-steps <N>       Model turns allowed (default: 40)
    --system <TEXT>       Override the system prompt
    --quiet               Do not report tool calls on stderr
    --version             Print the version line
    --help                Print this text

EXIT STATUS:
    0   the turn finished (a refused tool is a result, not a failure)
    1   configuration, provider or workspace failure
    130 cancelled with Ctrl-C
";

fn main() {
    let code = match run(std::env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("niki-rs: {error:#}");
            1
        }
    };
    std::process::exit(code);
}

fn run(args: Vec<String>) -> Result<i32> {
    let options = match parse_options(args)? {
        Parsed::Help => {
            print!("{USAGE}");
            return Ok(0);
        }
        Parsed::Version => {
            println!("niki-rs {}", env!("CARGO_PKG_VERSION"));
            return Ok(0);
        }
        Parsed::Run(options) => options,
    };

    let prompt = options
        .prompt
        .clone()
        .context("niki-rs needs a prompt; pass -n <TEXT> (see --help)")?;

    let config_path = options
        .config
        .clone()
        .unwrap_or_else(|| config_home().join("config.toml"));
    let config =
        Config::load(&config_path).with_context(|| format!("read {}", config_path.display()))?;

    let spec = options
        .model
        .clone()
        .or_else(|| config.default_model.clone())
        .with_context(|| {
            format!(
                "no model selected: set [models].default in {} or pass --model provider:model",
                config_path.display()
            )
        })?;
    let (provider_name, model) = spec.split_once(':').with_context(|| {
        format!("model spec {spec:?} is not provider:model (for example groq:llama-3.3-70b)")
    })?;
    let block = config.provider(provider_name).with_context(|| {
        format!(
            "unknown provider {provider_name:?}; this build resolves: {}",
            config.provider_names().join(", ")
        )
    })?;
    let provider: Box<dyn niki_provider::Provider> = match block.protocol() {
        Protocol::OpenAi => Box::new(
            OpenAiProvider::from_config(provider_name, &block, RetryPolicy::default())
                .context("build the provider")?,
        ),
        Protocol::Anthropic => Box::new(
            AnthropicProvider::from_config(provider_name, &block, RetryPolicy::default())
                .context("build the provider")?,
        ),
    };

    let root = options
        .root
        .clone()
        .map_or_else(std::env::current_dir, Ok)
        .context("resolve the workspace")?;
    let ctx = ToolContext::new(root.clone())
        .with_context(|| format!("open the workspace at {}", root.display()))?;

    let mode = mode(options.mode.as_deref());
    let allow = AllowList::parse(options.allow.as_deref().unwrap_or(""));
    let config_settings = AgentConfig {
        model: model.trim().to_owned(),
        system: options
            .system
            .clone()
            .unwrap_or_else(|| AgentConfig::default().system),
        max_steps: options.max_steps.unwrap_or(40),
        ..AgentConfig::default()
    };

    let cancel = CancelHandle::new();
    let mut agent = Agent::new(provider, ctx, config_settings)
        .with_policy(mode, allow)
        .with_cancel(cancel.clone());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .context("start the async runtime")?;

    let quiet = options.quiet;
    let result: Result<i32> = runtime.block_on(async move {
        let signal = cancel.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                signal.cancel();
            }
        });

        let mut out = std::io::stdout().lock();
        let outcome = agent
            .run(prompt, &mut |event| report(event, quiet, &mut out))
            .await
            .context("run the turn")?;

        let _ = writeln!(out);
        let _ = out.flush();

        if quiet {
            return Ok(0);
        }
        let cancelled = matches!(&outcome.stop, niki_provider::StopReason::Other(reason) if reason == "cancelled");
        eprintln!(
            "\n{} steps, {} tool calls, {} in / {} out tokens",
            outcome.steps,
            outcome.tool_calls,
            outcome.usage.input_tokens,
            outcome.usage.output_tokens,
        );
        Ok(i32::from(cancelled) * 130)
    });
    result
}

#[derive(Default)]
struct Options {
    prompt: Option<String>,
    model: Option<String>,
    config: Option<PathBuf>,
    root: Option<PathBuf>,
    mode: Option<String>,
    allow: Option<String>,
    max_steps: Option<u32>,
    system: Option<String>,
    quiet: bool,
}

enum Parsed {
    Help,
    Version,
    Run(Options),
}

fn parse_options(args: Vec<String>) -> Result<Parsed> {
    let mut options = Options::default();
    let mut iter = args.into_iter();
    while let Some(argument) = iter.next() {
        match argument.as_str() {
            "-n" | "--prompt" => options.prompt = Some(value(&argument, iter.next())?),
            "--model" => options.model = Some(value(&argument, iter.next())?),
            "--config" => options.config = Some(PathBuf::from(value(&argument, iter.next())?)),
            "--root" => options.root = Some(PathBuf::from(value(&argument, iter.next())?)),
            "--mode" => options.mode = Some(value(&argument, iter.next())?),
            "--allow" => options.allow = Some(value(&argument, iter.next())?),
            "--max-steps" => {
                let raw = value(&argument, iter.next())?;
                options.max_steps = Some(
                    raw.parse()
                        .with_context(|| format!("--max-steps needs a number, got {raw:?}"))?,
                );
            }
            "--system" => options.system = Some(value(&argument, iter.next())?),
            "--quiet" | "-q" => options.quiet = true,
            "--help" | "-h" => return Ok(Parsed::Help),
            "--version" | "-V" => return Ok(Parsed::Version),
            other if other.starts_with('-') && other.len() > 1 => {
                bail!("unknown option {other}; run niki-rs --help")
            }
            positional => bail!("unexpected argument {positional:?}; the prompt is passed with -n"),
        }
    }
    Ok(Parsed::Run(options))
}

fn value(flag: &str, value: Option<String>) -> Result<String> {
    value.with_context(|| format!("{flag} needs a value"))
}

fn mode(raw: Option<&str>) -> Mode {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("auto") => Mode::Auto,
        Some("yolo") => Mode::Yolo,
        Some("manual" | "") | None => Mode::Manual,
        Some(other) => {
            eprintln!("niki-rs: unknown mode {other:?}; using manual (manual, auto or yolo)");
            Mode::Manual
        }
    }
}

/// Print one event. Answer text goes to stdout so `niki-rs -n … > out.txt`
/// captures the answer; tool activity goes to stderr so a pipe stays clean.
fn report(event: &AgentEvent, quiet: bool, out: &mut impl Write) {
    match event {
        AgentEvent::Text(delta) => {
            let _ = out.write_all(delta.as_bytes());
            let _ = out.flush();
        }
        AgentEvent::ToolStart { name, arguments } if !quiet => {
            let _ = out.flush();
            eprintln!("· {name} {}", one_line(arguments));
        }
        AgentEvent::ToolEnd {
            name,
            output,
            is_error,
        } if !quiet => {
            let mark = if *is_error { '✗' } else { '✓' };
            eprintln!("  {mark} {name} {}", one_line(output));
        }
        AgentEvent::StepLimit { steps } if !quiet => {
            eprintln!("niki-rs: step limit of {steps} reached");
        }
        AgentEvent::Cancelled if !quiet => {
            eprintln!("niki-rs: cancelled");
        }
        _ => {}
    }
}

/// Shorten a multi-line argument or result to one readable line.
fn one_line(text: &str) -> String {
    let mut line: String = text.lines().next().unwrap_or("").trim().to_owned();
    if line.chars().count() > 120 {
        line = line.chars().take(117).collect::<String>() + "...";
    }
    line
}
