//! The connect screen: pick a provider, store a key, name a model.
//!
//! This is the in-terminal counterpart of editing `config.toml` by hand. It
//! lists the providers the product ships, plus one custom endpoint, and walks a
//! person through the two things a provider needs: a credential and a model.
//! The key is written to `credentials.toml`, never to `config.toml`; the model
//! becomes `[models].default`.
//!
//! The state machine is pure so it is tested by driving keys, with no terminal
//! involved (RULES.md 8).

use std::env;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use niki_config::{Config, Protocol};

use crate::Outcome;

/// One endpoint the connect screen offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    /// The `[providers.<name>]` table name, or `custom`.
    pub name: String,
    /// What the list shows.
    pub label: String,
    /// The endpoint a request goes to.
    pub base_url: String,
    /// The wire protocol the endpoint speaks.
    pub protocol: Protocol,
    /// The variable a key would otherwise come from.
    pub key_var: Option<String>,
    /// Whether the endpoint accepts requests with no credential.
    pub keyless: bool,
    /// Whether a key is already stored or present in the environment.
    pub connected: bool,
    /// Model ids worth offering as the placeholder.
    pub suggestions: &'static [&'static str],
    /// Whether this row asks for a base URL, which the shipped rows do not.
    pub custom: bool,
}

/// What the person chose, once the walk is over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectRequest {
    /// The provider table name.
    pub name: String,
    /// The endpoint, when the person typed one.
    pub base_url: Option<String>,
    /// `openai` or `anthropic`.
    pub protocol: Protocol,
    /// The pasted key, when there was one.
    pub key: Option<String>,
    /// Whether the endpoint needs no credential.
    pub keyless: bool,
    /// The model id, without the `provider:` prefix.
    pub model: String,
}

/// Which step the walk is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Picking a provider.
    Choose,
    /// Typing the endpoint for a custom provider.
    BaseUrl,
    /// Pasting the key.
    Key,
    /// Naming the model.
    Model,
}

/// The connect screen's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectModal {
    entries: Vec<CatalogEntry>,
    selected: usize,
    stage: Stage,
    draft: String,
    base_url: Option<String>,
    key: Option<String>,
    keyless: bool,
    error: Option<String>,
}

impl ConnectModal {
    /// Build the catalog from the shipped providers and the current keys.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let mut entries: Vec<CatalogEntry> = niki_config::builtin_providers()
            .into_iter()
            .map(|(name, _)| {
                let provider = config.provider(name).unwrap_or_default();
                let connected = provider.api_key.is_some()
                    || provider
                        .api_key_env
                        .as_deref()
                        .is_some_and(|var| env::var_os(var).is_some_and(|value| !value.is_empty()));
                let protocol = provider.protocol();
                let keyless = provider.keyless();
                CatalogEntry {
                    name: name.to_owned(),
                    label: label(name),
                    base_url: provider.base_url.unwrap_or_default(),
                    protocol,
                    key_var: provider.api_key_env,
                    keyless,
                    connected,
                    suggestions: suggestions(name),
                    custom: false,
                }
            })
            .collect();
        entries.push(CatalogEntry {
            name: "custom".to_owned(),
            label: "Custom endpoint".to_owned(),
            base_url: String::new(),
            protocol: Protocol::OpenAi,
            key_var: Some("CUSTOM_API_KEY".to_owned()),
            keyless: false,
            connected: false,
            suggestions: &[],
            custom: true,
        });
        Self {
            entries,
            selected: 0,
            stage: Stage::Choose,
            draft: String::new(),
            base_url: None,
            key: None,
            keyless: false,
            error: None,
        }
    }

    /// The rows the list draws.
    #[must_use]
    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }

    /// The highlighted row.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The step the walk is on.
    #[must_use]
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// What has been typed on the current step.
    #[must_use]
    pub fn draft(&self) -> &str {
        &self.draft
    }

    /// The error line for the current step, when the last Enter was refused.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The highlighted row.
    #[must_use]
    pub fn highlighted(&self) -> Option<&CatalogEntry> {
        self.entries.get(self.selected)
    }

    /// The model placeholder for the current provider.
    #[must_use]
    pub fn placeholder(&self) -> &'static str {
        self.highlighted()
            .and_then(|entry| entry.suggestions.first().copied())
            .unwrap_or("model id")
    }

    /// Handle one key press.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        match self.stage {
            Stage::Choose => self.choose_key(key),
            Stage::BaseUrl | Stage::Key | Stage::Model => self.text_key(key),
        }
    }

    fn choose_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Esc => Outcome::Close,
            KeyCode::Enter => {
                let Some((custom, keyless)) = self
                    .highlighted()
                    .map(|entry| (entry.custom, entry.keyless))
                else {
                    return Outcome::Close;
                };
                self.error = None;
                self.draft.clear();
                self.key = None;
                self.base_url = None;
                self.keyless = keyless;
                if custom {
                    self.stage = Stage::BaseUrl;
                } else if keyless {
                    self.stage = Stage::Model;
                } else {
                    self.stage = Stage::Key;
                }
                Outcome::Stay
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.step(1);
                Outcome::Stay
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.step(-1);
                Outcome::Stay
            }
            KeyCode::Home => {
                self.selected = 0;
                Outcome::Stay
            }
            KeyCode::End => {
                self.selected = self.entries.len().saturating_sub(1);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }

    fn text_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Esc => {
                self.error = None;
                self.draft.clear();
                self.stage = match self.stage {
                    Stage::BaseUrl => Stage::Choose,
                    Stage::Key => {
                        if self.highlighted().is_some_and(|entry| entry.custom) {
                            Stage::BaseUrl
                        } else {
                            Stage::Choose
                        }
                    }
                    Stage::Model => {
                        if self.keyless {
                            Stage::Choose
                        } else {
                            Stage::Key
                        }
                    }
                    Stage::Choose => Stage::Choose,
                };
                Outcome::Stay
            }
            KeyCode::Enter => self.advance(),
            KeyCode::Backspace => {
                self.draft.pop();
                Outcome::Stay
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => Outcome::Stay,
            KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.stage == Stage::Model || self.draft.len() < 4096 {
                    self.draft.push(character);
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }

    /// Take the current field and move to the next step, or finish.
    fn advance(&mut self) -> Outcome {
        self.error = None;
        match self.stage {
            Stage::Choose => Outcome::Stay,
            Stage::BaseUrl => {
                let url = self.draft.trim().to_owned();
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    self.error =
                        Some("the endpoint must start with http:// or https://".to_owned());
                    return Outcome::Stay;
                }
                self.base_url = Some(url);
                self.draft.clear();
                self.stage = Stage::Key;
                Outcome::Stay
            }
            Stage::Key => {
                let key = self.draft.trim().to_owned();
                self.key = (!key.is_empty()).then_some(key);
                if self.key.is_none() && self.highlighted().is_some_and(|entry| entry.custom) {
                    self.keyless = true;
                }
                self.draft.clear();
                self.stage = Stage::Model;
                Outcome::Stay
            }
            Stage::Model => {
                let model = self.draft.trim().to_owned();
                if model.is_empty() {
                    self.error = Some("name the model id the provider exposes".to_owned());
                    return Outcome::Stay;
                }
                if model.contains(char::is_whitespace) {
                    self.error = Some("a model id has no spaces".to_owned());
                    return Outcome::Stay;
                }
                let Some(entry) = self.highlighted() else {
                    return Outcome::Close;
                };
                Outcome::Connected(ConnectRequest {
                    name: entry.name.clone(),
                    base_url: self.base_url.clone(),
                    protocol: entry.protocol,
                    key: self.key.clone(),
                    keyless: self.keyless,
                    model,
                })
            }
        }
    }

    fn step(&mut self, direction: isize) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as isize;
        self.selected = ((self.selected as isize + direction).rem_euclid(len)) as usize;
    }
}

/// A human label for a shipped provider name.
fn label(name: &str) -> String {
    match name {
        "openai" => "OpenAI".to_owned(),
        "xai" => "xAI".to_owned(),
        "google_genai" => "Google Gemini".to_owned(),
        "mistralai" => "Mistral".to_owned(),
        "ollama" => "Ollama (local)".to_owned(),
        "openrouter" => "OpenRouter".to_owned(),
        other => {
            let mut characters = other.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        }
    }
}

/// Model ids worth showing as a placeholder for a shipped provider.
fn suggestions(name: &str) -> &'static [&'static str] {
    match name {
        "anthropic" => &["claude-sonnet-4-5"],
        "openai" => &["gpt-5.1-codex"],
        "google_genai" => &["gemini-2.5-pro"],
        "groq" => &["llama-3.3-70b-versatile"],
        "openrouter" => &["anthropic/claude-sonnet-4.5"],
        "nvidia" => &["nvidia/llama-3.3-nemotron-super-49b-v1"],
        "deepseek" => &["deepseek-chat"],
        "mistralai" => &["mistral-large-latest"],
        "xai" => &["grok-4"],
        "together" => &["meta-llama/Llama-3.3-70B-Instruct-Turbo"],
        "ollama" => &["qwen2.5-coder:7b"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn typed(modal: &mut ConnectModal, text: &str) {
        for character in text.chars() {
            modal.handle_key(key(KeyCode::Char(character)));
        }
    }

    fn modal() -> ConnectModal {
        ConnectModal::new(&Config::default())
    }

    #[test]
    fn the_catalog_lists_the_shipped_providers_and_a_custom_endpoint() {
        let modal = modal();
        let names: Vec<&str> = modal
            .entries()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert!(names.contains(&"anthropic"));
        assert!(names.contains(&"groq"));
        assert_eq!(names.last(), Some(&"custom"));
    }

    #[test]
    fn picking_a_hosted_provider_asks_for_the_key_then_the_model() {
        let mut modal = modal();
        modal.selected = 0;
        assert_eq!(modal.handle_key(key(KeyCode::Enter)), Outcome::Stay);
        assert_eq!(modal.stage(), Stage::Key);

        typed(&mut modal, "sk-test");
        modal.handle_key(key(KeyCode::Enter));
        assert_eq!(modal.stage(), Stage::Model);

        typed(&mut modal, "claude-sonnet-4-5");
        let outcome = modal.handle_key(key(KeyCode::Enter));
        let Outcome::Connected(request) = outcome else {
            panic!("expected a connect request, got {outcome:?}");
        };
        assert_eq!(request.name, "anthropic");
        assert_eq!(request.key.as_deref(), Some("sk-test"));
        assert_eq!(request.model, "claude-sonnet-4-5");
    }

    #[test]
    fn an_empty_key_means_the_environment_variable() {
        let mut modal = modal();
        modal.handle_key(key(KeyCode::Enter));
        modal.handle_key(key(KeyCode::Enter));
        assert_eq!(modal.stage(), Stage::Model);
        typed(&mut modal, "m");
        let Outcome::Connected(request) = modal.handle_key(key(KeyCode::Enter)) else {
            panic!("expected a connect request");
        };
        assert_eq!(request.key, None);
    }

    #[test]
    fn a_local_provider_skips_the_key_step() {
        let mut modal = modal();
        let ollama = modal
            .entries()
            .iter()
            .position(|entry| entry.name == "ollama")
            .expect("ollama is shipped");
        modal.selected = ollama;
        modal.handle_key(key(KeyCode::Enter));
        assert_eq!(modal.stage(), Stage::Model);
    }

    #[test]
    fn a_custom_endpoint_asks_for_the_url_first_and_refuses_a_bare_word() {
        let mut modal = modal();
        let custom = modal.entries().len() - 1;
        modal.selected = custom;
        modal.handle_key(key(KeyCode::Enter));
        assert_eq!(modal.stage(), Stage::BaseUrl);

        typed(&mut modal, "example.test");
        modal.handle_key(key(KeyCode::Enter));
        assert_eq!(modal.stage(), Stage::BaseUrl, "a bare host is refused");
        assert!(modal.error().is_some());

        for _ in 0..12 {
            modal.handle_key(key(KeyCode::Backspace));
        }
        typed(&mut modal, "https://example.test/v1");
        modal.handle_key(key(KeyCode::Enter));
        assert_eq!(modal.stage(), Stage::Key);
    }

    #[test]
    fn the_model_step_refuses_an_empty_id() {
        let mut modal = modal();
        modal.handle_key(key(KeyCode::Enter));
        modal.handle_key(key(KeyCode::Enter));
        modal.handle_key(key(KeyCode::Enter));
        assert_eq!(modal.stage(), Stage::Model);
        assert!(modal.error().is_some());
    }

    #[test]
    fn escape_steps_back_and_then_closes() {
        let mut modal = modal();
        modal.handle_key(key(KeyCode::Enter));
        assert_eq!(modal.stage(), Stage::Key);
        modal.handle_key(key(KeyCode::Esc));
        assert_eq!(modal.stage(), Stage::Choose);
        assert_eq!(modal.handle_key(key(KeyCode::Esc)), Outcome::Close);
    }

    #[test]
    fn the_highlight_wraps() {
        let mut modal = modal();
        let last = modal.entries().len() - 1;
        modal.handle_key(key(KeyCode::Up));
        assert_eq!(modal.selected(), last);
        modal.handle_key(key(KeyCode::Down));
        assert_eq!(modal.selected(), 0);
    }
}
