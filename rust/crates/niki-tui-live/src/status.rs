use std::time::Duration;

use niki_agent::{ContextReport, SessionUsage, Usage};
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
    /// The conversation's estimated size, once the agent reports one.
    pub context: Option<ContextReport>,
    /// What the session has spent, once the agent reports it.
    pub usage: Option<SessionUsage>,
    /// The model's context window, when the host knows one.
    pub context_window: Option<usize>,
    /// The checked-out branch, when the workspace is a repository.
    pub git_branch: Option<String>,
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
            context: None,
            usage: None,
            context_window: None,
            git_branch: None,
            cwd: cwd.into(),
        }
    }

    /// Name the checked-out branch, if the workspace has one.
    pub fn set_branch(&mut self, branch: Option<String>) {
        self.git_branch = branch.filter(|branch| !branch.is_empty() && branch != "HEAD");
    }

    /// Remember the agent's context report and session usage.
    pub fn set_context(&mut self, report: ContextReport, usage: SessionUsage) {
        self.context = Some(report);
        self.usage = Some(usage);
    }

    /// The context field: a percentage when the window is known, otherwise the
    /// estimated token count.
    #[must_use]
    pub fn context_label(&self) -> Option<String> {
        let report = self.context?;
        Some(format_context(report.estimated_tokens, self.context_window))
    }

    /// The session's input and output tokens together, when there are any.
    #[must_use]
    pub fn total_label(&self) -> Option<String> {
        let total = match self.usage {
            Some(usage) => usage.total_tokens(),
            None if self.tokens == Usage::default() => return None,
            None => self.tokens.input_tokens + self.tokens.output_tokens,
        };
        Some(format!("{} tok", format_tokens(total)))
    }

    /// The state line: what the session is, without counts.
    ///
    /// Model, approval mode, what the turn is doing and the directory the tools
    /// are rooted at. Token and context accounting lives on its own line, so a
    /// long session never pushes the directory off the screen.
    #[must_use]
    pub fn state_line(&self, separator: &str, spinner: Option<char>) -> String {
        let mut fields = vec![self.model.clone(), self.mode.label().to_owned()];
        fields.extend(self.git_branch.clone());
        let mut state = self.turn.label().to_owned();
        if let Some(spinner) = spinner {
            state.push(' ');
            state.push(spinner);
        }
        fields.push(state);
        fields.push(self.cwd.clone());
        fields.join(separator)
    }

    /// The state line without the turn field: everything that is true for the
    /// whole session.
    #[must_use]
    pub fn context_line(&self, separator: &str) -> String {
        let mut fields = vec![self.model.clone(), self.mode.label().to_owned()];
        fields.extend(self.git_branch.clone());
        fields.push(self.cwd.clone());
        fields.join(separator)
    }

    /// The context and token accounting, once there is anything to report.
    #[must_use]
    pub fn accounting(&self, separator: &str) -> Option<String> {
        let mut fields = Vec::new();
        if let Some(context) = self.context_label() {
            fields.push(context);
        }
        if let Some(tokens) = self.total_label() {
            fields.push(tokens);
        }
        if fields.is_empty() {
            None
        } else {
            Some(fields.join(separator))
        }
    }
}

/// The context field for a token count and an optional window.
///
/// A window of zero is treated as unknown rather than dividing by it.
#[must_use]
pub fn format_context(tokens: usize, window: Option<usize>) -> String {
    match window.and_then(|window| context_percentage(tokens, window)) {
        Some(percent) => format!("ctx {percent}%"),
        None => format!(
            "ctx {} tok",
            format_tokens(u64::try_from(tokens).unwrap_or(u64::MAX))
        ),
    }
}

/// The whole percentage of `window` that `tokens` occupies, rounded half up.
#[must_use]
pub fn context_percentage(tokens: usize, window: usize) -> Option<u32> {
    if window == 0 {
        return None;
    }
    let tokens = u128::try_from(tokens).ok()?;
    let window = u128::try_from(window).ok()?;
    let percent = (tokens.saturating_mul(100).saturating_add(window / 2)) / window;
    u32::try_from(percent).ok()
}

/// The elapsed seconds of a running turn, in the shortest readable form.
///
/// `12s`, `1m 05s`, `1h 00m 05s` — the same shape the reference client uses so
/// two clock readings a minute apart are comparable at a glance.
#[must_use]
pub fn format_elapsed(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        return format!("{seconds}s");
    }
    if seconds < 3_600 {
        return format!("{}m {:02}s", seconds / 60, seconds % 60);
    }
    format!(
        "{}h {:02}m {:02}s",
        seconds / 3_600,
        (seconds % 3_600) / 60,
        seconds % 60
    )
}

/// `count` with a comma between each group of three digits.
#[must_use]
pub fn format_tokens(count: u64) -> String {
    let digits = count.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.char_indices() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(digit);
    }
    formatted
}

/// One line describing what `/context` prints.
#[must_use]
pub fn describe_context(report: ContextReport, usage: SessionUsage) -> String {
    format!(
        "context: {} messages, {} estimated tokens, last {} in / {} out, {} steps, {} session tokens",
        report.messages,
        format_tokens(u64::try_from(report.estimated_tokens).unwrap_or(u64::MAX)),
        format_tokens(report.last_input_tokens),
        format_tokens(report.last_output_tokens),
        report.steps,
        format_tokens(usage.total_tokens()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> Status {
        Status::new("mock:mock-model", Mode::Manual, "/work")
    }

    #[test]
    fn a_known_branch_sits_between_the_mode_and_the_turn() {
        let mut status = status();
        status.set_branch(Some("niki-agent".to_owned()));
        assert_eq!(
            status.state_line(" · ", None),
            "mock:mock-model · manual · niki-agent · idle · /work"
        );
        assert_eq!(
            status.context_line(" · "),
            "mock:mock-model · manual · niki-agent · /work"
        );
    }

    #[test]
    fn a_detached_head_is_not_a_branch() {
        let mut status = status();
        status.set_branch(Some("HEAD".to_owned()));
        assert_eq!(status.git_branch, None);
        status.set_branch(Some(String::new()));
        assert_eq!(status.git_branch, None);
        status.set_branch(None);
        assert_eq!(status.git_branch, None);
    }

    #[test]
    fn the_context_line_leaves_the_turn_out() {
        let mut status = status();
        status.turn = TurnState::Working;
        assert_eq!(
            status.context_line(" · "),
            "mock:mock-model · manual · /work"
        );
    }

    #[test]
    fn the_state_line_names_every_field_in_a_fixed_order() {
        let status = status();
        assert_eq!(
            status.state_line(" | ", None),
            "mock:mock-model | manual | idle | /work"
        );
        assert_eq!(status.accounting(" | "), None);
    }

    #[test]
    fn tokens_move_to_the_accounting_line_once_the_provider_reports_them() {
        let mut status = status();
        status.tokens = Usage {
            input_tokens: 12,
            output_tokens: 5,
        };
        assert_eq!(
            status.state_line(" | ", None),
            "mock:mock-model | manual | idle | /work",
            "the state line never carries counts"
        );
        assert_eq!(status.accounting(" | ").as_deref(), Some("17 tok"));
        assert_eq!(status.total_label().as_deref(), Some("17 tok"));
    }

    #[test]
    fn session_usage_overrides_the_per_run_counters() {
        let mut status = status();
        status.tokens = Usage {
            input_tokens: 12,
            output_tokens: 5,
        };
        status.set_context(
            ContextReport {
                messages: 4,
                estimated_tokens: 1_234,
                last_input_tokens: 900,
                last_output_tokens: 120,
                steps: 2,
            },
            SessionUsage {
                input_tokens: 1_000,
                output_tokens: 234,
                steps: 2,
            },
        );
        assert_eq!(
            status.accounting(" | ").as_deref(),
            Some("ctx 1,234 tok | 1,234 tok")
        );
    }

    #[test]
    fn a_known_window_shows_a_rounded_percentage() {
        let mut status = status();
        status.context_window = Some(8_000);
        status.set_context(
            ContextReport {
                messages: 1,
                estimated_tokens: 1_000,
                ..ContextReport::default()
            },
            SessionUsage::default(),
        );
        assert_eq!(status.context_label().as_deref(), Some("ctx 13%"));
        assert_eq!(status.accounting(" | ").as_deref(), Some("ctx 13% | 0 tok"));
        assert_eq!(
            status.state_line(" | ", None),
            "mock:mock-model | manual | idle | /work"
        );
    }

    #[test]
    fn an_unknown_window_shows_the_estimate_instead() {
        assert_eq!(format_context(1_234, None), "ctx 1,234 tok");
        assert_eq!(format_context(1_234, Some(0)), "ctx 1,234 tok");
        assert_eq!(format_context(1_234, Some(4_936)), "ctx 25%");
    }

    #[test]
    fn percentages_round_half_up_and_never_divide_by_zero() {
        assert_eq!(context_percentage(0, 100), Some(0));
        assert_eq!(context_percentage(1, 8), Some(13));
        assert_eq!(context_percentage(2, 3), Some(67));
        assert_eq!(context_percentage(7, 8), Some(88));
        assert_eq!(context_percentage(15, 16), Some(94));
        assert_eq!(context_percentage(150, 100), Some(150));
        assert_eq!(context_percentage(1, 0), None);
    }

    #[test]
    fn elapsed_reads_as_seconds_then_minutes_then_hours() {
        assert_eq!(format_elapsed(Duration::from_secs(0)), "0s");
        assert_eq!(format_elapsed(Duration::from_secs(59)), "59s");
        assert_eq!(format_elapsed(Duration::from_secs(60)), "1m 00s");
        assert_eq!(format_elapsed(Duration::from_secs(65)), "1m 05s");
        assert_eq!(format_elapsed(Duration::from_secs(3_599)), "59m 59s");
        assert_eq!(format_elapsed(Duration::from_secs(3_600)), "1h 00m 00s");
        assert_eq!(format_elapsed(Duration::from_secs(7_389)), "2h 03m 09s");
    }

    #[test]
    fn token_counts_are_grouped_in_threes() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(999), "999");
        assert_eq!(format_tokens(1_000), "1,000");
        assert_eq!(format_tokens(12_345), "12,345");
        assert_eq!(format_tokens(1_234_567), "1,234,567");
    }

    #[test]
    fn the_context_description_names_every_field() {
        let line = describe_context(
            ContextReport {
                messages: 3,
                estimated_tokens: 1_234,
                last_input_tokens: 900,
                last_output_tokens: 120,
                steps: 2,
            },
            SessionUsage {
                input_tokens: 1_000,
                output_tokens: 234,
                steps: 2,
            },
        );
        assert_eq!(
            line,
            "context: 3 messages, 1,234 estimated tokens, last 900 in / 120 out, 2 steps, 1,234 session tokens"
        );
    }

    #[test]
    fn the_spinner_sits_inside_the_turn_field() {
        let mut status = status();
        status.turn = TurnState::Working;
        assert_eq!(
            status.state_line(" | ", Some('*')),
            "mock:mock-model | manual | working * | /work"
        );
        assert_eq!(
            status.state_line(" | ", None),
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
