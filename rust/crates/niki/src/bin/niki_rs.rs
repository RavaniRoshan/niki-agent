//! `niki-rs` — the Rust entry point.
//!
//! With `-n` it runs one prompt headlessly, reading the same `config.toml` the
//! Python product reads. Without a prompt it opens the full-screen app on the
//! same agent, provider and workspace. Named `niki-rs` until cutover
//! (RULES.md 2).

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use niki_agent::{Agent, AgentConfig, AgentEvent, AllowList, CancelHandle, Mode};
use niki_config::{Config, Protocol, config_home};
use niki_hooks::{HookRunner, Hooks, Loaded};
use niki_mcp::ToolBridge;
use niki_provider::{AnthropicProvider, OpenAiProvider, Provider, RetryPolicy};
use niki_session::SessionStore;
use niki_skills::SkillCatalog;
use niki_tools::{Registry, ToolContext};
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
    // Canonical, like the root the tools resolved: the path a skill is read
    // from must pass the same boundary check that `read_file` applies.
    let skills = skill_catalog(&ctx.root);
    for warning in skills.warnings() {
        // stderr, not stdout: the answer a pipe captures stays the answer.
        eprintln!("niki-rs: warning: {warning}");
    }

    let mode = mode(options.mode.as_deref());
    let allow = AllowList::parse(options.allow.as_deref().unwrap_or(""));
    let settings = agent_config(&options, &spec, &skills)?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .context("start the async runtime")?;

    let (mcp_tools, mcp_errors) = runtime.block_on(niki_mcp::load(&config.mcp_servers));
    for error in &mcp_errors {
        eprintln!("niki-rs: warning: {error}");
    }
    let hooks = load_hooks(&root, &config_home());

    let cancel = CancelHandle::new();
    let mut agent = Agent::new(provider, ctx, settings)
        .with_registry(tool_registry(&mcp_tools))
        .with_policy(mode, allow)
        .with_hooks(hooks)
        .with_cancel(cancel.clone());

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
    let skills = skill_catalog(&ctx.root);
    // The model cannot use a skill it was never told about, so `--system`
    // replaces the base text and the skills section is appended to it.
    let system = system_prompt(&options, &skills);

    let mut app = App::new(SessionInfo {
        model: spec.clone(),
        mode,
        cwd: root,
    });
    app.set_skills(skills);
    app.set_palette(palette_or_default(config.theme.as_deref()));
    app.set_models(ModelPicker::from_config(&config));
    app.set_glyphs(GlyphMode::from_value(env_value("NIKI_ASCII").as_deref()));
    app.set_motion(Motion::from_value(
        env_value("NIKI_REDUCED_MOTION").as_deref(),
    ));
    attach_sessions(&mut app);
    let hooks = install_hooks(Hooks::load(&ctx.root, &config_home()), &mut app);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("start the async runtime")?;
    let mcp_tools = Arc::new(load_mcp(&runtime, &config, &mut app));
    let tool_names = {
        let registry = tool_registry(&mcp_tools);
        registry
            .names()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    app.set_tools(tool_names);

    let build_config = config;
    let mut worker = WorkerHandle::spawn_with_runtime(&spec, mode, runtime, move |parts| {
        let (provider_name, model) =
            split_spec(&parts.model).map_err(|error| format!("{error:#}"))?;
        let provider =
            build_provider(&build_config, provider_name).map_err(|error| format!("{error:#}"))?;
        let settings = AgentConfig {
            model: model.to_owned(),
            system: system.clone(),
            max_steps,
            ..AgentConfig::default()
        };
        Ok(Agent::new(provider, ctx.clone(), settings)
            .with_registry(tool_registry(&mcp_tools))
            .with_policy(parts.mode, allow.clone())
            .with_hooks(Arc::clone(&hooks))
            .with_cancel(parts.cancel)
            .with_approver(parts.approver))
    });

    run_screen(&mut app, &mut worker).context("run the full-screen app")
}

/// Open the session store and give the screen a session to write into.
///
/// A store that cannot be opened is a warning, not a failed startup: the screen
/// still works, it just cannot save or resume.
fn attach_sessions(app: &mut App) {
    let home = config_home();
    match open_store(&home) {
        Ok(store) => app.attach_sessions(store),
        Err(error) => app
            .transcript
            .push_notice(format!("warning: sessions are not saved: {error:#}")),
    }
}

/// Open `sessions.db` under `home`, creating the directory when it is missing.
fn open_store(home: &Path) -> Result<SessionStore> {
    std::fs::create_dir_all(home).with_context(|| format!("create {}", home.display()))?;
    SessionStore::open(home.join("sessions.db"))
        .with_context(|| format!("open the session store in {}", home.display()))
}

/// Connect every configured MCP server before the screen takes the terminal.
///
/// A server that fails is a warning line in the transcript, not a failed
/// startup: the session keeps the tools of every server that did answer. The
/// runtime is handed to the worker afterwards, because the connections belong
/// to it.
fn load_mcp(runtime: &tokio::runtime::Runtime, config: &Config, app: &mut App) -> Vec<ToolBridge> {
    let (tools, errors) = runtime.block_on(niki_mcp::load(&config.mcp_servers));
    for error in &errors {
        app.transcript.push_notice(format!("warning: {error}"));
    }
    tools
}

/// The built-in tools plus every tool discovered from an MCP server.
fn tool_registry(tools: &[ToolBridge]) -> Registry {
    let mut registry = Registry::new();
    for tool in tools {
        registry.register(tool.clone());
    }
    registry
}

/// Load hooks from the workspace and the user configuration directory for the
/// headless run, where a problem is one warning line on stderr.
fn load_hooks(root: &Path, config_dir: &Path) -> Arc<dyn HookRunner> {
    let loaded = Hooks::load(root, config_dir);
    for warning in &loaded.warnings {
        eprintln!("niki-rs: warning: {warning}");
    }
    Arc::new(loaded.hooks)
}

/// Install hooks in the full-screen app. A broken source is one warning row in
/// the transcript, added before the first frame and not repeated per turn.
fn install_hooks(loaded: Loaded, app: &mut App) -> Arc<dyn HookRunner> {
    for warning in &loaded.warnings {
        app.transcript.push_notice(format!("warning: {warning}"));
    }
    Arc::new(loaded.hooks)
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

/// Discover the project skills, the user's skills and the built-ins.
///
/// The user directory follows `NIKI_HOME`, so a profile selected for this run is
/// the profile whose skills are used.
fn skill_catalog(root: &Path) -> SkillCatalog {
    SkillCatalog::discover(root, &config_home().join("skills"))
}

fn agent_config(options: &Options, spec: &str, skills: &SkillCatalog) -> Result<AgentConfig> {
    let (_, model) = split_spec(spec)?;
    Ok(AgentConfig {
        model: model.trim().to_owned(),
        system: system_prompt(options, skills),
        max_steps: options.max_steps.unwrap_or(40),
        ..AgentConfig::default()
    })
}

/// The system prompt one turn is sent with: the base text plus the skills.
fn system_prompt(options: &Options, skills: &SkillCatalog) -> String {
    let base = options
        .system
        .clone()
        .unwrap_or_else(|| AgentConfig::default().system);
    skills.inject_into(&base)
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
        AgentEvent::Notice(text) => {
            let _ = out.flush();
            eprintln!("niki-rs: {text}");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_setting_selects_the_palette() {
        let light = Config::from_str("[ui]\ntheme = \"light\"\n").expect("config");
        assert_eq!(
            palette_or_default(light.theme.as_deref()).name,
            "high-contrast"
        );
        let colourblind = Config::from_str("[ui]\ntheme = \"colour-blind\"\n").expect("config");
        assert_eq!(
            palette_or_default(colourblind.theme.as_deref()).name,
            "colorblind"
        );
        let unknown = Config::from_str("[ui]\ntheme = \"purple\"\n").expect("config");
        assert_eq!(palette_or_default(unknown.theme.as_deref()).name, "niki");
        assert_eq!(palette_or_default(None).name, "niki");
    }

    #[test]
    fn the_glyph_and_motion_flags_read_the_environment_values() {
        assert_eq!(GlyphMode::from_value(Some("1")), GlyphMode::Ascii);
        assert_eq!(GlyphMode::from_value(None), GlyphMode::Unicode);
        assert_eq!(Motion::from_value(Some("1")), Motion::Reduced);
        assert_eq!(Motion::from_value(None), Motion::Full);
    }

    #[test]
    fn a_model_spec_splits_into_provider_and_model() {
        let (provider, model) = split_spec("groq:llama-3.3-70b").expect("spec");
        assert_eq!(provider, "groq");
        assert_eq!(model, "llama-3.3-70b");
        assert!(split_spec("llama").is_err());
    }

    #[test]
    fn discovered_skills_reach_the_system_prompt() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let directory = project.path().join(".niki/skills/deploy");
        std::fs::create_dir_all(&directory).expect("skill directory");
        std::fs::write(
            directory.join("SKILL.md"),
            "---\nname: deploy\ndescription: Ship a release safely.\n---\nbody\n",
        )
        .expect("skill");

        let skills = SkillCatalog::discover(project.path(), user.path());
        let settings =
            agent_config(&Options::default(), "mock:mock-model", &skills).expect("config");
        assert!(settings.system.contains("## Skills"), "{}", settings.system);
        assert!(settings.system.contains("deploy"));
        assert!(settings.system.contains("Ship a release safely."));
        assert!(settings.system.contains("deploy/SKILL.md"));

        let custom = Options {
            system: Some("custom base".to_owned()),
            ..Options::default()
        };
        let settings = agent_config(&custom, "mock:mock-model", &skills).expect("config");
        assert!(
            settings.system.starts_with("custom base"),
            "{}",
            settings.system
        );
        assert!(settings.system.contains("deploy/SKILL.md"));
    }

    #[test]
    fn the_session_store_is_created_under_the_config_home() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join(".niki");
        let store = open_store(&home).expect("store");
        let session = store.create(temp.path()).expect("session");
        assert_eq!(store.list().expect("list").len(), 1);
        assert!(store.load(&session).expect("load").is_empty());
        assert!(home.join("sessions.db").exists());
    }

    #[test]
    fn a_broken_mcp_server_is_a_warning_and_the_session_still_starts() {
        use niki_tui_live::{RowKind, TurnState};

        let config = Config::from_str(
            r#"
[mcp_servers.also_broken]
command = "/nonexistent/niki-mcp-startup-2"

[mcp_servers.broken]
command = "/nonexistent/niki-mcp-startup"
"#,
        )
        .expect("config");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let mut app = App::new(SessionInfo {
            model: "mock:mock-model".to_owned(),
            mode: Mode::Manual,
            cwd: PathBuf::from("/work"),
        });

        let tools = load_mcp(&runtime, &config, &mut app);

        assert!(tools.is_empty(), "neither server can start");
        assert_eq!(app.status.turn, TurnState::Idle);
        let notices: Vec<&str> = app
            .transcript
            .rows()
            .iter()
            .filter(|row| row.kind == RowKind::Notice)
            .map(|row| row.text.as_str())
            .collect();
        assert_eq!(notices.len(), 2, "{notices:?}");
        assert!(notices[0].contains("also_broken"), "{}", notices[0]);
        assert!(notices[1].contains("broken"), "{}", notices[1]);
        assert!(
            notices.iter().all(|notice| notice.starts_with("warning: ")),
            "{notices:?}"
        );
    }

    #[test]
    fn a_broken_hooks_file_is_one_warning_row_and_the_session_starts() {
        use niki_tui_live::{RowKind, TurnState};

        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".niki")).expect("mkdir");
        std::fs::write(dir.path().join(".niki/hooks.json"), "{ not json").expect("write");
        let mut app = App::new(SessionInfo {
            model: "mock:mock-model".to_owned(),
            mode: Mode::Manual,
            cwd: dir.path().to_path_buf(),
        });

        let loaded = Hooks::load(dir.path(), &dir.path().join("home"));
        let _hooks = install_hooks(loaded, &mut app);

        assert_eq!(app.status.turn, TurnState::Idle);
        let notices: Vec<&str> = app
            .transcript
            .rows()
            .iter()
            .filter(|row| row.kind == RowKind::Notice)
            .map(|row| row.text.as_str())
            .collect();
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(notices[0].starts_with("warning: "), "{}", notices[0]);
        assert!(notices[0].contains("hooks.json"), "{}", notices[0]);
    }
}
