//! The user's `~/.niki/config.toml`.
//!
//! The table names are `[providers.<name>]` for user providers (also accepted
//! under `[models.providers.<name>]`), `[models].default` for the default
//! model, `[ui]` for the theme, the motion preference and the per-effect
//! switches, `[mcp_servers.<name>]` for MCP servers, and `[notify]` for the
//! program run when a turn ends. Unknown keys are
//! ignored, so a file written for a newer release still loads. This crate only
//! reads `config.toml`; the binary does not write it. [`instructions`] is the
//! one part that writes a file, and only when `/init` asks it to.
//!
//! [`trust`] holds the persisted workspace trust decisions that gate project
//! skills, hooks and instruction files.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, fs};

use serde::Deserialize;

mod builtin;
pub mod credentials;
pub mod edit;
pub mod instructions;
pub mod trust;

/// The providers the product ships, in picker order.
///
/// Each entry carries the endpoint, the protocol and the key variable a
/// `[providers.*]` block would otherwise have to repeat. The connect screen
/// lists these, so the catalog and the resolver cannot disagree.
#[must_use]
pub fn builtin_providers() -> Vec<(&'static str, Provider)> {
    builtin::BUILTIN_NAMES
        .iter()
        .filter_map(|name| builtin::builtin(name).map(|provider| (*name, provider)))
        .collect()
}

/// The wire protocol a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// OpenAI Chat Completions, which every non-Anthropic provider ships.
    OpenAi,
    /// Anthropic Messages.
    Anthropic,
}

/// One `[providers.<name>]` block.
///
/// Every optional field stays optional: a block may name only `base_url` and
/// rely on the protocol defaults.
#[derive(Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Provider {
    /// Human-readable name shown in the model picker.
    pub display_name: Option<String>,
    /// Provider class path, read as the protocol selector.
    pub class_path: Option<String>,
    /// Endpoint base URL.
    pub base_url: Option<String>,
    /// Name of the environment variable that holds the API key.
    pub api_key_env: Option<String>,
    /// Model identifiers this provider exposes.
    pub models: Vec<String>,
    /// Explicit protocol selector, overriding [`Provider::protocol`] inference.
    pub protocol: Option<String>,
    /// Whether the endpoint needs no credential, for a local daemon.
    ///
    /// Unset follows the built-in default: `true` for a local endpoint, `false`
    /// for every hosted one. A user block may set it either way.
    pub keyless: Option<bool>,
    /// The credential itself, when one was stored beside `config.toml`.
    ///
    /// Never written to `config.toml`: the reader fills it from
    /// `credentials.toml`, and the model client prefers it over the
    /// environment variable. `Debug` masks it.
    pub api_key: Option<String>,
}

impl Provider {
    /// The protocol adapter this provider selects.
    ///
    /// An explicit `protocol` key wins when it names a known protocol; a value
    /// this crate does not recognise falls back to the `class_path` inference
    /// rather than rejecting the provider. Without either signal the provider
    /// is OpenAI-compatible, which is what every built-in non-Anthropic
    /// provider is.
    #[must_use]
    pub fn protocol(&self) -> Protocol {
        let explicit = self
            .protocol
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase);
        match explicit.as_deref() {
            Some("anthropic") => Protocol::Anthropic,
            Some("openai" | "openai-compatible" | "openai_compatible") => Protocol::OpenAi,
            _ => match self.class_path.as_deref() {
                Some(path) if path.to_ascii_lowercase().contains("anthropic") => {
                    Protocol::Anthropic
                }
                _ => Protocol::OpenAi,
            },
        }
    }

    /// Whether the endpoint needs no credential.
    #[must_use]
    pub fn keyless(&self) -> bool {
        self.keyless == Some(true)
    }

    fn api_key_env_set(&self) -> bool {
        self.api_key_env
            .as_deref()
            .is_some_and(|name| !name.is_empty() && env::var_os(name).is_some())
    }
}

/// Never prints an API key value, only whether the named variable is set.
impl fmt::Debug for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Provider")
            .field("display_name", &self.display_name)
            .field("class_path", &self.class_path)
            .field("base_url", &self.base_url)
            .field("api_key_env", &self.api_key_env)
            .field("api_key_env_set", &self.api_key_env_set())
            .field("api_key", &self.api_key.as_deref().map(|_| "<stored>"))
            .field("models", &self.models)
            .field("protocol", &self.protocol())
            .field("keyless", &self.keyless())
            .finish()
    }
}

/// One `[mcp_servers.<name>]` entry.
///
/// String values are expanded once, when the config is read: `${VAR}` becomes
/// the value of environment variable `VAR`, and `$$` becomes a literal `$`.
/// Expansion covers `command`, every element of `args`, every value of `env`,
/// `url`, and every value of `headers`. An unset or non-UTF-8 variable, an
/// empty name (`${}`), and a `${` with no closing brace are rejected as
/// [`ConfigError::McpServer`] naming the server and the variable; a missing
/// token never becomes an empty URL or header at runtime. A `$` that starts
/// neither form is literal text.
///
/// `timeout`, in seconds, bounds both the startup handshake and every later
/// request for this server. Unset keeps the client defaults: the global
/// startup budget (`NIKI_MCP_STARTUP_TIMEOUT_SECS` or ten seconds) for the
/// handshake and the one-minute request budget. A zero or negative value is
/// rejected as [`ConfigError::McpServer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServer {
    /// How the client reaches this server.
    pub transport: McpTransport,
    /// Per-server budget for the handshake and every later request.
    pub timeout: Option<Duration>,
}

/// The transport a `[mcp_servers.<name>]` entry declares.
///
/// The shape matches `.mcp.json`: `command` plus `args` and `env` for stdio,
/// `url` plus `headers` for HTTP. An entry without a `type` infers the
/// transport from which of `url` and `command` it sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpTransport {
    /// A child process speaking line-delimited JSON-RPC on stdin/stdout.
    Stdio {
        /// Executable to run.
        command: String,
        /// Arguments passed to the executable.
        args: Vec<String>,
        /// Extra environment variables for the child; the parent's are kept.
        env: BTreeMap<String, String>,
        /// Working directory for the child, when one is configured.
        cwd: Option<PathBuf>,
    },
    /// A streamable HTTP endpoint.
    Http {
        /// Endpoint URL.
        url: String,
        /// Headers sent with every request.
        headers: BTreeMap<String, String>,
    },
}

/// One `[notify]` block: the program run once when a turn ends.
///
/// A block without `program` — or with an empty one — spawns nothing. `args`
/// are passed to the program as separate arguments, never through a shell.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Notify {
    /// Executable to run.
    pub program: Option<String>,
    /// Arguments passed to the executable, verbatim.
    pub args: Vec<String>,
}

/// The parts of `config.toml` this port consumes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// Provider blocks, keyed by the table name after `providers.`.
    pub providers: BTreeMap<String, Provider>,
    /// `[models].default`, a `provider:model` spec.
    pub default_model: Option<String>,
    /// `[ui].theme`.
    pub theme: Option<String>,
    /// `[ui].motion`: `full` or `reduced`.
    pub motion: Option<String>,
    /// `[ui].welcome`, `.progress`, `.shimmer` and `.title`.
    pub effects: Effects,
    /// MCP servers, keyed by the table name after `mcp_servers.`.
    pub mcp_servers: BTreeMap<String, McpServer>,
    /// `[notify]`, the turn-end program.
    pub notify: Notify,
    /// Provider API keys read from `credentials.toml`, keyed by provider name.
    ///
    /// Filled by [`Config::load`] from the file beside the configuration; an
    /// in-memory config built from text has none.
    pub credentials: BTreeMap<String, String>,
}

/// The per-effect switches under `[ui]`, each absent meaning on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Effects {
    /// Draw the welcome block on the first screen.
    pub welcome: Option<bool>,
    /// Draw the spinner beside the turn state.
    pub progress: Option<bool>,
    /// Animate the activity marker on a running tool call.
    pub shimmer: Option<bool>,
    /// Set the terminal window title.
    pub title: Option<bool>,
}

impl Config {
    /// Parse configuration text.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Parse`] when the text is not valid TOML or a
    /// known key has the wrong type, and [`ConfigError::McpServer`] when one
    /// `[mcp_servers.<name>]` entry cannot be used. Unknown keys never fail.
    #[expect(
        clippy::should_implement_trait,
        reason = "the wave API names Config::from_str, and callers should not have to import a trait"
    )]
    pub fn from_str(text: &str) -> Result<Self, ConfigError> {
        let raw: RawConfig = toml::from_str(text)?;
        raw.into_config()
    }

    /// Read `path`, treating a missing file as [`Config::default`].
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Read`] for any read failure other than the file
    /// being absent, [`ConfigError::ParseFile`] when the file is not valid
    /// TOML, and [`ConfigError::McpServer`] when one MCP entry is malformed.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let mut config = match fs::read_to_string(path) {
            Ok(text) => {
                let raw: RawConfig =
                    toml::from_str(&text).map_err(|source| ConfigError::ParseFile {
                        path: path.to_path_buf(),
                        source,
                    })?;
                raw.into_config()?
            }
            Err(source) if source.kind() == ErrorKind::NotFound => Self::default(),
            Err(source) => {
                return Err(ConfigError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        config.credentials = credentials::load(&credentials::path_for(path));
        Ok(config)
    }

    /// The provider `name` resolves to: the user's block over the built-in one,
    /// field by field, or `None` when neither exists.
    ///
    /// Built-in endpoints and key variables cover the well-known providers, so
    /// a config that only sets `[models].default` still runs. A user block
    /// overrides whichever fields it sets and inherits the rest.
    #[must_use]
    pub fn provider(&self, name: &str) -> Option<Provider> {
        let user = self.providers.get(name);
        let mut resolved = match (user, builtin::builtin(name)) {
            (Some(user), Some(base)) => Provider {
                display_name: user.display_name.clone().or(base.display_name),
                class_path: user.class_path.clone().or(base.class_path),
                base_url: user.base_url.clone().or(base.base_url),
                api_key_env: user.api_key_env.clone().or(base.api_key_env),
                models: if user.models.is_empty() {
                    base.models
                } else {
                    user.models.clone()
                },
                protocol: user.protocol.clone().or(base.protocol),
                keyless: user.keyless.or(base.keyless),
                api_key: user.api_key.clone().or(base.api_key),
            },
            (Some(user), None) => user.clone(),
            (None, Some(base)) => base,
            (None, None) => return None,
        };
        if resolved.api_key.is_none() {
            resolved.api_key = self.credentials.get(name).cloned();
        }
        Some(resolved)
    }

    /// Every provider name this config can resolve: user blocks first, then the
    /// built-ins the user did not override, in picker order.
    #[must_use]
    pub fn provider_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.providers.keys().cloned().collect();
        for name in builtin::BUILTIN_NAMES {
            if !self.providers.contains_key(*name) {
                names.push((*name).to_owned());
            }
        }
        names
    }
}

/// The user-level Niki directory: `$NIKI_HOME`, else `$HOME/.niki`.
///
/// With neither variable set the fallback is the relative `.niki`, which keeps
/// the function total without inventing a home directory the process does not
/// have.
#[must_use]
pub fn config_home() -> PathBuf {
    resolve_config_home(env::var_os("NIKI_HOME"), env::var_os("HOME"))
}

fn resolve_config_home(niki_home: Option<OsString>, home: Option<OsString>) -> PathBuf {
    if let Some(root) = non_empty(niki_home) {
        return PathBuf::from(root);
    }
    match non_empty(home) {
        Some(home) => PathBuf::from(home).join(".niki"),
        None => PathBuf::from(".niki"),
    }
}

fn non_empty(value: Option<OsString>) -> Option<OsString> {
    value.filter(|value| !value.is_empty())
}

/// Why a configuration file could not be used.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The file exists but cannot be read.
    #[error("cannot read {path}: {source}")]
    Read {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },
    /// The file is not valid TOML.
    #[error("invalid TOML in {path}: {source}")]
    ParseFile {
        /// The file that failed to parse.
        path: PathBuf,
        /// The parser's diagnostic.
        #[source]
        source: toml::de::Error,
    },
    /// Configuration text is not valid TOML.
    #[error("invalid TOML: {0}")]
    Parse(#[from] toml::de::Error),
    /// One MCP server entry is malformed. The message names the server so the
    /// user can fix the block.
    #[error("mcp server `{server}` in config.toml: {message}")]
    McpServer {
        /// The table name after `mcp_servers.`.
        server: String,
        /// What the entry needs or got wrong.
        message: String,
    },
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawConfig {
    providers: BTreeMap<String, Provider>,
    models: RawModels,
    ui: RawUi,
    mcp_servers: BTreeMap<String, toml::Value>,
    notify: Notify,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawModels {
    default: Option<String>,
    providers: BTreeMap<String, Provider>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawUi {
    theme: Option<String>,
    motion: Option<String>,
    welcome: Option<bool>,
    progress: Option<bool>,
    shimmer: Option<bool>,
    title: Option<bool>,
}

/// One `[mcp_servers.<name>]` table, before validation.
///
/// Every field is optional here so a malformed entry produces a message that
/// names the server; the typed [`McpTransport`] is built only when the shape
/// makes sense.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawMcpServer {
    #[serde(rename = "type", alias = "transport")]
    kind: Option<String>,
    command: Option<String>,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    cwd: Option<String>,
    url: Option<String>,
    headers: BTreeMap<String, String>,
    timeout: Option<i64>,
}

impl RawMcpServer {
    fn into_server(self, name: &str) -> Result<McpServer, ConfigError> {
        let timeout = self.timeout(name)?;
        let kind = self.kind.as_deref().map(normalize_kind);
        match kind.as_deref() {
            Some("stdio") => self.stdio(name, timeout),
            Some("http") => self.http(name, timeout),
            Some(other) => Err(ConfigError::McpServer {
                server: name.to_owned(),
                message: format!(
                    "unsupported transport `{other}`; this build speaks `stdio` and `http`"
                ),
            }),
            None if self.url.is_some() => self.http(name, timeout),
            None if self.command.is_some() => self.stdio(name, timeout),
            None => Err(ConfigError::McpServer {
                server: name.to_owned(),
                message: "add `command = \"…\"` for a stdio server or `url = \"…\"` for an \
                          HTTP server"
                    .to_owned(),
            }),
        }
    }

    fn timeout(&self, name: &str) -> Result<Option<Duration>, ConfigError> {
        match self.timeout {
            None => Ok(None),
            Some(seconds) if seconds > 0 => Ok(Some(Duration::from_secs(seconds as u64))),
            Some(_) => Err(ConfigError::McpServer {
                server: name.to_owned(),
                message: "`timeout` must be a positive number of seconds".to_owned(),
            }),
        }
    }

    fn stdio(self, name: &str, timeout: Option<Duration>) -> Result<McpServer, ConfigError> {
        if self.url.is_some() {
            return Err(ConfigError::McpServer {
                server: name.to_owned(),
                message: "a stdio entry cannot also set `url`; remove it or set \
                          `type = \"http\"`"
                    .to_owned(),
            });
        }
        let Some(command) = self.command.filter(|command| !command.trim().is_empty()) else {
            return Err(ConfigError::McpServer {
                server: name.to_owned(),
                message: "a stdio entry needs a `command`".to_owned(),
            });
        };
        let command = expand(&command, name, "command")?;
        let args = self
            .args
            .iter()
            .map(|arg| expand(arg, name, "args"))
            .collect::<Result<Vec<_>, _>>()?;
        let env = expand_map(self.env, name, "env")?;
        let cwd = self.cwd.filter(|cwd| !cwd.is_empty()).map(PathBuf::from);
        Ok(McpServer {
            transport: McpTransport::Stdio {
                command,
                args,
                env,
                cwd,
            },
            timeout,
        })
    }

    fn http(self, name: &str, timeout: Option<Duration>) -> Result<McpServer, ConfigError> {
        if self.command.is_some() {
            return Err(ConfigError::McpServer {
                server: name.to_owned(),
                message: "an HTTP entry cannot also set `command`; remove it or set \
                          `type = \"stdio\"`"
                    .to_owned(),
            });
        }
        let Some(url) = self.url.filter(|url| !url.trim().is_empty()) else {
            return Err(ConfigError::McpServer {
                server: name.to_owned(),
                message: "an HTTP entry needs a `url`".to_owned(),
            });
        };
        let url = expand(&url, name, "url")?;
        let headers = expand_map(self.headers, name, "headers")?;
        Ok(McpServer {
            transport: McpTransport::Http { url, headers },
            timeout,
        })
    }
}

/// Expand `${VAR}` references and `$$` escapes in one configured string.
fn expand(input: &str, server: &str, field: &str) -> Result<String, ConfigError> {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(dollar) = rest.find('$') {
        output.push_str(&rest[..dollar]);
        let after = &rest[dollar + 1..];
        if let Some(literal) = after.strip_prefix('$') {
            output.push('$');
            rest = literal;
        } else if let Some(braced) = after.strip_prefix('{') {
            let Some(close) = braced.find('}') else {
                return Err(ConfigError::McpServer {
                    server: server.to_owned(),
                    message: format!(
                        "unterminated `${{` in `{field}`; write `$${{` for a literal `${{`"
                    ),
                });
            };
            let name = &braced[..close];
            if name.is_empty() {
                return Err(ConfigError::McpServer {
                    server: server.to_owned(),
                    message: format!("empty variable name `${{}}` in `{field}`"),
                });
            }
            let value = env::var(name).map_err(|_| ConfigError::McpServer {
                server: server.to_owned(),
                message: format!(
                    "environment variable `{name}` in `{field}` is not set or is not valid UTF-8"
                ),
            })?;
            output.push_str(&value);
            rest = &braced[close + 1..];
        } else {
            output.push('$');
            rest = after;
        }
    }
    output.push_str(rest);
    Ok(output)
}

fn expand_map(
    values: BTreeMap<String, String>,
    server: &str,
    field: &str,
) -> Result<BTreeMap<String, String>, ConfigError> {
    values
        .into_iter()
        .map(|(key, value)| Ok((key, expand(&value, server, field)?)))
        .collect()
}

fn normalize_kind(kind: &str) -> String {
    match kind.trim().to_ascii_lowercase().as_str() {
        "streamable_http" | "streamable-http" => "http".to_owned(),
        other => other.to_owned(),
    }
}

impl RawConfig {
    fn into_config(self) -> Result<Config, ConfigError> {
        let mut providers = self.models.providers;
        // The README documents `[providers.<name>]`; the reader also accepts
        // `[models.providers.<name>]`. A name present in both resolves to the
        // documented top-level block.
        providers.extend(self.providers);

        let mut mcp_servers = BTreeMap::new();
        for (name, value) in self.mcp_servers {
            if name.trim().is_empty() {
                return Err(ConfigError::McpServer {
                    server: name,
                    message: "the server name is empty".to_owned(),
                });
            }
            let raw: RawMcpServer = value.try_into().map_err(|source| ConfigError::McpServer {
                server: name.clone(),
                message: source.to_string(),
            })?;
            mcp_servers.insert(name.clone(), raw.into_server(&name)?);
        }

        Ok(Config {
            providers,
            default_model: self.models.default,
            theme: self.ui.theme,
            motion: self.ui.motion,
            effects: Effects {
                welcome: self.ui.welcome,
                progress: self.ui.progress,
                shimmer: self.ui.shimmer,
                title: self.ui.title,
            },
            mcp_servers,
            notify: self.notify,
            credentials: BTreeMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const README_PROVIDER: &str = r#"
[providers.kimi]
display_name = "Kimi"
class_path = "openai:Chat"
base_url = "https://api.moonshot.ai/v1"
api_key_env = "MOONSHOT_API_KEY"
models = ["kimi-k3", "kimi-k2.7-code", "kimi-k2.6"]
"#;

    fn missing_path() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        env::temp_dir().join(format!(
            "niki-config-absent-{}-{nanos}.toml",
            std::process::id()
        ))
    }

    fn provider(class_path: Option<&str>, protocol: Option<&str>) -> Provider {
        Provider {
            class_path: class_path.map(str::to_owned),
            protocol: protocol.map(str::to_owned),
            ..Provider::default()
        }
    }

    #[test]
    fn readme_provider_block_parses() -> Result<(), ConfigError> {
        let config = Config::from_str(README_PROVIDER)?;
        let expected = Provider {
            display_name: Some("Kimi".to_owned()),
            class_path: Some("openai:Chat".to_owned()),
            base_url: Some("https://api.moonshot.ai/v1".to_owned()),
            api_key_env: Some("MOONSHOT_API_KEY".to_owned()),
            models: vec![
                "kimi-k3".to_owned(),
                "kimi-k2.7-code".to_owned(),
                "kimi-k2.6".to_owned(),
            ],
            protocol: None,
            keyless: None,
            api_key: None,
        };
        assert_eq!(config.providers.get("kimi"), Some(&expected));
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.default_model, None);
        assert_eq!(config.theme, None);
        Ok(())
    }

    #[test]
    fn unknown_keys_are_ignored() -> Result<(), ConfigError> {
        let config = Config::from_str(
            r#"
top_level_mystery = 7

[providers.kimi]
display_name = "Kimi"
short_name = "Kimi"
params = { temperature = 0 }
models = ["kimi-k3"]

[models]
default = "kimi:kimi-k3"
mystery = true

[ui]
theme = "niki"
unknown = "value"

[not_a_known_table]
anything = "goes"
"#,
        )?;
        let provider = config.providers.get("kimi");
        assert_eq!(
            provider.and_then(|provider| provider.display_name.as_deref()),
            Some("Kimi")
        );
        assert_eq!(config.default_model.as_deref(), Some("kimi:kimi-k3"));
        assert_eq!(config.theme.as_deref(), Some("niki"));
        Ok(())
    }

    #[test]
    fn the_ui_block_carries_motion_and_effect_switches() -> Result<(), ConfigError> {
        let config =
            Config::from_str("[ui]\nmotion = \"reduced\"\nwelcome = false\nshimmer = false\n")?;
        assert_eq!(config.motion.as_deref(), Some("reduced"));
        assert_eq!(
            config.effects,
            Effects {
                welcome: Some(false),
                progress: None,
                shimmer: Some(false),
                title: None,
            }
        );

        let plain = Config::from_str("[ui]\ntheme = \"mono\"\n")?;
        assert_eq!(plain.motion, None);
        assert_eq!(plain.effects, Effects::default());
        Ok(())
    }

    #[test]
    fn models_providers_table_is_read_and_top_level_wins() -> Result<(), ConfigError> {
        let config = Config::from_str(
            r#"
[models.providers.legacy]
class_path = "openai:Chat"

[models.providers.shared]
display_name = "Read from the models table"

[providers.shared]
display_name = "Read from the providers table"
"#,
        )?;
        assert!(config.providers.contains_key("legacy"));
        assert_eq!(
            config
                .providers
                .get("shared")
                .and_then(|provider| provider.display_name.as_deref()),
            Some("Read from the providers table")
        );
        Ok(())
    }

    #[test]
    fn stdio_mcp_servers_parse() -> Result<(), ConfigError> {
        let config = Config::from_str(
            r#"
[mcp_servers.fs]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
env = { LOG_LEVEL = "debug" }
cwd = "/tmp/work"
"#,
        )?;
        let server = config.mcp_servers.get("fs").expect("fs");
        let McpTransport::Stdio {
            command,
            args,
            env,
            cwd,
        } = &server.transport
        else {
            panic!("expected stdio, got {:?}", server.transport);
        };
        assert_eq!(command, "npx");
        assert_eq!(args.len(), 3);
        assert_eq!(env.get("LOG_LEVEL").map(String::as_str), Some("debug"));
        assert_eq!(cwd.as_deref(), Some(Path::new("/tmp/work")));
        Ok(())
    }

    #[test]
    fn http_mcp_servers_parse_and_a_url_selects_the_transport() -> Result<(), ConfigError> {
        let config = Config::from_str(
            r#"
[mcp_servers.docs]
url = "https://example.com/mcp"
headers = { Authorization = "Bearer token" }
"#,
        )?;
        let server = config.mcp_servers.get("docs").expect("docs");
        let McpTransport::Http { url, headers } = &server.transport else {
            panic!("expected http, got {:?}", server.transport);
        };
        assert_eq!(url, "https://example.com/mcp");
        assert_eq!(
            headers.get("Authorization").map(String::as_str),
            Some("Bearer token")
        );
        Ok(())
    }

    #[test]
    fn a_notify_block_parses_and_args_are_optional() -> Result<(), ConfigError> {
        let config = Config::from_str(
            r#"
[notify]
program = "/usr/local/bin/notify-turn"
args = ["--title", "Niki"]
"#,
        )?;
        assert_eq!(
            config.notify.program.as_deref(),
            Some("/usr/local/bin/notify-turn")
        );
        assert_eq!(config.notify.args, vec!["--title", "Niki"]);

        let without_args = Config::from_str("[notify]\nprogram = \"notify\"\n")?;
        assert!(without_args.notify.args.is_empty());

        let absent = Config::from_str("")?;
        assert_eq!(absent.notify, Notify::default());
        assert_eq!(absent.notify.program, None);
        Ok(())
    }

    #[test]
    fn an_mcp_entry_with_no_transport_is_an_error_naming_the_server() {
        let error = Config::from_str("[mcp_servers.broken]\nargs = [\"x\"]\n").expect_err("error");
        assert!(matches!(error, ConfigError::McpServer { ref server, .. } if server == "broken"));
        let text = error.to_string();
        assert!(text.contains("broken"), "{text}");
        assert!(text.contains("command"), "{text}");
    }

    #[test]
    fn an_mcp_entry_with_the_wrong_field_type_names_the_server() {
        let error = Config::from_str("[mcp_servers.broken]\ncommand = \"x\"\nargs = 3\n")
            .expect_err("error");
        assert!(matches!(error, ConfigError::McpServer { ref server, .. } if server == "broken"));
        assert!(error.to_string().contains("broken"), "{error}");
    }

    #[test]
    fn an_mcp_entry_cannot_set_both_command_and_url() {
        let error = Config::from_str(
            "[mcp_servers.broken]\ncommand = \"x\"\nurl = \"https://example.com/mcp\"\n",
        )
        .expect_err("error");
        assert!(matches!(error, ConfigError::McpServer { ref server, .. } if server == "broken"));
    }

    #[test]
    fn an_explicit_mcp_type_overrides_the_field_inference() -> Result<(), ConfigError> {
        let config = Config::from_str(
            r#"
[mcp_servers.remote]
type = "streamable_http"
url = "https://example.com/mcp"

[mcp_servers.local]
type = "stdio"
command = "server"
"#,
        )?;
        assert!(matches!(
            config.mcp_servers["remote"].transport,
            McpTransport::Http { .. }
        ));
        assert!(matches!(
            config.mcp_servers["local"].transport,
            McpTransport::Stdio { .. }
        ));
        Ok(())
    }

    #[test]
    fn mcp_values_expand_environment_variables_in_every_place() -> Result<(), ConfigError> {
        let path = env::var("PATH").expect("PATH is set for the test process");
        let config = Config::from_str(
            r#"
[mcp_servers.local]
command = "${PATH}"
args = ["--root=${PATH}"]
env = { ROOT = "${PATH}" }

[mcp_servers.remote]
url = "https://example.com/${PATH}"
headers = { Authorization = "Bearer ${PATH}" }
"#,
        )?;
        let McpTransport::Stdio {
            command, args, env, ..
        } = &config.mcp_servers["local"].transport
        else {
            panic!("expected stdio");
        };
        assert_eq!(command, &path);
        let root_arg = format!("--root={path}");
        assert_eq!(
            args.iter().map(String::as_str).collect::<Vec<_>>(),
            vec![root_arg.as_str()]
        );
        assert_eq!(env.get("ROOT"), Some(&path));

        let McpTransport::Http { url, headers } = &config.mcp_servers["remote"].transport else {
            panic!("expected http");
        };
        assert_eq!(url, &format!("https://example.com/{path}"));
        assert_eq!(
            headers.get("Authorization"),
            Some(&format!("Bearer {path}"))
        );
        Ok(())
    }

    #[test]
    fn a_double_dollar_is_one_literal_dollar() -> Result<(), ConfigError> {
        let config = Config::from_str(
            r#"
[mcp_servers.local]
command = "server"
args = ["price=$$5", "literal=$${NOT_A_VARIABLE}", "tail=$$"]
"#,
        )?;
        let McpTransport::Stdio { args, .. } = &config.mcp_servers["local"].transport else {
            panic!("expected stdio");
        };
        assert_eq!(
            args.iter().map(String::as_str).collect::<Vec<_>>(),
            vec!["price=$5", "literal=${NOT_A_VARIABLE}", "tail=$"]
        );
        Ok(())
    }

    #[test]
    fn an_unset_variable_is_an_error_naming_the_server_and_variable() {
        let error = Config::from_str(
            r#"
[mcp_servers.tokened]
url = "https://example.com/${NIKI_CONFIG_TEST_SURELY_UNSET}"
"#,
        )
        .expect_err("an unset variable");
        assert!(
            matches!(error, ConfigError::McpServer { ref server, .. } if server == "tokened"),
            "{error:?}"
        );
        let text = error.to_string();
        assert!(text.contains("tokened"), "{text}");
        assert!(text.contains("NIKI_CONFIG_TEST_SURELY_UNSET"), "{text}");
    }

    #[test]
    fn an_unterminated_reference_is_an_error_naming_the_server() {
        let error = Config::from_str("[mcp_servers.broken]\ncommand = \"${PATH\"\n")
            .expect_err("an unterminated reference");
        assert!(
            matches!(error, ConfigError::McpServer { ref server, .. } if server == "broken"),
            "{error:?}"
        );
        assert!(error.to_string().contains("unterminated"), "{error}");
    }

    #[test]
    fn an_mcp_timeout_must_be_a_positive_number_of_seconds() {
        for value in ["0", "-1", "-30"] {
            let error = Config::from_str(&format!(
                "[mcp_servers.slow]\ncommand = \"server\"\ntimeout = {value}\n"
            ))
            .expect_err("a non-positive timeout");
            assert!(
                matches!(error, ConfigError::McpServer { ref server, .. } if server == "slow"),
                "{error:?}"
            );
            assert!(error.to_string().contains("timeout"), "{error}");
        }
    }

    #[test]
    fn an_mcp_timeout_is_optional_and_read_in_seconds() -> Result<(), ConfigError> {
        let config = Config::from_str("[mcp_servers.slow]\ncommand = \"server\"\ntimeout = 30\n")?;
        assert_eq!(
            config.mcp_servers["slow"].timeout,
            Some(Duration::from_secs(30))
        );

        let absent = Config::from_str("[mcp_servers.slow]\ncommand = \"server\"\n")?;
        assert_eq!(absent.mcp_servers["slow"].timeout, None);
        Ok(())
    }

    #[test]
    fn a_missing_file_is_the_default_config() -> Result<(), ConfigError> {
        let config = Config::load(&missing_path())?;
        assert_eq!(config, Config::default());
        Ok(())
    }

    #[test]
    fn malformed_toml_is_an_error() {
        let error = Config::from_str("providers = [").err();
        assert!(matches!(error, Some(ConfigError::Parse(_))));

        let wrong_type = Config::from_str("models = 3").err();
        assert!(matches!(wrong_type, Some(ConfigError::Parse(_))));
    }

    #[test]
    fn protocol_is_inferred_from_class_path() {
        assert_eq!(
            provider(Some("openai:Chat"), None).protocol(),
            Protocol::OpenAi
        );
        assert_eq!(
            provider(Some("anthropic:Chat"), None).protocol(),
            Protocol::Anthropic
        );
        assert_eq!(provider(None, None).protocol(), Protocol::OpenAi);
        assert_eq!(
            provider(Some("custom:Client"), None).protocol(),
            Protocol::OpenAi
        );
    }

    #[test]
    fn an_explicit_protocol_key_overrides_the_class_path() {
        assert_eq!(
            provider(Some("openai:Chat"), Some("anthropic")).protocol(),
            Protocol::Anthropic
        );
        assert_eq!(
            provider(Some("anthropic:Chat"), Some("openai")).protocol(),
            Protocol::OpenAi
        );
        assert_eq!(
            provider(Some("openai:Chat"), Some("nonsense")).protocol(),
            Protocol::OpenAi
        );
    }

    #[test]
    fn a_builtin_provider_resolves_without_a_block() -> Result<(), ConfigError> {
        let config = Config::from_str("[models]\ndefault = \"anthropic:claude\"\n")?;
        let provider = config.provider("anthropic").expect("built-in");
        assert_eq!(
            provider.base_url.as_deref(),
            Some("https://api.anthropic.com")
        );
        assert_eq!(provider.api_key_env.as_deref(), Some("ANTHROPIC_API_KEY"));
        assert_eq!(provider.protocol(), Protocol::Anthropic);
        assert!(!provider.keyless());
        Ok(())
    }

    #[test]
    fn a_local_builtin_needs_no_credential() -> Result<(), ConfigError> {
        let config = Config::from_str("")?;
        let provider = config.provider("ollama").expect("built-in");
        assert!(provider.keyless());
        assert_eq!(
            provider.base_url.as_deref(),
            Some("http://localhost:11434/v1")
        );
        Ok(())
    }

    #[test]
    fn a_user_block_overrides_fields_and_inherits_the_rest() -> Result<(), ConfigError> {
        let config = Config::from_str(
            r#"
[providers.anthropic]
base_url = "https://gateway.example.com/anthropic"
models = ["claude-custom"]
"#,
        )?;
        let provider = config.provider("anthropic").expect("merged");
        assert_eq!(
            provider.base_url.as_deref(),
            Some("https://gateway.example.com/anthropic")
        );
        assert_eq!(provider.api_key_env.as_deref(), Some("ANTHROPIC_API_KEY"));
        assert_eq!(provider.models, vec!["claude-custom".to_owned()]);
        assert_eq!(provider.protocol(), Protocol::Anthropic);
        Ok(())
    }

    #[test]
    fn an_unknown_provider_resolves_to_nothing() -> Result<(), ConfigError> {
        let config = Config::from_str("")?;
        assert_eq!(config.provider("nope"), None);
        assert!(config.provider_names().contains(&"anthropic".to_owned()));
        assert!(config.provider_names().contains(&"ollama".to_owned()));
        Ok(())
    }

    #[test]
    fn config_home_prefers_niki_home_then_home() {
        assert_eq!(
            resolve_config_home(Some("/tmp/niki".into()), Some("/home/user".into())),
            PathBuf::from("/tmp/niki")
        );
        assert_eq!(
            resolve_config_home(None, Some("/home/user".into())),
            PathBuf::from("/home/user/.niki")
        );
        assert_eq!(
            resolve_config_home(Some("".into()), Some("/home/user".into())),
            PathBuf::from("/home/user/.niki")
        );
        assert_eq!(resolve_config_home(None, None), PathBuf::from(".niki"));
    }

    #[test]
    fn debug_names_the_key_variable_without_its_value() {
        let provider = Provider {
            api_key_env: Some("NIKI_TEST_SURELY_UNSET_API_KEY".to_owned()),
            ..Provider::default()
        };
        let output = format!("{provider:?}");
        assert!(output.contains("NIKI_TEST_SURELY_UNSET_API_KEY"));
        assert!(output.contains("api_key_env_set: false"));
        assert!(!output.contains("sk-"));

        let set = Provider {
            api_key_env: Some("PATH".to_owned()),
            ..Provider::default()
        };
        if let Ok(path) = env::var("PATH") {
            let output = format!("{set:?}");
            assert!(output.contains("api_key_env_set: true"));
            assert!(!output.contains(&path));
        }
    }

    #[test]
    fn debug_of_a_config_does_not_leak_a_key() {
        let mut config = Config::default();
        config.providers.insert(
            "kimi".to_owned(),
            Provider {
                api_key_env: Some("PATH".to_owned()),
                ..Provider::default()
            },
        );
        let output = format!("{config:?}");
        if let Ok(path) = env::var("PATH") {
            assert!(!output.contains(&path));
        }
    }
}
