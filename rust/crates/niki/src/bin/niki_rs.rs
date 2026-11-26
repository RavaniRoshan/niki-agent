//! `niki-rs` — the Rust entry point.
//!
//! With `-n` it runs one prompt headlessly, reading the same `config.toml` the
//! Python product reads. Without a prompt it opens the full-screen app on the
//! same agent, provider and workspace. Named `niki-rs` until cutover
//! (RULES.md 2).

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use niki_agent::{Agent, AgentConfig, AgentEvent, AllowList, CancelHandle, Mode};
use niki_config::{Config, Protocol, config_home};
use niki_provider::{AnthropicProvider, OpenAiProvider, Provider, RetryPolicy};
use niki_tools::ToolContext;
use niki_tui_live::{App, ModelPicker, SessionInfo, WorkerHandle, run as run_screen};
use niki_tui_themes::{GlyphMode, Motion, palette_or_default};

const USAGE: &str = "\
niki-rs — Niki agent (Rust)

USAGE:
    niki-rs -n <PROMPT> [options]   run one prompt and exit
    niki-rs [options]              open the full-screen app

OPTIONS:
    -n, --prompt <TEXT>   Prompt to run headlessly; without it the app opens
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

ENVIRONMENT:
    NIKI_ASCII=1          Draw the app with ASCII characters only
    NIKI_REDUCED_MOTION=1 Do not animate the spinner

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

    let config_path = options
        .config
        .clone()
        .unwrap_or_else(|| config_home().join("config.toml"));
    let config =
        Config::load(&config_path).with_context(|| format!("read {}", config_path.display()))?;

    match options.prompt.clone() {
        Some(prompt) => headless(options, &config, &config_path, prompt),
        None => screen(options, config, config_path),
    }
}

/// One turn, answering on stdout. This is the W1 entry point and stays as it
/// was: a script that ran `niki-rs -n …` before still does.
fn headless(options: Options, config: &Config, config_path: &Path, prompt: String) -> Result<i32> {
    let spec = model_spec(&options, config, config_path)?;
    let (provider_name, _) = split_spec(&spec)?;
    let provider = build_provider(config, provider_name)?;

    let root = workspace_root(&options)?;
    let ctx = ToolContext::new(root.clone())
        .with_context(|| format!("open the workspace at {}", root.display()))?;

    let mode = mode(options.mode.as_deref());
    let allow = AllowList::parse(options.allow.as_deref().unwrap_or(""));
    let settings = agent_config(&options, &spec)?;

    let cancel = CancelHandle::new();
    let mut agent = Agent::new(provider, ctx, settings)
        .with_policy(mode, allow)
        .with_cancel(cancel.clone());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .context("start the async runtime")?;

    let quiet = options.quiet;
    runtime.block_on(async move {
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
    })
}

/// The full-screen app. The agent is built on the worker thread, once per
/// model, so switching model mid-session needs no restart.
fn screen(options: Options, config: Config, config_path: PathBuf) -> Result<i32> {
    if !std::io::stdout().is_terminal() {
        bail!(
            "the full-screen app needs a terminal; pass -n <PROMPT> to run one prompt headlessly"
        );
    }
    let spec = model_spec(&options, &config, &config_path)?;
    let root = workspace_root(&options)?;
    let ctx = ToolContext::new(root.clone())
        .with_context(|| format!("open the workspace at {}", root.display()))?;
    let mode = mode(options.mode.as_deref());
    let allow = AllowList::parse(options.allow.as_deref().unwrap_or(""));
    let max_steps = options.max_steps.unwrap_or(40);
    let system = options
        .system
        .clone()
        .unwrap_or_else(|| AgentConfig::default().system);

    let mut app = App::new(SessionInfo {
        model: spec.clone(),
        mode,
        cwd: root,
    });
    app.set_palette(palette_or_default(config.theme.as_deref()));
    app.set_models(ModelPicker::from_config(&config));
    app.set_glyphs(GlyphMode::from_value(env_value("NIKI_ASCII").as_deref()));
    app.set_motion(Motion::from_value(
        env_value("NIKI_REDUCED_MOTION").as_deref(),
    ));

    let build_config = config;
    let mut worker = WorkerHandle::spawn_with(&spec, mode, move |parts| {
        let (provider_name, model) =
            split_spec(&parts.model).map_err(|error| format!("{error:#}"))?;
        let provider = build_provider(&build_config, provider_name)
            .map_err(|error| format!("{error:#}"))?;
        let settings = AgentConfig {
            model: model.to_owned(),
            system: system.clone(),
            max_steps,
            ..AgentConfig::default()
        };
        Ok(Agent::new(provider, ctx.clone(), settings)
            .with_policy(parts.mode, allow.clone())
            .with_cancel(parts.cancel)
            .with_approver(parts.approver))
    });

    run_screen(&mut app, &mut worker).context("run the full-screen app")
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

/// The `provider:model` spec to run, from the flag or from the config.
fn model_spec(options: &Options, config: &Config, config_path: &Path) -> Result<String> {
    options
        .model
        .clone()
        .or_else(|| config.default_model.clone())
        .with_context(|| {
            format!(
                "no model selected: set [models].default in {} or pass --model provider:model",
                config_path.display()
            )
        })
}

fn split_spec(spec: &str) -> Result<(&str, &str)> {
    spec.split_once(':').with_context(|| {
        format!("model spec {spec:?} is not provider:model (for example groq:llama-3.3-70b)")
    })
}

fn build_provider(config: &Config, provider_name: &str) -> Result<Box<dyn Provider>> {
    let block = config.provider(provider_name).with_context(|| {
        format!(
            "unknown provider {provider_name:?}; this build resolves: {}",
            config.provider_names().join(", ")
        )
    })?;
    match block.protocol() {
        Protocol::OpenAi => Ok(Box::new(
            OpenAiProvider::from_config(provider_name, &block, RetryPolicy::default())
                .context("build the provider")?,
        )),
        Protocol::Anthropic => Ok(Box::new(
            AnthropicProvider::from_config(provider_name, &block, RetryPolicy::default())
                .context("build the provider")?,
        )),
    }
}

fn workspace_root(options: &Options) -> Result<PathBuf> {
    options
        .root
        .clone()
        .map_or_else(std::env::current_dir, Ok)
        .context("resolve the workspace")
}

fn agent_config(options: &Options, spec: &str) -> Result<AgentConfig> {
    let (_, model) = split_spec(spec)?;
    Ok(AgentConfig {
        model: model.trim().to_owned(),
        system: options
            .system
            .clone()
            .unwrap_or_else(|| AgentConfig::default().system),
        max_steps: options.max_steps.unwrap_or(40),
        ..AgentConfig::default()
    })
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

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok()
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
