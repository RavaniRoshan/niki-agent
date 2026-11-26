//! The user's `~/.niki/config.toml`, read the way the Python product reads it.
//!
//! ADR-005: one config file serves both binaries during the port. The table
//! names are the ones the Python product already uses — `[providers.<name>]`
//! for user providers (also accepted under `[models.providers.<name>]`),
//! `[models].default` for the default model, and `[ui].theme` for the theme.
//! Unknown keys are ignored, so a file written for a newer Python release
//! still loads. This crate only reads; the Rust binary does not write the file
//! in W1.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::{env, fs};

use serde::Deserialize;

mod builtin;

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
/// Every field the Python product treats as optional stays optional here: a
/// block may name only `base_url` and rely on the protocol defaults.
#[derive(Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Provider {
    /// Human-readable name shown in the model picker.
    pub display_name: Option<String>,
    /// Python class path, read as the protocol selector.
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
            .field("models", &self.models)
            .field("protocol", &self.protocol())
            .field("keyless", &self.keyless())
            .finish()
    }
}

/// The parts of `config.toml` this wave consumes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// Provider blocks, keyed by the table name after `providers.`.
    pub providers: BTreeMap<String, Provider>,
    /// `[models].default`, a `provider:model` spec.
    pub default_model: Option<String>,
    /// `[ui].theme`.
    pub theme: Option<String>,
}

impl Config {
    /// Parse configuration text.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Parse`] when the text is not valid TOML or a
    /// known key has the wrong type. Unknown keys never fail.
    #[expect(
        clippy::should_implement_trait,
        reason = "the wave API names Config::from_str, and callers should not have to import a trait"
    )]
    pub fn from_str(text: &str) -> Result<Self, ConfigError> {
        let raw: RawConfig = toml::from_str(text)?;
        Ok(raw.into_config())
    }

    /// Read `path`, treating a missing file as [`Config::default`].
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Read`] for any read failure other than the file
    /// being absent, and [`ConfigError::ParseFile`] when the file is not valid
    /// TOML.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match fs::read_to_string(path) {
            Ok(text) => {
                let raw: RawConfig =
                    toml::from_str(&text).map_err(|source| ConfigError::ParseFile {
                        path: path.to_path_buf(),
                        source,
                    })?;
                Ok(raw.into_config())
            }
            Err(source) if source.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    /// The provider `name` resolves to: the user's block over the built-in one,
    /// field by field, or `None` when neither exists.
    ///
    /// The Python product ships endpoints and key variables for its well-known
    /// providers, so a config that only sets `[models].default` still runs. A
    /// user block overrides whichever fields it sets and inherits the rest.
    #[must_use]
    pub fn provider(&self, name: &str) -> Option<Provider> {
        let user = self.providers.get(name);
        match (user, builtin::builtin(name)) {
            (Some(user), Some(base)) => Some(Provider {
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
            }),
            (Some(user), None) => Some(user.clone()),
            (None, Some(base)) => Some(base),
            (None, None) => None,
        }
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
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawConfig {
    providers: BTreeMap<String, Provider>,
    models: RawModels,
    ui: RawUi,
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
}

impl RawConfig {
    fn into_config(self) -> Config {
        let mut providers = self.models.providers;
        // The README documents `[providers.<name>]`; the Python reader also
        // accepts `[models.providers.<name>]`. A name present in both resolves
        // to the documented top-level block.
        providers.extend(self.providers);
        Config {
            providers,
            default_model: self.models.default,
            theme: self.ui.theme,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const README_PROVIDER: &str = r#"
[providers.kimi]
display_name = "Kimi"
class_path = "langchain_openai:ChatOpenAI"
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
            class_path: Some("langchain_openai:ChatOpenAI".to_owned()),
            base_url: Some("https://api.moonshot.ai/v1".to_owned()),
            api_key_env: Some("MOONSHOT_API_KEY".to_owned()),
            models: vec![
                "kimi-k3".to_owned(),
                "kimi-k2.7-code".to_owned(),
                "kimi-k2.6".to_owned(),
            ],
            protocol: None,
            keyless: None,
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
    fn models_providers_table_is_read_and_top_level_wins() -> Result<(), ConfigError> {
        let config = Config::from_str(
            r#"
[models.providers.legacy]
class_path = "langchain_openai:ChatOpenAI"

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
            provider(Some("langchain_openai:ChatOpenAI"), None).protocol(),
            Protocol::OpenAi
        );
        assert_eq!(
            provider(Some("langchain_anthropic:ChatAnthropic"), None).protocol(),
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
            provider(Some("langchain_openai:ChatOpenAI"), Some("anthropic")).protocol(),
            Protocol::Anthropic
        );
        assert_eq!(
            provider(Some("langchain_anthropic:ChatAnthropic"), Some("openai")).protocol(),
            Protocol::OpenAi
        );
        assert_eq!(
            provider(Some("langchain_openai:ChatOpenAI"), Some("nonsense")).protocol(),
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
