//! `niki` — the Rust entry point.
//!
//! With `-n` it runs one prompt headlessly, reading the same `config.toml` the
//! Python product reads. With `--acp` it serves an editor over stdin/stdout on
//! that same agent, provider and workspace. Without either it opens the
//! full-screen app. Named `niki` until cutover (RULES.md 2).

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use niki_agent::{Agent, AgentConfig, AgentEvent, AllowList, CancelHandle, Mode, RunOutcome};
use niki_config::instructions::Instructions;
use niki_config::{Config, Protocol, config_home};
use niki_hooks::{HookRunner, Hooks, Loaded, Notify};
use niki_mcp::ToolBridge;
use niki_provider::{
    AnthropicProvider, Message, OpenAiProvider, Provider, RetryPolicy, StopReason,
};
use niki_render::DiffTheme;
use niki_session::{SessionId, SessionStore};
use niki_skills::SkillCatalog;
use niki_subagents::{SubagentTool, TOOL_NAME};
use niki_tools::{Registry, ToolContext};
use niki_tui_live::{App, ModelPicker, SessionInfo, WorkerHandle, diff_source, run as run_screen};
use niki_tui_themes::{GlyphMode, Motion, palette_or_default};
use serde_json::json;

const USAGE: &str = "\
niki — Niki agent (Rust)

USAGE:
    niki -n <PROMPT> [options]   run one prompt and exit
    niki --acp [options]         serve an editor over stdin/stdout
    niki [options]              open the full-screen app

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
    --output-format <text|json|jsonl>
                          text streams the answer (default); json prints one
                          object with the answer, steps, tool calls, usage, the
                          session totals and a context audit; jsonl writes one
                          JSON object per event, one per line
    --output <FORMAT>     Alias for --output-format
    --resume <ID|last>    Continue a saved session by id, or the newest one
    --continue            Continue the newest session for this workspace
    --version             Print the version line
    --help                Print this text

ENVIRONMENT:
    NIKI_ASCII=1          Draw the app with ASCII characters only
    NIKI_REDUCED_MOTION=1 Do not animate the spinner

EXIT STATUS:
    0   the turn finished (a refused tool is a result, not a failure)
    1   configuration, provider or workspace failure
    2   usage error: --acp cannot be combined with -n
    124 the step budget ran out before the model stopped calling tools
    130 cancelled with Ctrl-C
";

fn main() {
    let code = match run(std::env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("niki: {}", sanitize_terminal(&format!("{error:#}")));
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
            println!("niki {}", env!("CARGO_PKG_VERSION"));
            return Ok(0);
        }
        Parsed::Run(options) => *options,
    };

    if options.acp && options.prompt.is_some() {
        eprintln!(
            "niki: --acp and -n cannot be combined; --acp serves an editor on stdin/stdout \
             and -n runs one prompt"
        );
        return Ok(2);
    }

    if options.prompt.as_deref().map(str::trim) == Some("/diff") {
        return headless_diff(&options);
    }

    let config_path = options
        .config
        .clone()
        .unwrap_or_else(|| config_home().join("config.toml"));
    let config =
        Config::load(&config_path).with_context(|| format!("read {}", config_path.display()))?;

    if let Some(prompt) = options.prompt.clone() {
        headless(options, &config, &config_path, prompt)
    } else if options.acp {
        acp(options, config, config_path)
    } else {
        screen(options, config, config_path)
    }
}

/// One turn, answering on stdout. This is the W1 entry point and stays as it
/// was: a script that ran `niki -n …` before still does.
fn headless(options: Options, config: &Config, config_path: &Path, prompt: String) -> Result<i32> {
    if prompt.split_whitespace().next() == Some("/init") {
        return init(&options);
    }
    if prompt.split_whitespace().next() == Some("/rename") {
        return rename(&options, remainder(&prompt).as_deref());
    }
    if prompt.split_whitespace().next() == Some("/export") {
        return export(&options, remainder(&prompt).as_deref());
    }

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
        eprintln!("niki: warning: {}", sanitize_terminal(&warning.to_string()));
    }

    let instructions = instructions_for(&ctx.root);
    if !instructions.is_empty() {
        eprintln!(
            "niki: {} instruction file(s) loaded ({} bytes)",
            instructions.len(),
            instructions.bytes(),
        );
    }

    let mode = mode(options.mode.as_deref());
    let allow = AllowList::parse(options.allow.as_deref().unwrap_or(""));
    let settings = agent_config(&options, &spec, &skills, &instructions)?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .context("start the async runtime")?;

    let (mcp_tools, mcp_errors) = runtime.block_on(niki_mcp::load(&config.mcp_servers));
    for error in &mcp_errors {
        eprintln!("niki: warning: {}", sanitize_terminal(&error.to_string()));
    }
    let mcp_tools = Arc::new(mcp_tools);
    let hooks = load_hooks(&root, &config_home());

    let Resumed {
        store,
        session,
        history,
    } = open_session(&options, &root)?;

    let cancel = CancelHandle::new();
    let delegation = Delegation {
        config: config.clone(),
        spec: spec.clone(),
        system: settings.system.clone(),
        mode,
        allow: allow.clone(),
        hooks: Arc::clone(&hooks),
        cancel: cancel.clone(),
    };
    let subagent = delegation.tool(&ctx, Arc::clone(&mcp_tools));
    let notify = notifier(config);
    let mut agent = Agent::new(provider, ctx, settings)
        .with_registry(tool_registry(mcp_tools.as_slice(), Some(subagent)))
        .with_policy(mode, allow)
        .with_hooks(hooks)
        .with_cancel(cancel.clone())
        .with_history(history);
    if let Some(notify) = &notify {
        agent = agent.with_notify(Arc::clone(notify), spec.clone());
    }
    if let Some(session) = &session {
        agent = agent.with_session_id(session.as_str());
    }

    let quiet = options.quiet;
    let json = options.output == OutputFormat::Json;
    let jsonl = options.output == OutputFormat::Jsonl;
    let mut stream = JsonlStream::new(&spec, session.as_ref().map(SessionId::as_str), &root);
    let saved_prompt = prompt.clone();
    let code = runtime.block_on(async move {
        let signal = cancel.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                signal.cancel();
            }
        });

        let mut out = std::io::stdout().lock();
        let mut rows: Vec<(String, String)> = Vec::new();
        let mut buffered = String::new();
        if jsonl {
            stream.start(&mut out);
        }
        let result = agent
            .run(prompt, &mut |event| {
                match event {
                    AgentEvent::Text(delta) if json => buffered.push_str(delta),
                    AgentEvent::ToolStart { name, arguments } => {
                        rows.push((
                            "tool_call".to_owned(),
                            format!("{name} {}", one_line(arguments)),
                        ));
                    }
                    AgentEvent::ToolEnd { output, .. } => {
                        rows.push(("tool_result".to_owned(), output.clone()));
                    }
                    _ => {}
                }
                if jsonl {
                    stream.event(event, &mut out);
                    if !matches!(event, AgentEvent::Text(_)) {
                        report(event, quiet, &mut out);
                    }
                } else if !(json && matches!(event, AgentEvent::Text(_))) {
                    report(event, quiet, &mut out);
                }
            })
            .await;

        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                if jsonl {
                    stream.error(&mut out, &format!("{error:#}"));
                    stream.end(
                        &mut out,
                        "error",
                        stream.turns,
                        stream.results,
                        stream.input,
                        stream.output,
                    );
                }
                return Err(error).context("run the turn");
            }
        };

        if jsonl {
            stream.end(
                &mut out,
                stop_label(&outcome.stop),
                outcome.steps,
                outcome.tool_calls,
                outcome.usage.input_tokens,
                outcome.usage.output_tokens,
            );
        } else if json {
            let session_usage = agent.session_usage();
            let audit = agent.context_audit();
            let value = json!({
                "answer": outcome.text,
                "steps": outcome.steps,
                "tool_calls": outcome.tool_calls,
                "usage": {
                    "input_tokens": outcome.usage.input_tokens,
                    "output_tokens": outcome.usage.output_tokens,
                },
                "session_usage": {
                    "input_tokens": session_usage.input_tokens,
                    "output_tokens": session_usage.output_tokens,
                    "steps": session_usage.steps,
                },
                "context_audit": {
                    "system": audit.system_tokens,
                    "tool_schemas": audit.tool_schema_tokens,
                    "messages": audit.message_tokens,
                    "tools": audit.tool_count,
                    "total": audit.total_tokens(),
                },
                "stop": stop_label(&outcome.stop),
                "session": session.as_ref().map(SessionId::as_str),
            });
            let _ = writeln!(out, "{value}");
        } else {
            let _ = writeln!(out);
        }
        let _ = out.flush();

        if let (Some(store), Some(session)) = (&store, &session) {
            if let Err(error) = record(store, session, &saved_prompt, &outcome, &rows) {
                eprintln!(
                    "niki: warning: the session was not saved: {}",
                    sanitize_terminal(&format!("{error:#}"))
                );
            }
        }

        if !quiet {
            eprintln!(
                "\n{} steps, {} tool calls, {} in / {} out tokens",
                outcome.steps,
                outcome.tool_calls,
                outcome.usage.input_tokens,
                outcome.usage.output_tokens,
            );
        }
        Ok::<i32, anyhow::Error>(exit_code(&outcome))
    })?;
    // A failing notifier is warned about from a background thread; give a fast
    // one the moment it needs to reach stderr before the process closes it. A
    // slow program is left behind rather than delaying the exit.
    if let Some(notify) = &notify {
        let _ = notify.wait_for(std::time::Duration::from_millis(250));
    }
    Ok(code)
}

/// `niki -n /diff` prints the working tree's diff instead of running a turn.
fn headless_diff(options: &Options) -> Result<i32> {
    let root = workspace_root(options)?;
    let views = diff_source::collect(
        &root,
        &diff_source::DiffSection::ALL,
        diff_source::RENDER_WIDTH,
        &DiffTheme::default(),
    )?;
    let text = format!("{}\n", diff_source::plain_text(&views));
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => Ok(0),
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(0),
        Err(error) => Err(error.into()),
    }
}

/// The saved conversation a headless run continues.
struct Resumed {
    store: Option<SessionStore>,
    session: Option<SessionId>,
    history: Vec<Message>,
}

/// Open the session store and pick the conversation this run appends to.
///
/// Without `--resume` or `--continue` a new session is created. With either,
/// the named session (or the newest one for this workspace) supplies its user
/// and assistant turns as history; tool rows stay in the store but are not
/// resent, because a stored row carries no tool-call id to answer.
fn open_session(options: &Options, root: &Path) -> Result<Resumed> {
    let home = config_home();
    let store = match open_store(&home) {
        Ok(store) => store,
        Err(error) => {
            if options.resume.is_some() || options.continue_session {
                return Err(error).context("resume needs the session store");
            }
            eprintln!(
                "niki: warning: sessions are not saved: {}",
                sanitize_terminal(&format!("{error:#}"))
            );
            return Ok(Resumed {
                store: None,
                session: None,
                history: Vec::new(),
            });
        }
    };

    let wanted = if options.continue_session {
        Some("last".to_owned())
    } else {
        options.resume.clone()
    };
    let Some(wanted) = wanted else {
        let session = store.create(root).context("create a session")?;
        return Ok(Resumed {
            store: Some(store),
            session: Some(session),
            history: Vec::new(),
        });
    };

    let summary = if wanted == "last" {
        store.latest_for(root).context("list the saved sessions")?
    } else {
        store.find(&wanted).context("look up the saved session")?
    };
    let summary = summary.with_context(|| {
        format!(
            "no saved session {wanted:?} for {}; run without --resume to start one",
            root.display()
        )
    })?;
    let rows = store.load(&summary.id).context("read the saved session")?;
    let history = rows.iter().filter_map(model_message).collect();
    Ok(Resumed {
        store: Some(store),
        session: Some(summary.id),
        history,
    })
}

/// The model message a stored row replays as, when it replays as one.
fn model_message(row: &niki_session::Message) -> Option<Message> {
    match row.role.as_str() {
        "user" => Some(Message::user(row.content.clone())),
        "assistant" => Some(Message::assistant(row.content.clone())),
        _ => None,
    }
}

/// Append the completed turn: the prompt, the tool rows, then the answer.
fn record(
    store: &SessionStore,
    session: &SessionId,
    prompt: &str,
    outcome: &RunOutcome,
    rows: &[(String, String)],
) -> Result<()> {
    store
        .append(session, "user", prompt)
        .context("save the prompt")?;
    for (role, content) in rows {
        store
            .append(session, role, content)
            .context("save a tool row")?;
    }
    if !outcome.text.is_empty() {
        store
            .append(session, "assistant", &outcome.text)
            .context("save the answer")?;
    }
    Ok(())
}

/// Why the turn ended, in the JSON output's vocabulary.
fn stop_label(stop: &StopReason) -> &'static str {
    match stop {
        StopReason::EndTurn => "end_turn",
        StopReason::ToolUse => "tool_use",
        StopReason::MaxTokens => "max_tokens",
        StopReason::Other(reason) if reason == "cancelled" => "cancelled",
        StopReason::Other(reason) if reason.starts_with("step limit") => "step_limit",
        StopReason::Other(_) => "other",
    }
}

/// The process exit code for a finished turn.
fn exit_code(outcome: &RunOutcome) -> i32 {
    match &outcome.stop {
        StopReason::Other(reason) if reason == "cancelled" => 130,
        StopReason::Other(reason) if reason.starts_with("step limit") => 124,
        _ => 0,
    }
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
    // replaces the base text, the project instruction chain follows it and the
    // skills section is appended last. The closure recomposes the prompt, so
    // `/init` can hand the worker a fresh one without a restart.
    let base_system = options
        .system
        .clone()
        .unwrap_or_else(|| AgentConfig::default().system);
    let compose: Arc<dyn Fn() -> String + Send + Sync> = {
        let base = base_system;
        let skills = skills.clone();
        let root = ctx.root.clone();
        Arc::new(move || {
            let instructions = instructions_for(&root);
            skills.inject_into(&instructions.inject_into(&base))
        })
    };

    let mut app = App::new(SessionInfo {
        model: spec.clone(),
        mode,
        cwd: root,
    });
    app.set_allow_list(allow.clone());
    app.set_approver(true);
    app.set_skills(skills);
    app.set_palette(palette_or_default(config.theme.as_deref()));
    app.set_models(ModelPicker::from_config(&config));
    app.set_glyphs(GlyphMode::from_value(env_value("NIKI_ASCII").as_deref()));
    app.set_motion(Motion::from_value(
        env_value("NIKI_REDUCED_MOTION").as_deref(),
    ));
    attach_sessions(&mut app);
    let session_id = app.session_id().map(str::to_owned);
    let hooks = install_hooks(Hooks::load(&ctx.root, &config_home()), &mut app);
    {
        let compose = Arc::clone(&compose);
        app.set_system_refresh(move || compose());
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("start the async runtime")?;
    let mcp_tools = Arc::new(load_mcp(&runtime, &config, &mut app));
    let tool_names = {
        let mut names = tool_registry(mcp_tools.as_slice(), None)
            .names()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        names.push(TOOL_NAME.to_owned());
        names.sort_unstable();
        names
    };
    app.set_tools(tool_names);

    let notify = notifier(&config);
    let build_config = config;
    let compact_config = build_config.clone();
    let build_system = Arc::clone(&compose);
    let provider_builder = move |spec: &str| -> Result<Box<dyn Provider>, String> {
        let (provider_name, _model) = split_spec(spec).map_err(|error| format!("{error:#}"))?;
        build_provider(&compact_config, provider_name).map_err(|error| format!("{error:#}"))
    };
    let mut worker = WorkerHandle::spawn_with_runtime_and_provider(
        &spec,
        mode,
        runtime,
        move |parts| {
            let (provider_name, model) =
                split_spec(&parts.model).map_err(|error| format!("{error:#}"))?;
            let provider = build_provider(&build_config, provider_name)
                .map_err(|error| format!("{error:#}"))?;
            let settings = AgentConfig {
                model: model.to_owned(),
                system: build_system(),
                max_steps,
                ..AgentConfig::default()
            };
            let delegation = Delegation {
                config: build_config.clone(),
                spec: parts.model.clone(),
                system: build_system(),
                mode: parts.mode,
                allow: allow.clone(),
                hooks: Arc::clone(&hooks),
                cancel: parts.cancel.clone(),
            };
            let subagent = delegation.tool(&ctx, Arc::clone(&mcp_tools));
            let mut agent = Agent::new(provider, ctx.clone(), settings)
                .with_registry(tool_registry(mcp_tools.as_slice(), Some(subagent)))
                .with_policy(parts.mode, allow.clone())
                .with_hooks(Arc::clone(&hooks))
                .with_cancel(parts.cancel)
                .with_approver(parts.approver);
            if let Some(notify) = &notify {
                agent = agent.with_notify(Arc::clone(notify), parts.model.clone());
            }
            if let Some(session_id) = &session_id {
                agent = agent.with_session_id(session_id.clone());
            }
            Ok(agent)
        },
        provider_builder,
    );

    run_screen(&mut app, &mut worker).context("run the full-screen app")
}

/// Serve the Agent Client Protocol on stdin/stdout.
///
/// The provider, tools, policy and workspace are the host's: the server builds
/// one agent per session through [`niki_acp::AgentFactory`] and owns the
/// protocol, the session store and the approval round-trip. Nothing but frames
/// may reach stdout while this runs; warnings go to stderr.
fn acp(options: Options, config: Config, config_path: PathBuf) -> Result<i32> {
    let spec = model_spec(&options, &config, &config_path)?;
    let default_mode = mode(options.mode.as_deref());
    let allow = AllowList::parse(options.allow.as_deref().unwrap_or(""));
    let max_steps = options.max_steps.unwrap_or(40);
    let base_system = options
        .system
        .clone()
        .unwrap_or_else(|| AgentConfig::default().system);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .context("start the async runtime")?;

    let (mcp_tools, mcp_errors) = runtime.block_on(niki_mcp::load(&config.mcp_servers));
    for error in &mcp_errors {
        eprintln!("niki: warning: {}", sanitize_terminal(&error.to_string()));
    }
    let mcp_tools = Arc::new(mcp_tools);

    let build_config = config;
    let notify = notifier(&build_config);
    let factory_tools = Arc::clone(&mcp_tools);
    let factory = Box::new(
        move |seed: niki_acp::SessionSeed| -> Result<Agent, String> {
            let (provider_name, model) = split_spec(&spec).map_err(|error| format!("{error:#}"))?;
            let provider = build_provider(&build_config, provider_name)
                .map_err(|error| format!("{error:#}"))?;
            let ctx = ToolContext::new(seed.cwd.as_path()).map_err(|error| error.to_string())?;
            let skills = skill_catalog(&ctx.root);
            for warning in skills.warnings() {
                eprintln!("niki: warning: {}", sanitize_terminal(&warning.to_string()));
            }
            let instructions = instructions_for(&ctx.root);
            let system = skills.inject_into(&instructions.inject_into(&base_system));
            let settings = AgentConfig {
                model: model.to_owned(),
                system: system.clone(),
                max_steps,
                ..AgentConfig::default()
            };
            let hooks = load_hooks(&seed.cwd, &config_home());
            let delegation = Delegation {
                config: build_config.clone(),
                spec: spec.clone(),
                system,
                mode: seed.mode,
                allow: allow.clone(),
                hooks: Arc::clone(&hooks),
                cancel: seed.cancel.clone(),
            };
            let subagent = delegation.tool(&ctx, Arc::clone(&factory_tools));
            let session_id = seed.session_id.clone();
            let mut agent = Agent::new(provider, ctx, settings)
                .with_registry(tool_registry(factory_tools.as_slice(), Some(subagent)))
                .with_policy(seed.mode, allow.clone())
                .with_hooks(hooks)
                .with_cancel(seed.cancel)
                .with_approver(seed.approver)
                .with_session_id(session_id);
            if let Some(notify) = &notify {
                agent = agent.with_notify(Arc::clone(notify), spec.clone());
            }
            Ok(agent)
        },
    );

    let options = niki_acp::ServerOptions::new()
        .with_store(config_home().join("sessions.db"))
        .with_default_mode(default_mode);
    runtime
        .block_on(niki_acp::serve_stdio(factory, options))
        .context("serve the editor protocol")?;
    Ok(0)
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
    create_private_dir(home).with_context(|| format!("create {}", home.display()))?;
    SessionStore::open(home.join("sessions.db"))
        .with_context(|| format!("open the session store in {}", home.display()))
}

/// Create a directory only the current user may read or write.
#[cfg(unix)]
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    // `DirBuilder` does not tighten a directory that already exists.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
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

/// What a `task` child inherits from the session that started it.
struct Delegation {
    config: Config,
    spec: String,
    system: String,
    mode: Mode,
    allow: AllowList,
    hooks: Arc<dyn HookRunner>,
    cancel: CancelHandle,
}

impl Delegation {
    /// Build the `task` tool. Each call builds a fresh provider, agent and
    /// registry, so a child never inherits the parent's conversation.
    fn tool(&self, ctx: &ToolContext, tools: Arc<Vec<ToolBridge>>) -> SubagentTool {
        let config = self.config.clone();
        let spec = self.spec.clone();
        let system = self.system.clone();
        let mode = self.mode;
        let allow = self.allow.clone();
        let hooks = Arc::clone(&self.hooks);
        let ctx = ctx.clone();
        SubagentTool::new(
            move || tool_registry(tools.as_slice(), None),
            move |registry, steps| {
                let (provider_name, model) =
                    split_spec(&spec).map_err(|error| format!("{error:#}"))?;
                let provider =
                    build_provider(&config, provider_name).map_err(|error| format!("{error:#}"))?;
                let settings = AgentConfig {
                    model: model.to_owned(),
                    system: system.clone(),
                    max_steps: steps,
                    ..AgentConfig::default()
                };
                Ok(Agent::new(provider, ctx.clone(), settings)
                    .with_registry(registry)
                    .with_policy(mode, allow.clone())
                    .with_hooks(Arc::clone(&hooks)))
            },
        )
        .with_cancel(self.cancel.clone())
    }
}

/// The built-in tools, every tool discovered from an MCP server, and the `task`
/// tool when the session delegates.
fn tool_registry(tools: &[ToolBridge], subagent: Option<SubagentTool>) -> Registry {
    let mut registry = Registry::new();
    for tool in tools {
        registry.register(tool.clone());
    }
    if let Some(subagent) = subagent {
        registry.register(subagent);
    }
    registry
}

/// The turn-end notifier, when `[notify].program` is configured.
///
/// An absent or blank program yields `None`, and nothing is spawned.
fn notifier(config: &Config) -> Option<Arc<Notify>> {
    Notify::from_program(config.notify.program.as_deref(), config.notify.args.clone()).map(Arc::new)
}

/// Load hooks from the workspace and the user configuration directory for the
/// headless run, where a problem is one warning line on stderr.
fn load_hooks(root: &Path, config_dir: &Path) -> Arc<dyn HookRunner> {
    let loaded = Hooks::load(root, config_dir);
    for warning in &loaded.warnings {
        eprintln!("niki: warning: {}", sanitize_terminal(warning));
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
    acp: bool,
    model: Option<String>,
    config: Option<PathBuf>,
    root: Option<PathBuf>,
    mode: Option<String>,
    allow: Option<String>,
    max_steps: Option<u32>,
    system: Option<String>,
    quiet: bool,
    output: OutputFormat,
    resume: Option<String>,
    continue_session: bool,
}

/// What the headless run writes to stdout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum OutputFormat {
    /// Stream the answer text as the model produces it.
    #[default]
    Text,
    /// Print one JSON object after the turn ends.
    Json,
    /// Stream each event as one JSON object per line.
    Jsonl,
}

enum Parsed {
    Help,
    Version,
    Run(Box<Options>),
}

fn parse_options(args: Vec<String>) -> Result<Parsed> {
    let mut options = Options::default();
    let mut iter = args.into_iter();
    while let Some(argument) = iter.next() {
        match argument.as_str() {
            "-n" | "--prompt" => options.prompt = Some(value(&argument, iter.next())?),
            "--acp" => options.acp = true,
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
            "--output-format" | "--output" => {
                let raw = value(&argument, iter.next())?;
                options.output = match raw.as_str() {
                    "text" => OutputFormat::Text,
                    "json" => OutputFormat::Json,
                    "jsonl" => OutputFormat::Jsonl,
                    other => {
                        bail!("--output-format takes text, json or jsonl, got {other:?}");
                    }
                };
            }
            "--resume" => options.resume = Some(value(&argument, iter.next())?),
            "--continue" => options.continue_session = true,
            "--quiet" | "-q" => options.quiet = true,
            "--help" | "-h" => return Ok(Parsed::Help),
            "--version" | "-V" => return Ok(Parsed::Version),
            other if other.starts_with('-') && other.len() > 1 => {
                bail!("unknown option {other}; run niki --help")
            }
            positional => bail!("unexpected argument {positional:?}; the prompt is passed with -n"),
        }
    }
    Ok(Parsed::Run(Box::new(options)))
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

fn agent_config(
    options: &Options,
    spec: &str,
    skills: &SkillCatalog,
    instructions: &Instructions,
) -> Result<AgentConfig> {
    let (_, model) = split_spec(spec)?;
    Ok(AgentConfig {
        model: model.trim().to_owned(),
        system: system_prompt(options, skills, instructions),
        max_steps: options.max_steps.unwrap_or(40),
        ..AgentConfig::default()
    })
}

/// The instruction chain for this workspace and the directory the agent runs
/// from.
fn instructions_for(root: &Path) -> Instructions {
    let cwd = std::env::current_dir().unwrap_or_else(|_| root.to_path_buf());
    Instructions::discover(root, &cwd, &config_home())
}

/// The system prompt one turn is sent with: the base text, the project
/// instruction chain and the closing line, then the skills.
fn system_prompt(options: &Options, skills: &SkillCatalog, instructions: &Instructions) -> String {
    let base = options
        .system
        .clone()
        .unwrap_or_else(|| AgentConfig::default().system);
    skills.inject_into(&instructions.inject_into(&base))
}

/// `/init` in a headless run: scaffold `AGENTS.md` and print one line.
fn init(options: &Options) -> Result<i32> {
    let root = workspace_root(options)?;
    match niki_config::instructions::write_scaffold(&root) {
        Ok(outcome) => {
            println!("{}", outcome.message());
            Ok(0)
        }
        Err(error) => {
            eprintln!(
                "niki: could not write {}: {error}",
                root.join("AGENTS.md").display()
            );
            Ok(1)
        }
    }
}

/// `/rename` in a headless run: name the current session and print one line.
///
/// Without `--resume`/`--continue` the newest session for the workspace is the
/// current one, and a workspace with none gets a session to name, the way the
/// screen creates one at startup.
fn rename(options: &Options, title: Option<&str>) -> Result<i32> {
    let root = workspace_root(options)?;
    rename_in(&config_home(), &root, options, title)
}

fn rename_in(home: &Path, root: &Path, options: &Options, title: Option<&str>) -> Result<i32> {
    let (store, session) = current_or_new_session(home, root, options)?;
    match title {
        Some(raw) => match store.set_title(&session, Some(raw)) {
            Ok(Some(title)) => println!("renamed to {title}"),
            Ok(None) => println!("title cleared"),
            Err(error) => {
                eprintln!("niki: could not rename the session: {error}");
                return Ok(1);
            }
        },
        None => match store.title(&session) {
            Ok(Some(title)) => println!("{title}"),
            Ok(None) => println!("untitled"),
            Err(error) => {
                eprintln!("niki: could not read the session title: {error}");
                return Ok(1);
            }
        },
    }
    Ok(0)
}

/// `/export` in a headless run: write the current session and print its path.
fn export(options: &Options, path: Option<&str>) -> Result<i32> {
    let root = workspace_root(options)?;
    export_in(&config_home(), &root, options, path)
}

fn export_in(home: &Path, root: &Path, options: &Options, path: Option<&str>) -> Result<i32> {
    let Some((store, session)) = named_session(home, root, options, false)? else {
        eprintln!(
            "niki: no saved session for {}; run a prompt first",
            root.display()
        );
        return Ok(1);
    };
    match niki_session::export::write(&store, &session, root, path) {
        Ok(written) => {
            println!("{}", written.display());
            Ok(0)
        }
        Err(error) => {
            eprintln!("niki: {error}");
            Ok(1)
        }
    }
}

/// The session a headless command names: `--resume`/`--continue` when given,
/// else the newest one for the workspace.
///
/// `create` lets a command that would name a session make one when the
/// workspace has none; an export never writes into a session it invented.
fn named_session(
    home: &Path,
    root: &Path,
    options: &Options,
    create: bool,
) -> Result<Option<(SessionStore, SessionId)>> {
    let store = open_store(home)?;
    if options.continue_session {
        let latest = store.latest_for(root).context("list the saved sessions")?;
        if let Some(summary) = latest {
            return Ok(Some((store, summary.id)));
        }
        return if create {
            let id = store.create(root).context("create a session")?;
            Ok(Some((store, id)))
        } else {
            Ok(None)
        };
    }
    if let Some(id) = &options.resume {
        let summary = store
            .find(id)
            .context("look up the saved session")?
            .with_context(|| {
                format!(
                    "no saved session {id:?} for {}; run without --resume to start one",
                    root.display()
                )
            })?;
        return Ok(Some((store, summary.id)));
    }
    let latest = store.latest_for(root).context("list the saved sessions")?;
    if let Some(summary) = latest {
        return Ok(Some((store, summary.id)));
    }
    if create {
        let id = store.create(root).context("create a session")?;
        return Ok(Some((store, id)));
    }
    Ok(None)
}

fn current_or_new_session(
    home: &Path,
    root: &Path,
    options: &Options,
) -> Result<(SessionStore, SessionId)> {
    named_session(home, root, options, true)?.context("create a session")
}

/// Everything after the command word, trimmed, when anything is left.
fn remainder(text: &str) -> Option<String> {
    let mut words = text.trim_start().splitn(2, char::is_whitespace);
    let _name = words.next();
    let rest = words.next().unwrap_or_default().trim();
    (!rest.is_empty()).then(|| rest.to_owned())
}

fn mode(raw: Option<&str>) -> Mode {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("auto") => Mode::Auto,
        Some("yolo") => Mode::Yolo,
        Some("manual" | "") | None => Mode::Manual,
        Some(other) => {
            eprintln!("niki: unknown mode {other:?}; using manual (manual, auto or yolo)");
            Mode::Manual
        }
    }
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Print one event. Answer text goes to stdout so `niki -n … > out.txt`
/// captures the answer; tool activity goes to stderr so a pipe stays clean.
fn report(event: &AgentEvent, quiet: bool, out: &mut impl Write) {
    match event {
        AgentEvent::Text(delta) => {
            let _ = out.write_all(delta.as_bytes());
            let _ = out.flush();
        }
        AgentEvent::ToolStart { name, arguments } if !quiet => {
            let _ = out.flush();
            eprintln!("· {} {}", sanitize_terminal(name), one_line(arguments));
        }
        AgentEvent::ToolEnd {
            name,
            output,
            is_error,
        } if !quiet => {
            let mark = if *is_error { '✗' } else { '✓' };
            eprintln!("  {mark} {} {}", sanitize_terminal(name), one_line(output));
        }
        AgentEvent::StepLimit { steps } if !quiet => {
            eprintln!("niki: step limit of {steps} reached");
        }
        AgentEvent::Notice(text) => {
            let _ = out.flush();
            eprintln!("niki: {}", sanitize_terminal(text));
        }
        AgentEvent::Cancelled if !quiet => {
            eprintln!("niki: cancelled");
        }
        _ => {}
    }
}

/// Room for one tool result on a JSON line. A file read or a shell command can
/// be far larger than a pipeline wants to buffer.
const JSONL_TOOL_TEXT_BYTES: usize = 4096;

/// Writes a turn as JSON Lines: one object per event, in order.
struct JsonlStream {
    model: String,
    session: String,
    cwd: String,
    calls: u64,
    pending: Option<String>,
    turns: u32,
    results: usize,
    input: u64,
    output: u64,
}

impl JsonlStream {
    fn new(model: &str, session: Option<&str>, cwd: &Path) -> Self {
        Self {
            model: model.to_owned(),
            session: session.unwrap_or("").to_owned(),
            cwd: cwd.display().to_string(),
            calls: 0,
            pending: None,
            turns: 0,
            results: 0,
            input: 0,
            output: 0,
        }
    }

    fn start(&self, out: &mut impl Write) {
        self.write(
            out,
            json!({
                "type": "start",
                "model": self.model.as_str(),
                "session": self.session.as_str(),
                "cwd": self.cwd.as_str(),
            }),
        );
    }

    fn event(&mut self, event: &AgentEvent, out: &mut impl Write) {
        match event {
            AgentEvent::Text(delta) => {
                self.write(out, json!({"type": "text", "delta": delta}));
            }
            AgentEvent::ToolStart { name, arguments } => {
                self.calls += 1;
                let id = format!("call_{}", self.calls);
                self.pending = Some(id.clone());
                let arguments = serde_json::from_str::<serde_json::Value>(arguments)
                    .unwrap_or_else(|_| json!({ "raw": arguments }));
                self.write(
                    out,
                    json!({
                        "type": "tool_call",
                        "id": id,
                        "name": name,
                        "arguments": arguments,
                    }),
                );
            }
            AgentEvent::ToolEnd {
                name,
                output,
                is_error,
            } => {
                self.results += 1;
                let id = self.pending.take().unwrap_or_else(|| {
                    self.calls += 1;
                    format!("call_{}", self.calls)
                });
                let (text, truncated) = truncate_tool_text(output);
                let mut value = json!({
                    "type": "tool_result",
                    "id": id,
                    "name": name,
                    "ok": !is_error,
                    "text": text,
                });
                if truncated {
                    value["truncated"] = json!(true);
                }
                self.write(out, value);
            }
            AgentEvent::Usage(usage) => {
                self.turns += 1;
                self.input += usage.input_tokens;
                self.output += usage.output_tokens;
                self.write(
                    out,
                    json!({
                        "type": "usage",
                        "input": usage.input_tokens,
                        "output": usage.output_tokens,
                        "total": usage.input_tokens + usage.output_tokens,
                    }),
                );
            }
            AgentEvent::Notice(_) | AgentEvent::StepLimit { .. } | AgentEvent::Cancelled => {}
        }
    }

    fn error(&self, out: &mut impl Write, message: &str) {
        self.write(out, json!({"type": "error", "message": message}));
    }

    fn end(
        &self,
        out: &mut impl Write,
        reason: &str,
        steps: u32,
        tool_calls: usize,
        input: u64,
        output: u64,
    ) {
        self.write(
            out,
            json!({
                "type": "end",
                "reason": reason,
                "steps": steps,
                "tool_calls": tool_calls,
                "usage": {"input": input, "output": output},
            }),
        );
    }

    fn write(&self, out: &mut impl Write, value: serde_json::Value) {
        if let Ok(line) = serde_json::to_string(&value) {
            let _ = writeln!(out, "{line}");
            let _ = out.flush();
        }
    }
}

/// Cut a tool result to the JSON-lines budget without splitting a character.
fn truncate_tool_text(text: &str) -> (&str, bool) {
    if text.len() <= JSONL_TOOL_TEXT_BYTES {
        return (text, false);
    }
    let mut end = JSONL_TOOL_TEXT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], true)
}

/// Remove the control characters that let a string drive the terminal.
///
/// A tool result, a file name or a hook message can carry CSI/OSC sequences
/// (clipboard writes, title changes, cursor moves). `is_control` covers C0,
/// DEL and C1.
fn sanitize_terminal(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .collect()
}

/// Shorten a multi-line argument or result to one readable line.
fn one_line(text: &str) -> String {
    let mut line: String = text
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    if line.chars().count() > 120 {
        line = line.chars().take(117).collect::<String>() + "...";
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_output_cannot_drive_the_terminal() {
        let hostile = "ok\u{1b}]52;c;ZWNobyBwd25lZA==\u{7}done";
        let line = one_line(hostile);
        assert!(!line.contains('\u{1b}'), "{line:?}");
        assert!(!line.chars().any(char::is_control), "{line:?}");
        assert!(line.contains("done"), "{line:?}");

        let notice = sanitize_terminal("warn\u{9b}2Jning\nnext");
        assert!(!notice.contains('\u{9b}'), "{notice:?}");
        assert!(notice.contains("next"), "{notice:?}");
    }

    #[cfg(unix)]
    #[test]
    fn the_session_home_and_database_are_private_to_the_user() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join(".niki");
        let _store = open_store(&home).expect("store");

        let home_mode = std::fs::metadata(&home)
            .expect("home metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(home_mode, 0o700, "{home_mode:o}");

        let database = home.join("sessions.db");
        let database_mode = std::fs::metadata(&database)
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(database_mode, 0o600, "{database_mode:o}");
    }

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
    fn the_acp_flag_rejects_a_headless_prompt() {
        let args = vec!["--acp".to_owned(), "-n".to_owned(), "hi".to_owned()];
        assert_eq!(run(args).expect("run"), 2);
    }

    #[test]
    fn the_acp_flag_does_not_ask_for_a_prompt() {
        let Ok(Parsed::Run(options)) = parse_options(vec!["--acp".to_owned()]) else {
            panic!("not a run");
        };
        assert!(options.acp);
        assert!(options.prompt.is_none());
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
        let instructions = Instructions::default();
        let settings = agent_config(
            &Options::default(),
            "mock:mock-model",
            &skills,
            &instructions,
        )
        .expect("config");
        assert!(settings.system.contains("## Skills"), "{}", settings.system);
        assert!(settings.system.contains("deploy"));
        assert!(settings.system.contains("Ship a release safely."));
        assert!(settings.system.contains("deploy/SKILL.md"));

        let custom = Options {
            system: Some("custom base".to_owned()),
            ..Options::default()
        };
        let settings =
            agent_config(&custom, "mock:mock-model", &skills, &instructions).expect("config");
        assert!(
            settings.system.starts_with("custom base"),
            "{}",
            settings.system
        );
        assert!(settings.system.contains("deploy/SKILL.md"));
    }

    #[test]
    fn project_instructions_follow_the_built_in_prompt_and_precede_the_skills() {
        let project = tempfile::tempdir().expect("project");
        let user_home = tempfile::tempdir().expect("user home");
        std::fs::write(project.path().join("AGENTS.md"), "REPO-INSTRUCTIONS\n")
            .expect("instructions");
        let directory = project.path().join(".niki/skills/deploy");
        std::fs::create_dir_all(&directory).expect("skill directory");
        std::fs::write(
            directory.join("SKILL.md"),
            "---\nname: deploy\ndescription: Ship a release safely.\n---\nbody\n",
        )
        .expect("skill");

        let skills = SkillCatalog::discover(project.path(), user_home.path());
        let instructions = Instructions::discover(project.path(), project.path(), user_home.path());
        let system = system_prompt(&Options::default(), &skills, &instructions);

        let built_in = system
            .find(niki_agent::DEFAULT_SYSTEM_PROMPT)
            .expect("the built-in prompt");
        let body = system.find("REPO-INSTRUCTIONS").expect("the file body");
        let skills_section = system.find("## Skills").expect("the skills section");
        assert_eq!(built_in, 0, "the built-in prompt stays first");
        assert!(body < skills_section, "{system}");
        assert!(
            system.contains(niki_config::instructions::CLOSING),
            "{system}"
        );
    }

    #[test]
    fn init_headless_writes_the_scaffold_and_keeps_it_on_the_second_run() {
        let directory = tempfile::tempdir().expect("tempdir");
        let config = directory.path().join("config.toml");
        let root = directory.path().join("workspace");
        std::fs::create_dir_all(&root).expect("workspace");
        let args = vec![
            "-n".to_owned(),
            "/init".to_owned(),
            "--root".to_owned(),
            root.display().to_string(),
            "--config".to_owned(),
            config.display().to_string(),
        ];

        assert_eq!(run(args.clone()).expect("first run"), 0);
        assert_eq!(
            std::fs::read_to_string(root.join("AGENTS.md")).expect("scaffold"),
            niki_config::instructions::SCAFFOLD
        );

        std::fs::write(root.join("AGENTS.md"), "hand written").expect("edit");
        assert_eq!(run(args).expect("second run"), 0);
        assert_eq!(
            std::fs::read_to_string(root.join("AGENTS.md")).expect("kept"),
            "hand written"
        );
    }

    #[test]
    fn rename_headless_names_and_clears_the_current_session() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join(".niki");
        let root = temp.path().join("workspace");
        std::fs::create_dir_all(&root).expect("workspace");

        assert_eq!(
            rename_in(&home, &root, &Options::default(), Some("  plan the port  "))
                .expect("rename"),
            0
        );
        let store = SessionStore::open(home.join("sessions.db")).expect("store");
        let summary = store.latest_for(&root).expect("list").expect("a session");
        assert_eq!(summary.title.as_deref(), Some("plan the port"));

        assert_eq!(
            rename_in(&home, &root, &Options::default(), None).expect("show"),
            0,
            "no argument prints the title rather than changing it"
        );
        assert_eq!(
            rename_in(&home, &root, &Options::default(), Some("   ")).expect("clear"),
            0
        );
        let summary = store.latest_for(&root).expect("list").expect("a session");
        assert_eq!(summary.title, None, "an empty title clears it");
    }

    #[test]
    fn export_headless_writes_the_session_without_overwriting() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join(".niki");
        let root = temp.path().join("workspace");
        std::fs::create_dir_all(&root).expect("workspace");

        assert_eq!(
            rename_in(&home, &root, &Options::default(), Some("export me")).expect("rename"),
            0
        );
        {
            let store = SessionStore::open(home.join("sessions.db")).expect("store");
            let session = store
                .latest_for(&root)
                .expect("list")
                .expect("a session")
                .id;
            store.append(&session, "user", "hello").expect("prompt");
            store.append(&session, "assistant", "hi").expect("answer");
        }

        assert_eq!(
            export_in(&home, &root, &Options::default(), None).expect("first export"),
            0
        );
        assert_eq!(
            export_in(&home, &root, &Options::default(), None).expect("second export"),
            0
        );

        let mut names: Vec<String> = std::fs::read_dir(&root)
            .expect("workspace")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(
            names.iter().any(|name| name.ends_with("-2.md")),
            "{names:?}"
        );
        assert!(
            names.iter().any(|name| !name.contains("-2.md")),
            "{names:?}"
        );

        let second = names
            .iter()
            .find(|name| name.ends_with("-2.md"))
            .expect("the second file");
        let text = std::fs::read_to_string(root.join(second)).expect("markdown");
        assert!(text.starts_with("# export me\n"), "{text}");
        assert!(text.contains("## You\n\nhello"), "{text}");
    }

    #[test]
    fn export_headless_refuses_a_path_outside_the_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join(".niki");
        let root = temp.path().join("workspace");
        std::fs::create_dir_all(&root).expect("workspace");

        assert_eq!(
            rename_in(&home, &root, &Options::default(), Some("keep me")).expect("rename"),
            0
        );
        assert_eq!(
            export_in(&home, &root, &Options::default(), Some("../outside.md")).expect("export"),
            1
        );
        assert!(!temp.path().join("outside.md").exists());
    }

    #[test]
    fn a_workspace_without_instruction_files_keeps_the_old_prompt_exactly() {
        let project = tempfile::tempdir().expect("project");
        let user_home = tempfile::tempdir().expect("user home");
        let instructions = Instructions::discover(project.path(), project.path(), user_home.path());
        let system = system_prompt(&Options::default(), &SkillCatalog::default(), &instructions);
        assert_eq!(system, niki_agent::DEFAULT_SYSTEM_PROMPT);
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
