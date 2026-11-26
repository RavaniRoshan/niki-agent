use crossterm::event::{KeyCode, KeyEvent};
use niki_tui::{Action, BINDINGS, chord_label};

use crate::Outcome;

/// What each app action does, in words for the person at the terminal.
#[must_use]
pub fn describe(action: Action) -> &'static str {
    match action {
        Action::QuitOrInterrupt => "Stop the running turn; leaves when nothing is running",
        Action::Quit => "Leave the app",
        Action::Interrupt => "Clear the draft, then stop the running turn",
        Action::CommandPalette => "Start a slash command in the composer",
        Action::Help => "This list",
        Action::ToggleAutoApprove => "Cycle manual, auto and yolo",
        Action::ToggleToolOutput => "Show or hide tool output (not in this build)",
        Action::ToggleSubagents => "Show or hide subagent activity (not in this build)",
        Action::OpenNotifications => "Show notifications (not in this build)",
        Action::OpenEditor => "Edit the prompt in your editor (not in this build)",
        Action::ScrollUp => "Move the cursor up a line in the composer",
        Action::ScrollDown => "Move the cursor down a line in the composer",
        Action::PageUp => "Move the transcript back one page",
        Action::PageDown => "Move the transcript forward one page",
        Action::ScrollHome => "Move the cursor to the start of its line",
        Action::ScrollEnd => "Move the cursor to the end of its line",
        Action::ApprovalYes => "Allow the command that is waiting",
        Action::ApprovalNo => "Refuse the command that is waiting",
    }
}

/// The keymap, and the lines it is drawn as.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HelpOverlay {
    offset: usize,
}

impl HelpOverlay {
    /// A fresh overlay, scrolled to the top.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every binding as `chord` and description, in keymap order.
    #[must_use]
    pub fn entries() -> Vec<(String, &'static str)> {
        BINDINGS
            .iter()
            .map(|binding| (chord_label(binding), describe(binding.action)))
            .collect()
    }

    /// How many lines the overlay is scrolled down.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Scroll, or close.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => Outcome::Close,
            KeyCode::Up => {
                self.offset = self.offset.saturating_sub(1);
                Outcome::Stay
            }
            KeyCode::Down => {
                self.offset = self.offset.saturating_add(1);
                Outcome::Stay
            }
            KeyCode::PageUp => {
                self.offset = self.offset.saturating_sub(10);
                Outcome::Stay
            }
            KeyCode::PageDown => {
                self.offset = self.offset.saturating_add(10);
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

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn every_binding_is_listed_once_in_keymap_order() {
        let entries = HelpOverlay::entries();
        assert_eq!(entries.len(), BINDINGS.len());
        assert_eq!(entries[0].0, "ctrl+c");
        assert_eq!(entries[0].1, describe(Action::QuitOrInterrupt));
        for (chord, description) in &entries {
            assert!(!chord.is_empty(), "a binding with no chord");
            assert!(!description.is_empty(), "{chord} has no description");
            assert!(!description.contains("  "), "{chord}: {description}");
        }
    }

    #[test]
    fn every_action_has_a_description_that_is_not_the_next_one() {
        let actions = [
            Action::QuitOrInterrupt,
            Action::Quit,
            Action::Interrupt,
            Action::CommandPalette,
            Action::Help,
            Action::ToggleAutoApprove,
            Action::ToggleToolOutput,
            Action::ToggleSubagents,
            Action::OpenNotifications,
            Action::OpenEditor,
            Action::ScrollUp,
            Action::ScrollDown,
            Action::PageUp,
            Action::PageDown,
            Action::ScrollHome,
            Action::ScrollEnd,
            Action::ApprovalYes,
            Action::ApprovalNo,
        ];
        for (index, action) in actions.iter().enumerate() {
            let text = describe(*action);
            assert!(!text.is_empty());
            for other in actions.iter().skip(index + 1) {
                assert_ne!(
                    text,
                    describe(*other),
                    "{action:?} and {other:?} share text"
                );
            }
        }
    }

    #[test]
    fn escape_closes_and_arrows_scroll() {
        let mut help = HelpOverlay::new();
        assert_eq!(help.handle_key(key(KeyCode::Down)), Outcome::Stay);
        assert_eq!(help.offset(), 1);
        assert_eq!(help.handle_key(key(KeyCode::PageDown)), Outcome::Stay);
        assert_eq!(help.offset(), 11);
        assert_eq!(help.handle_key(key(KeyCode::PageUp)), Outcome::Stay);
        assert_eq!(help.offset(), 1);
        assert_eq!(help.handle_key(key(KeyCode::Up)), Outcome::Stay);
        assert_eq!(help.offset(), 0);
        assert_eq!(help.handle_key(key(KeyCode::Up)), Outcome::Stay);
        assert_eq!(help.offset(), 0, "scrolling stops at the top");
        assert_eq!(help.handle_key(key(KeyCode::Esc)), Outcome::Close);
    }
}
