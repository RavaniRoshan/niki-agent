//! `hooks.json` loading, validation and per-event precedence.
//!
//! Two files are read: `{project_root}/.niki/hooks.json` and
//! `{config_dir}/hooks.json` (normally `~/.niki/hooks.json`). The project file
//! wins for an event it defines; the user file supplies the events it does not.
//! A file that cannot be read or parsed is a warning, not a failed start.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use niki_config::trust::ProjectTrust;
use serde_json::{Map, Value};

use crate::matcher::Matcher;

/// Hook events this build runs. Every other event in the file is ignored with
/// a warning until its lifecycle seam exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event {
    /// Before a tool call runs; may block it.
    PreToolUse,
    /// After a tool call; may add feedback to its result.
    PostToolUse,
    /// Before a prompt reaches the model; may stop it.
    UserPromptSubmit,
}

impl Event {
    /// Every event this build understands, in declaration order.
    pub const ALL: [Self; 3] = [Self::PreToolUse, Self::PostToolUse, Self::UserPromptSubmit];

    /// The event name written in `hooks.json` and on the wire.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::UserPromptSubmit => "UserPromptSubmit",
        }
    }

    /// Timeout applied when a handler does not set one.
    #[must_use]
    pub fn default_timeout(self) -> Duration {
        match self {
            Self::PreToolUse | Self::PostToolUse => Duration::from_secs(600),
            Self::UserPromptSubmit => Duration::from_secs(30),
        }
    }

    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "PreToolUse" => Some(Self::PreToolUse),
            "PostToolUse" => Some(Self::PostToolUse),
            "UserPromptSubmit" => Some(Self::UserPromptSubmit),
            _ => None,
        }
    }
}

/// One command handler, already bounded by its timeout.
#[derive(Debug, Clone)]
pub(crate) struct Handler {
    pub(crate) command: String,
    pub(crate) timeout: Duration,
}

/// A matcher and the handlers it selects.
#[derive(Debug, Clone)]
pub(crate) struct Group {
    pub(crate) matcher: Option<Matcher>,
    pub(crate) handlers: Vec<Handler>,
}

impl Group {
    pub(crate) fn matches(&self, target: &str) -> bool {
        match &self.matcher {
            None => true,
            Some(matcher) => matcher.matches(target),
        }
    }
}

/// The validated configuration plus the warnings raised while loading it.
#[derive(Debug)]
pub struct Loaded {
    /// The runner built from both files.
    pub hooks: Hooks,
    /// Human-readable problems. A caller shows them once and carries on.
    pub warnings: Vec<String>,
}

/// Hook configuration for one session.
#[derive(Debug, Default)]
pub struct Hooks {
    pub(crate) groups: BTreeMap<Event, Vec<Group>>,
    project_suppressed: bool,
}

impl Hooks {
    /// Load `{project_root}/.niki/hooks.json` and
    /// `{config_dir}/hooks.json`, admitting project hooks.
    ///
    /// This is the historical entry point: a host that has not made a trust
    /// decision keeps the behaviour it had before the gate existed. A host that
    /// gates project content calls [`Self::load_with`] with the workspace's
    /// [`ProjectTrust`].
    ///
    /// `config_dir` is the user configuration directory — the one holding
    /// `config.toml` — so `NIKI_HOME` is honoured without this crate reading
    /// the environment itself.
    #[must_use]
    pub fn load(project_root: &Path, config_dir: &Path) -> Loaded {
        Self::load_with(project_root, config_dir, ProjectTrust::Trusted)
    }

    /// Load the project and user hook files under `trust`.
    ///
    /// An untrusted workspace never reads the project file; the user file still
    /// loads and the reason is one [`Loaded::warnings`] entry.
    #[must_use]
    pub fn load_with(project_root: &Path, config_dir: &Path, trust: ProjectTrust) -> Loaded {
        Self::load_from_with(
            &project_root.join(".niki").join("hooks.json"),
            &config_dir.join("hooks.json"),
            trust,
        )
    }

    /// Load two explicit files, admitting project hooks.
    ///
    /// The project path wins per event, the user path fills in the rest; equal
    /// paths are read once.
    #[must_use]
    pub fn load_from(project: &Path, user: &Path) -> Loaded {
        Self::load_from_with(project, user, ProjectTrust::Trusted)
    }

    /// Load two explicit files under `trust`.
    ///
    /// An untrusted workspace does not read the project path; the user path
    /// still loads unless it is the same file. The warning is recorded only
    /// when the project file exists, because a missing file suppresses nothing.
    #[must_use]
    pub fn load_from_with(project: &Path, user: &Path, trust: ProjectTrust) -> Loaded {
        let mut warnings = Vec::new();
        let project_document = if trust.is_trusted() {
            read_document(project, &mut warnings)
        } else {
            if project.try_exists().unwrap_or(false) {
                warnings.push(format!(
                    "project hooks at {} are not loaded because this workspace is not trusted",
                    project.display()
                ));
            }
            BTreeMap::new()
        };
        let user_document = if same_file(project, user) {
            BTreeMap::new()
        } else {
            read_document(user, &mut warnings)
        };

        let mut groups = BTreeMap::new();
        for event in Event::ALL {
            let chosen = project_document
                .get(&event)
                .filter(|groups| !groups.is_empty())
                .or_else(|| user_document.get(&event));
            if let Some(chosen) = chosen {
                groups.insert(event, chosen.clone());
            }
        }

        Loaded {
            hooks: Self {
                groups,
                project_suppressed: !trust.is_trusted(),
            },
            warnings,
        }
    }

    /// Whether the project hook file was skipped because the workspace is
    /// untrusted.
    #[must_use]
    pub fn project_suppressed(&self) -> bool {
        self.project_suppressed
    }

    /// Whether the configuration has at least one handler for `event`.
    #[must_use]
    pub fn has_handlers(&self, event: Event) -> bool {
        self.groups
            .get(&event)
            .is_some_and(|groups| groups.iter().any(|group| !group.handlers.is_empty()))
    }
}

fn same_file(left: &Path, right: &Path) -> bool {
    match (std::path::absolute(left), std::path::absolute(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn read_document(path: &Path, warnings: &mut Vec<String>) -> BTreeMap<Event, Vec<Group>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return BTreeMap::new();
        }
        Err(error) => {
            warnings.push(format!(
                "cannot read hooks config at {}: {error}",
                path.display()
            ));
            return BTreeMap::new();
        }
    };

    let document: Value = match serde_json::from_slice(&bytes) {
        Ok(document) => document,
        Err(error) => {
            warnings.push(format!(
                "hooks config at {} is not valid JSON: {error}",
                path.display()
            ));
            return BTreeMap::new();
        }
    };

    let Some(root) = document.as_object() else {
        warnings.push(format!(
            "hooks config at {} must be a JSON object",
            path.display()
        ));
        return BTreeMap::new();
    };

    let events = match root.get("hooks") {
        Some(Value::Object(events)) => events,
        Some(_) => {
            warnings.push(format!(
                "hooks config at {}: `hooks` must be an object",
                path.display()
            ));
            return BTreeMap::new();
        }
        // A bare map of event name to matcher groups is accepted as well.
        None => root,
    };

    let mut parsed: BTreeMap<Event, Vec<Group>> = BTreeMap::new();
    for (name, value) in events {
        let Some(event) = Event::parse(name) else {
            warnings.push(format!(
                "hooks config at {}: ignoring unsupported event {name}",
                path.display()
            ));
            continue;
        };
        let groups = parse_groups(path, event, value, warnings);
        if !groups.is_empty() {
            parsed.insert(event, groups);
        }
    }
    parsed
}

fn parse_groups(
    path: &Path,
    event: Event,
    value: &Value,
    warnings: &mut Vec<String>,
) -> Vec<Group> {
    let Some(items) = value.as_array() else {
        warnings.push(format!(
            "hooks config at {}: {} must be a list of matcher groups",
            path.display(),
            event.name()
        ));
        return Vec::new();
    };

    let mut groups = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let location = format!("{}[{}]", event.name(), index);
        let Some(raw) = item.as_object() else {
            warnings.push(format!(
                "hooks config at {}: {location} must be an object",
                path.display()
            ));
            continue;
        };

        let matcher = match raw.get("matcher") {
            None | Some(Value::Null) => None,
            Some(Value::String(raw)) => match Matcher::compile(raw) {
                Ok(matcher) => {
                    if event == Event::UserPromptSubmit && !raw.is_empty() && raw != "*" {
                        warnings.push(format!(
                            "hooks config at {}: UserPromptSubmit has no matcher; {raw:?} is ignored",
                            path.display()
                        ));
                        None
                    } else {
                        matcher
                    }
                }
                Err(message) => {
                    warnings.push(format!(
                        "hooks config at {}: {location}.matcher: {message}",
                        path.display()
                    ));
                    continue;
                }
            },
            Some(_) => {
                warnings.push(format!(
                    "hooks config at {}: {location}.matcher must be a string",
                    path.display()
                ));
                continue;
            }
        };

        let handlers = parse_handlers(path, &location, raw, event, warnings);
        if !handlers.is_empty() {
            groups.push(Group { matcher, handlers });
        }
    }
    groups
}

fn parse_handlers(
    path: &Path,
    location: &str,
    group: &Map<String, Value>,
    event: Event,
    warnings: &mut Vec<String>,
) -> Vec<Handler> {
    let Some(items) = group.get("hooks") else {
        warnings.push(format!(
            "hooks config at {}: {location}.hooks is required",
            path.display()
        ));
        return Vec::new();
    };
    let Some(items) = items.as_array() else {
        warnings.push(format!(
            "hooks config at {}: {location}.hooks must be a list of handlers",
            path.display()
        ));
        return Vec::new();
    };

    let mut handlers = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let field = format!("{location}.hooks[{index}]");
        let Some(raw) = item.as_object() else {
            warnings.push(format!(
                "hooks config at {}: {field} must be an object",
                path.display()
            ));
            continue;
        };
        match parse_handler(raw, event) {
            Ok(handler) => handlers.push(handler),
            Err(message) => warnings.push(format!(
                "hooks config at {}: {field}: {message}",
                path.display()
            )),
        }
    }
    handlers
}

fn parse_handler(raw: &Map<String, Value>, event: Event) -> Result<Handler, String> {
    if let Some(kind) = raw.get("type") {
        match kind.as_str() {
            Some("command") => {}
            _ => return Err("`type` must be \"command\"".to_owned()),
        }
    }
    if raw.get("async") == Some(&Value::Bool(true)) {
        return Err("async command hooks are not supported".to_owned());
    }
    let Some(command) = raw.get("command").and_then(Value::as_str) else {
        return Err("`command` is required and must be a string".to_owned());
    };
    if command.trim().is_empty() {
        return Err("`command` must not be empty".to_owned());
    }
    let timeout = match raw.get("timeout") {
        None | Some(Value::Null) => event.default_timeout(),
        Some(value) => {
            let seconds = value
                .as_f64()
                .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
                .ok_or_else(|| "`timeout` must be a positive number of seconds".to_owned())?;
            Duration::try_from_secs_f64(seconds)
                .map_err(|_| "`timeout` is out of range".to_owned())?
        }
    };
    Ok(Handler {
        command: command.to_owned(),
        timeout,
    })
}
