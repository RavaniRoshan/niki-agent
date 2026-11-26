use crossterm::event::{KeyCode, KeyEvent};
use niki_agent::{Approval, ApprovalRequest};
use niki_tui::{Action, action_for};

use crate::Outcome;

/// The answers, in the order they are drawn.
pub const OPTIONS: [(Approval, &str); 3] = [
    (Approval::Once, "Allow this once"),
    (Approval::Always, "Always allow this command"),
    (Approval::Deny, "Refuse it"),
];

/// A gated command waiting for the person's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalModal {
    request: ApprovalRequest,
    selected: usize,
}

impl ApprovalModal {
    /// Open the prompt for `request`, with "allow once" preselected.
    #[must_use]
    pub fn new(request: ApprovalRequest) -> Self {
        Self {
            request,
            selected: 0,
        }
    }

    /// The command and the mode that asked.
    #[must_use]
    pub fn request(&self) -> &ApprovalRequest {
        &self.request
    }

    /// The answers on offer.
    #[must_use]
    pub fn options(&self) -> [(Approval, &'static str); 3] {
        OPTIONS
    }

    /// Index of the highlighted answer.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The answer the highlight is on.
    #[must_use]
    pub fn highlighted(&self) -> Approval {
        OPTIONS[self.selected].0
    }

    /// Answer explicitly, or move the highlight and wait.
    ///
    /// Escape answers no: a prompt nobody answers would leave the turn waiting
    /// forever, and refusing is the safe way to close it.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if let Some(action) = action_for(key) {
            match action {
                Action::ApprovalYes => return Outcome::Answer(Approval::Once),
                Action::ApprovalNo => return Outcome::Answer(Approval::Deny),
                _ => {}
            }
        }
        match key.code {
            KeyCode::Char('a') => Outcome::Answer(Approval::Always),
            KeyCode::Enter => Outcome::Answer(self.highlighted()),
            KeyCode::Esc => Outcome::Answer(Approval::Deny),
            KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
                self.selected = (self.selected + OPTIONS.len() - 1) % OPTIONS.len();
                Outcome::Stay
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                self.selected = (self.selected + 1) % OPTIONS.len();
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use niki_agent::Mode;

    fn prompt() -> ApprovalModal {
        ApprovalModal::new(ApprovalRequest {
            command: "cargo publish".to_owned(),
            mode: Mode::Manual,
        })
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn letters_answer_without_moving_the_highlight() {
        let mut modal = prompt();
        assert_eq!(
            modal.handle_key(key(KeyCode::Char('y'))),
            Outcome::Answer(Approval::Once)
        );
        assert_eq!(
            modal.handle_key(key(KeyCode::Char('n'))),
            Outcome::Answer(Approval::Deny)
        );
        assert_eq!(
            modal.handle_key(key(KeyCode::Char('a'))),
            Outcome::Answer(Approval::Always)
        );
        assert_eq!(modal.selected(), 0, "a letter must not move the highlight");
    }

    #[test]
    fn enter_takes_the_highlighted_answer() {
        let mut modal = prompt();
        assert_eq!(modal.highlighted(), Approval::Once);
        assert_eq!(modal.handle_key(key(KeyCode::Down)), Outcome::Stay);
        assert_eq!(modal.selected(), 1);
        assert_eq!(modal.highlighted(), Approval::Always);
        assert_eq!(modal.handle_key(key(KeyCode::BackTab)), Outcome::Stay);
        assert_eq!(modal.selected(), 0);
        assert_eq!(
            modal.handle_key(key(KeyCode::Enter)),
            Outcome::Answer(Approval::Once)
        );

        let mut down = prompt();
        down.handle_key(key(KeyCode::Down));
        down.handle_key(key(KeyCode::Down));
        assert_eq!(
            down.handle_key(key(KeyCode::Enter)),
            Outcome::Answer(Approval::Deny)
        );
        assert_eq!(down.selected(), 2);
    }

    #[test]
    fn escape_refuses_rather_than_leaving_the_turn_waiting() {
        let mut modal = prompt();
        assert_eq!(
            modal.handle_key(key(KeyCode::Esc)),
            Outcome::Answer(Approval::Deny)
        );
    }

    #[test]
    fn the_prompt_names_the_command_and_the_mode() {
        let modal = prompt();
        assert_eq!(modal.request().command, "cargo publish");
        assert_eq!(modal.request().mode, Mode::Manual);
    }
}
