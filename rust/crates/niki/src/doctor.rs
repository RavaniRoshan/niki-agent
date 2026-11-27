//! `niki doctor`: a read-only diagnosis of the install.
//!
//! Every check is built from inputs the caller has already resolved — the
//! config file, the model, the credential state, the session store and so on.
//! The report itself touches no files, no network and no terminal, so it can be
//! exercised in unit tests and printed over a pipe.
//!
//! A check is `ok`, `warn` or `fail`. Only a `fail` makes the process exit `1`:
//! a warning is something the install can work around (sessions are not saved,
//! hooks are broken) while a failure is something a turn cannot start without
//! (a config that cannot be read, no model, no credential).

use std::path::PathBuf;

use niki_config::Config;

/// The severity of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// The check passed.
    Ok,
    /// Something is missing or degraded; a turn can still run.
    Warn,
    /// Something essential is missing; a turn cannot run.
    Fail,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

/// One `ok`/`warn`/`fail` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// How the check ended.
    pub level: Level,
    /// What was checked, for example `config`.
    pub label: &'static str,
    /// What was found, always on one line.
    pub detail: String,
}

impl Line {
    /// A line whose detail is folded onto one line and stripped of control
    /// characters, so a multi-line parser error cannot break the report.
    #[must_use]
    pub fn new(level: Level, label: &'static str, detail: impl AsRef<str>) -> Self {
        Self {
            level,
            label,
            detail: one_line(detail.as_ref()),
        }
    }

    /// Render this line in the report's fixed `ok   <label>: <detail>` shape.
    #[must_use]
    pub fn render(&self) -> String {
        format!("{:<5}{}: {}", self.level.label(), self.label, self.detail)
    }
}

/// The resolved config file, as the run path would see it.
#[derive(Debug)]
pub enum ConfigState {
    /// The file exists and parsed.
    Loaded(Config),
    /// No file at the resolved path; the built-in defaults apply.
    Missing,
    /// The file exists but cannot be read or parsed.
    Broken(String),
}

/// Whether the resolved provider has the credential a turn needs.
///
/// Only the environment variable's name is carried; its value is never stored
/// or printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    /// The variable is set and not blank.
    Set {
        /// Provider the variable belongs to.
        provider: String,
        /// Environment variable holding the key.
        var: String,
    },
    /// The variable is unset or blank.
    Unset {
        /// Provider the variable belongs to.
        provider: String,
        /// Environment variable the provider block names.
        var: String,
    },
    /// The provider names no key variable at all.
    Unnamed {
        /// Provider whose block has no `api_key_env`.
        provider: String,
    },
    /// The endpoint accepts requests without a key.
    NotRequired {
        /// Provider with a keyless endpoint.
        provider: String,
    },
    /// No provider resolved, so there is nothing to check.
    Unknown,
}

/// Read the credential state for `provider` through `lookup`.
///
/// The rules are the ones `niki_provider::api_key` applies when it builds the
/// client: a keyless provider needs no credential, a named variable that is
/// unset or blank is a missing key, and a block that names no variable cannot
/// be used. `lookup` is injectable so tests need not touch the process
/// environment; the value it returns is checked and dropped, never retained.
#[must_use]
pub fn credential(
    provider: Option<&niki_config::Provider>,
    provider_name: &str,
    lookup: impl Fn(&str) -> Option<String>,
) -> Credential {
    let Some(provider) = provider else {
        return Credential::Unknown;
    };
    let var = provider
        .api_key_env
        .as_deref()
        .map(str::trim)
        .filter(|var| !var.is_empty());

    if provider.keyless() {
        return match var {
            Some(var) if lookup(var).is_some_and(|value| !value.trim().is_empty()) => {
                Credential::Set {
                    provider: provider_name.to_owned(),
                    var: var.to_owned(),
                }
            }
            _ => Credential::NotRequired {
                provider: provider_name.to_owned(),
            },
        };
    }

    let Some(var) = var else {
        return Credential::Unnamed {
            provider: provider_name.to_owned(),
        };
    };
    match lookup(var) {
        Some(value) if !value.trim().is_empty() => Credential::Set {
            provider: provider_name.to_owned(),
            var: var.to_owned(),
        },
        _ => Credential::Unset {
            provider: provider_name.to_owned(),
            var: var.to_owned(),
        },
    }
}

/// What `hooks.json` contributed, without any of its warning text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookState {
    /// Hook events that have at least one handler.
    pub events: usize,
    /// Warnings raised while loading the files.
    pub warnings: usize,
}

/// Everything the report needs, already resolved by the caller.
#[derive(Debug)]
pub struct Inputs {
    /// The config path `--config`, `$NIKI_HOME/config.toml` or
    /// `~/.niki/config.toml` resolved to.
    pub config_path: PathBuf,
    /// Whether that file exists and parses.
    pub config: ConfigState,
    /// The resolved `provider:model` spec, or why one could not be resolved.
    pub model: Result<String, String>,
    /// The credential state for the model's provider.
    pub credential: Credential,
    /// The directory holding `sessions.db`.
    pub session_dir: PathBuf,
    /// The number of saved sessions, or why the store could not be read.
    pub sessions: Result<usize, String>,
    /// Instruction files loaded for the working directory.
    pub instruction_files: usize,
    /// Total instruction bytes after the chain's budget.
    pub instruction_bytes: usize,
    /// Discovered skills, built-ins included.
    pub skill_count: usize,
    /// Skill warnings, counted but not shown.
    pub skill_warnings: usize,
    /// Configured MCP servers; none is started.
    pub mcp_servers: usize,
    /// Whether hooks loaded, and how many warnings they raised.
    pub hooks: HookState,
}

/// The full report: one line per check plus a one-line summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    lines: Vec<Line>,
}

impl Report {
    /// The individual check lines, in report order.
    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// `0` when every check passed or warned, `1` when one failed.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        i32::from(self.lines().iter().any(|line| line.level == Level::Fail))
    }

    /// The one-line verdict that follows the checks.
    #[must_use]
    pub fn summary(&self) -> String {
        let ok = self
            .lines()
            .iter()
            .filter(|line| line.level == Level::Ok)
            .count();
        let warnings = self
            .lines()
            .iter()
            .filter(|line| line.level == Level::Warn)
            .count();
        let failures = self
            .lines()
            .iter()
            .filter(|line| line.level == Level::Fail)
            .count();
        let verdict = if failures == 0 {
            "the install is usable"
        } else {
            "fix the failures above"
        };
        format!(
            "summary: {} check(s): {ok} ok, {warnings} warning(s), {failures} failure(s) — {verdict}",
            self.lines().len(),
        )
    }

    /// The whole report, checks first and the summary last, newline-terminated.
    #[must_use]
    pub fn render(&self) -> String {
        let mut text = String::new();
        for line in self.lines() {
            text.push_str(&line.render());
            text.push('\n');
        }
        text.push_str(&self.summary());
        text.push('\n');
        text
    }
}

/// Build the report from resolved inputs.
#[must_use]
pub fn report(inputs: &Inputs) -> Report {
    Report {
        lines: vec![
            version_line(),
            config_line(inputs),
            model_line(inputs),
            credential_line(&inputs.credential),
            sessions_line(inputs),
            instructions_line(inputs),
            skills_line(inputs),
            mcp_line(inputs),
            hooks_line(inputs.hooks),
        ],
    }
}

/// The client version, exactly as `--version` prints it, plus the MCP protocol
/// this build speaks.
fn version_line() -> Line {
    Line::new(
        Level::Ok,
        "version",
        format!(
            "niki {} (MCP protocol {})",
            env!("CARGO_PKG_VERSION"),
            niki_mcp::PROTOCOL_VERSION
        ),
    )
}

fn config_line(inputs: &Inputs) -> Line {
    let path = inputs.config_path.display();
    match &inputs.config {
        ConfigState::Loaded(_) => {
            Line::new(Level::Ok, "config", format!("{path} exists and parses"))
        }
        ConfigState::Missing => Line::new(
            Level::Warn,
            "config",
            format!("{path} not found; built-in defaults apply"),
        ),
        ConfigState::Broken(message) => Line::new(Level::Fail, "config", message),
    }
}

fn model_line(inputs: &Inputs) -> Line {
    match &inputs.model {
        Ok(spec) => Line::new(Level::Ok, "model", spec),
        Err(message) => Line::new(Level::Fail, "model", message),
    }
}

fn credential_line(credential: &Credential) -> Line {
    match credential {
        Credential::Set { provider, var } => {
            Line::new(Level::Ok, "credential", format!("{provider}: {var} is set"))
        }
        Credential::Unset { provider, var } => Line::new(
            Level::Fail,
            "credential",
            format!("{provider}: {var} is unset; export {var} before running a turn"),
        ),
        Credential::Unnamed { provider } => Line::new(
            Level::Fail,
            "credential",
            format!(
                "{provider}: no API key variable configured; set [providers.{provider}].api_key_env"
            ),
        ),
        Credential::NotRequired { provider } => Line::new(
            Level::Ok,
            "credential",
            format!("{provider}: no credential needed for a keyless endpoint"),
        ),
        Credential::Unknown => Line::new(
            Level::Fail,
            "credential",
            "no provider resolved; set a model before checking the credential",
        ),
    }
}

fn sessions_line(inputs: &Inputs) -> Line {
    let dir = inputs.session_dir.display();
    match &inputs.sessions {
        Ok(count) => Line::new(
            Level::Ok,
            "sessions",
            format!("{dir}: {count} saved session(s)"),
        ),
        // Sessions are a convenience: a turn still runs when the store cannot
        // be read, so this is not a failed install.
        Err(message) => Line::new(Level::Warn, "sessions", format!("{dir}: {message}")),
    }
}

fn instructions_line(inputs: &Inputs) -> Line {
    Line::new(
        Level::Ok,
        "instructions",
        format!(
            "{} file(s), {} bytes for the working directory",
            inputs.instruction_files, inputs.instruction_bytes
        ),
    )
}

fn skills_line(inputs: &Inputs) -> Line {
    Line::new(
        Level::Ok,
        "skills",
        format!(
            "{} skill(s), {} warning(s)",
            inputs.skill_count, inputs.skill_warnings
        ),
    )
}

fn mcp_line(inputs: &Inputs) -> Line {
    Line::new(
        Level::Ok,
        "mcp",
        format!("{} server(s) configured (none started)", inputs.mcp_servers),
    )
}

fn hooks_line(hooks: HookState) -> Line {
    let detail = match (hooks.events, hooks.warnings) {
        (0, 0) => "no hooks configured".to_owned(),
        (0, warnings) => format!("no handlers loaded, {warnings} warning(s)"),
        (events, 0) => format!("{events} event(s) with handlers"),
        (events, warnings) => format!("{events} event(s) with handlers, {warnings} warning(s)"),
    };
    let level = if hooks.warnings > 0 {
        Level::Warn
    } else {
        Level::Ok
    };
    Line::new(level, "hooks", detail)
}

/// Fold arbitrary text onto one line without control characters.
fn one_line(text: &str) -> String {
    let mut line = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        let word: String = word
            .chars()
            .filter(|character| !character.is_control())
            .collect();
        if word.is_empty() {
            continue;
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word);
    }
    if line.chars().count() > 240 {
        line = line.chars().take(237).collect::<String>() + "...";
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn green() -> Inputs {
        Inputs {
            config_path: PathBuf::from("/home/tester/.niki/config.toml"),
            config: ConfigState::Loaded(
                Config::from_str("[models]\ndefault = \"groq:llama-3.3-70b\"\n").expect("config"),
            ),
            model: Ok("groq:llama-3.3-70b".to_owned()),
            credential: Credential::Set {
                provider: "groq".to_owned(),
                var: "GROQ_API_KEY".to_owned(),
            },
            session_dir: PathBuf::from("/home/tester/.niki"),
            sessions: Ok(3),
            instruction_files: 2,
            instruction_bytes: 2048,
            skill_count: 12,
            skill_warnings: 0,
            mcp_servers: 1,
            hooks: HookState {
                events: 2,
                warnings: 0,
            },
        }
    }

    fn line<'a>(report: &'a Report, label: &str) -> &'a Line {
        report
            .lines()
            .iter()
            .find(|line| line.label == label)
            .unwrap_or_else(|| panic!("no {label} line"))
    }

    #[test]
    fn a_healthy_install_is_all_ok_and_exits_zero() {
        let report = report(&green());

        assert_eq!(report.exit_code(), 0);
        assert!(
            report.lines().iter().all(|line| line.level == Level::Ok),
            "{:?}",
            report.lines()
        );
        assert_eq!(report.lines().len(), 9);
        assert!(
            report
                .render()
                .contains("summary: 9 check(s): 9 ok, 0 warning(s), 0 failure(s)")
        );
    }

    #[test]
    fn every_line_uses_the_fixed_shape_and_ends_with_the_summary() {
        let text = report(&green()).render();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 10, "{text}");
        for line in &lines[..9] {
            assert!(
                line.starts_with("ok   ") || line.starts_with("warn ") || line.starts_with("fail "),
                "{line:?}"
            );
        }
        assert!(lines[9].starts_with("summary: "), "{:?}", lines[9]);
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn a_missing_config_warns_and_still_exits_zero() {
        let mut inputs = green();
        inputs.config = ConfigState::Missing;

        let report = report(&inputs);
        let config = line(&report, "config");
        assert_eq!(config.level, Level::Warn);
        assert!(config.detail.contains("not found"), "{}", config.detail);
        assert_eq!(report.exit_code(), 0, "defaults still let a turn run");
    }

    #[test]
    fn a_missing_credential_fails_and_names_the_variable() {
        let mut inputs = green();
        inputs.credential = Credential::Unset {
            provider: "groq".to_owned(),
            var: "GROQ_API_KEY".to_owned(),
        };

        let report = report(&inputs);
        let credential = line(&report, "credential");
        assert_eq!(credential.level, Level::Fail);
        assert!(
            credential.detail.contains("GROQ_API_KEY"),
            "{}",
            credential.detail
        );
        assert_eq!(report.exit_code(), 1);
        assert!(report.render().contains("fix the failures above"));
    }

    #[test]
    fn an_unparseable_config_fails() {
        let mut inputs = green();
        inputs.config = ConfigState::Broken(
            "invalid TOML in /home/tester/.niki/config.toml: expected a closing quote\n\nline 2"
                .to_owned(),
        );

        let report = report(&inputs);
        let config = line(&report, "config");
        assert_eq!(config.level, Level::Fail);
        assert!(config.detail.contains("invalid TOML"), "{}", config.detail);
        assert!(
            !config.detail.contains('\n'),
            "the detail stays on one line: {}",
            config.detail
        );
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn a_secret_value_never_reaches_the_report() {
        let provider = niki_config::Provider {
            api_key_env: Some("NIKI_FAKE_KEY".to_owned()),
            ..niki_config::Provider::default()
        };
        let credential = credential(Some(&provider), "fake", |_| {
            Some("sk-super-secret-value".to_owned())
        });
        assert_eq!(
            credential,
            Credential::Set {
                provider: "fake".to_owned(),
                var: "NIKI_FAKE_KEY".to_owned(),
            }
        );

        let mut inputs = green();
        inputs.credential = credential;
        let text = report(&inputs).render();
        assert!(!text.contains("sk-super-secret-value"), "{text}");
        assert!(text.contains("NIKI_FAKE_KEY"), "{text}");
    }

    #[test]
    fn a_blank_or_absent_variable_is_reported_as_unset() {
        let provider = niki_config::Provider {
            api_key_env: Some("BLANK_KEY".to_owned()),
            ..niki_config::Provider::default()
        };
        assert_eq!(
            credential(Some(&provider), "fake", |_| Some("   ".to_owned())),
            Credential::Unset {
                provider: "fake".to_owned(),
                var: "BLANK_KEY".to_owned(),
            }
        );
        assert_eq!(
            credential(Some(&provider), "fake", |_| None),
            Credential::Unset {
                provider: "fake".to_owned(),
                var: "BLANK_KEY".to_owned(),
            }
        );

        let unnamed = niki_config::Provider::default();
        assert_eq!(
            credential(Some(&unnamed), "fake", |_| None),
            Credential::Unnamed {
                provider: "fake".to_owned(),
            }
        );
    }

    #[test]
    fn a_keyless_provider_needs_no_credential() {
        let provider = niki_config::Provider {
            api_key_env: Some("OLLAMA_API_KEY".to_owned()),
            keyless: Some(true),
            ..niki_config::Provider::default()
        };
        assert_eq!(
            credential(Some(&provider), "ollama", |_| None),
            Credential::NotRequired {
                provider: "ollama".to_owned(),
            }
        );
        assert_eq!(report(&green()).lines().len(), 9);
    }

    #[test]
    fn a_broken_hooks_file_warns_without_failing_the_install() {
        let mut inputs = green();
        inputs.hooks = HookState {
            events: 0,
            warnings: 1,
        };

        let report = report(&inputs);
        let hooks = line(&report, "hooks");
        assert_eq!(hooks.level, Level::Warn);
        assert!(hooks.detail.contains("1 warning(s)"), "{}", hooks.detail);
        assert_eq!(report.exit_code(), 0);
    }
}
