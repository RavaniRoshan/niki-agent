use niki_agent::Usage;
use niki_policy::Mode;

/// What the running turn is doing, in the words the status bar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    /// No turn is running.
    Idle,
    /// The model or a tool is working.
    Working,
    /// The turn is waiting for the person to approve a command.
    Waiting,
    /// The person stopped the turn; what it produced is still on screen.
    Cancelled,
    /// The turn ended on a provider failure.
    Failed,
}

impl TurnState {
    /// The label the status bar shows.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Waiting => "waiting for you",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    /// Whether the turn is still running.
    #[must_use]
    pub fn is_busy(self) -> bool {
        matches!(self, Self::Working | Self::Waiting)
    }
}

/// The one-line summary under the composer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// Model identifier as the provider names it.
    pub model: String,
    /// Approval mode in force.
    pub mode: Mode,
    /// What the turn is doing.
    pub turn: TurnState,
    /// Tokens reported so far, when the provider reports them.
    pub tokens: Usage,
    /// Working directory the tools are rooted at.
    pub cwd: String,
}

impl Status {
    /// Build a status line for a fresh session.
    #[must_use]
    pub fn new(model: impl Into<String>, mode: Mode, cwd: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            mode,
            turn: TurnState::Idle,
            tokens: Usage::default(),
            cwd: cwd.into(),
        }
    }

    /// The token counters, or `None` when the provider has not reported any.
    #[must_use]
    pub fn token_label(&self) -> Option<String> {
        if self.tokens == Usage::default() {
            return None;
        }
        Some(format!(
            "{} in / {} out",
            self.tokens.input_tokens, self.tokens.output_tokens
        ))
    }

    /// The whole line, with `separator` between the fields and the spinner
    /// glyph when one is running.
    #[must_use]
    pub fn line(&self, separator: &str, spinner: Option<char>) -> String {
        let mut fields = vec![self.model.clone(), self.mode.label().to_owned()];
        let mut state = self.turn.label().to_owned();
        if let Some(spinner) = spinner {
            state.push(' ');
            state.push(spinner);
        }
        fields.push(state);
        if let Some(tokens) = self.token_label() {
            fields.push(tokens);
        }
        fields.push(self.cwd.clone());
        fields.join(separator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> Status {
        Status::new("mock:mock-model", Mode::Manual, "/work")
    }

    #[test]
    fn the_line_names_every_field_in_a_fixed_order() {
        let status = status();
        assert_eq!(
            status.line(" | ", None),
            "mock:mock-model | manual | idle | /work"
        );
    }

    #[test]
    fn tokens_appear_once_the_provider_reports_them() {
        let mut status = status();
        status.tokens = Usage {
            input_tokens: 12,
            output_tokens: 5,
        };
        assert_eq!(
            status.line(" | ", None),
            "mock:mock-model | manual | idle | 12 in / 5 out | /work"
        );
        assert_eq!(status.token_label().as_deref(), Some("12 in / 5 out"));
    }

    #[test]
    fn the_spinner_sits_inside_the_turn_field() {
        let mut status = status();
        status.turn = TurnState::Working;
        assert_eq!(
            status.line(" | ", Some('*')),
            "mock:mock-model | manual | working * | /work"
        );
        assert_eq!(
            status.line(" | ", None),
            "mock:mock-model | manual | working | /work"
        );
    }

    #[test]
    fn every_turn_state_has_a_label() {
        assert_eq!(TurnState::Idle.label(), "idle");
        assert_eq!(TurnState::Working.label(), "working");
        assert_eq!(TurnState::Waiting.label(), "waiting for you");
        assert_eq!(TurnState::Cancelled.label(), "cancelled");
        assert_eq!(TurnState::Failed.label(), "failed");
        assert!(TurnState::Working.is_busy());
        assert!(TurnState::Waiting.is_busy());
        assert!(!TurnState::Idle.is_busy());
        assert!(!TurnState::Cancelled.is_busy());
    }
}
