use crossterm::event::{KeyCode, KeyEvent};
use niki_config::Config;

use crate::Outcome;

/// One model the configuration offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelChoice {
    /// `provider:model`, as `[models].default` and `--model` write it.
    pub spec: String,
    /// Provider name to show beside it, from `display_name`.
    pub provider: String,
}

/// The models `Config::providers` offers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelPicker {
    choices: Vec<ModelChoice>,
    selected: usize,
}

impl ModelPicker {
    /// Read every `provider:model` pair the configuration lists.
    ///
    /// The default model comes first, so a person who opens the picker and
    /// presses Enter changes nothing.
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        let mut choices = Vec::new();
        if let Some(default) = &config.default_model {
            push(
                &mut choices,
                default.clone(),
                provider_label(config, default),
            );
        }
        for (name, provider) in &config.providers {
            let label = provider
                .display_name
                .clone()
                .unwrap_or_else(|| name.clone());
            for model in &provider.models {
                push(&mut choices, format!("{name}:{model}"), label.clone());
            }
        }
        let selected = config
            .default_model
            .as_ref()
            .and_then(|default| choices.iter().position(|choice| &choice.spec == default))
            .unwrap_or(0);
        Self { choices, selected }
    }

    /// The models on offer.
    #[must_use]
    pub fn choices(&self) -> &[ModelChoice] {
        &self.choices
    }

    /// Whether the configuration listed any model at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.choices.is_empty()
    }

    /// Index of the highlighted model.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The highlighted model.
    #[must_use]
    pub fn highlighted(&self) -> Option<&ModelChoice> {
        self.choices.get(self.selected)
    }

    /// Move the highlight and choose, or close.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Esc => Outcome::Close,
            KeyCode::Enter => match self.highlighted() {
                Some(choice) => Outcome::Model(choice.spec.clone()),
                None => Outcome::Close,
            },
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
                self.selected = self.choices.len().saturating_sub(1);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }

    fn step(&mut self, direction: isize) {
        if self.choices.is_empty() {
            return;
        }
        let len = self.choices.len() as isize;
        self.selected = ((self.selected as isize + direction).rem_euclid(len)) as usize;
    }
}

fn push(choices: &mut Vec<ModelChoice>, spec: String, provider: String) {
    if !choices.iter().any(|choice| choice.spec == spec) {
        choices.push(ModelChoice { spec, provider });
    }
}

fn provider_label(config: &Config, spec: &str) -> String {
    let name = spec.split_once(':').map_or(spec, |(name, _)| name);
    config
        .providers
        .get(name)
        .and_then(|provider| provider.display_name.clone())
        .unwrap_or_else(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use niki_config::Config;

    const CONFIG: &str = r#"
[models]
default = "kimi:kimi-k3"

[providers.kimi]
display_name = "Kimi"
class_path = "openai:Chat"
models = ["kimi-k3", "kimi-k2.7-code"]

[providers.anthropic]
display_name = "Anthropic"
class_path = "anthropic:Chat"
models = ["claude-sonnet-4"]
"#;

    fn picker() -> ModelPicker {
        ModelPicker::from_config(&Config::from_str(CONFIG).expect("config"))
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn the_default_model_is_first_and_highlighted() {
        let picker = picker();
        let specs: Vec<&str> = picker
            .choices()
            .iter()
            .map(|choice| choice.spec.as_str())
            .collect();
        assert_eq!(
            specs,
            [
                "kimi:kimi-k3",
                "anthropic:claude-sonnet-4",
                "kimi:kimi-k2.7-code"
            ]
        );
        assert_eq!(picker.selected(), 0);
        assert_eq!(
            picker.highlighted().map(|c| c.spec.as_str()),
            Some("kimi:kimi-k3")
        );
        assert_eq!(
            picker.highlighted().map(|c| c.provider.as_str()),
            Some("Kimi")
        );
    }

    #[test]
    fn choosing_a_model_returns_its_spec() {
        let mut picker = picker();
        assert_eq!(picker.handle_key(key(KeyCode::Down)), Outcome::Stay);
        assert_eq!(picker.selected(), 1);
        assert_eq!(
            picker.handle_key(key(KeyCode::Enter)),
            Outcome::Model("anthropic:claude-sonnet-4".to_owned())
        );
        assert_eq!(picker.handle_key(key(KeyCode::Esc)), Outcome::Close);
    }

    #[test]
    fn the_highlight_wraps_and_home_end_jump() {
        let mut picker = picker();
        picker.handle_key(key(KeyCode::Up));
        assert_eq!(picker.selected(), 2, "up from the first wraps to the last");
        picker.handle_key(key(KeyCode::Down));
        assert_eq!(picker.selected(), 0);
        picker.handle_key(key(KeyCode::End));
        assert_eq!(picker.selected(), 2);
        picker.handle_key(key(KeyCode::Home));
        assert_eq!(picker.selected(), 0);
        picker.handle_key(key(KeyCode::Char('j')));
        assert_eq!(picker.selected(), 1);
        picker.handle_key(key(KeyCode::Char('k')));
        assert_eq!(picker.selected(), 0);
    }

    #[test]
    fn a_config_with_no_models_offers_none() {
        let mut picker = ModelPicker::from_config(&Config::default());
        assert!(picker.is_empty());
        assert!(picker.highlighted().is_none());
        assert_eq!(picker.handle_key(key(KeyCode::Down)), Outcome::Stay);
        assert_eq!(picker.handle_key(key(KeyCode::Enter)), Outcome::Close);
    }
}
