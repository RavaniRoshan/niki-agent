use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};

use crate::Outcome;

/// The answers, in the order they are drawn.
pub const OPTIONS: [&str; 2] = ["Not now", "Trust this workspace"];

/// The question asked once per workspace before project content may load.
///
/// The grant is the same record `niki trust` writes; this is only a way to give
/// it without leaving the app. Declining answers nothing, so the next launch
/// asks again — a workspace is never trusted by accident, and Escape is a
/// decline rather than a grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustModal {
    workspace: PathBuf,
    selected: usize,
}

impl TrustModal {
    /// Ask about `workspace`, with "Not now" preselected.
    #[must_use]
    pub fn new(workspace: impl Into<PathBuf>) -> Self {
        Self {
            workspace: workspace.into(),
            selected: 0,
        }
    }

    /// The workspace the question is about.
    #[must_use]
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// The answers on offer.
    #[must_use]
    pub fn options(&self) -> [&'static str; 2] {
        OPTIONS
    }

    /// Index of the highlighted answer.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Whether the highlight is on the grant.
    #[must_use]
    pub fn trusting(&self) -> bool {
        self.selected == 1
    }

    /// The lines that say what the grant would admit.
    #[must_use]
    pub fn body(&self, width: usize) -> Vec<String> {
        vec![
            crate::approval::ellipsize(&self.workspace.display().to_string(), width),
            crate::approval::ellipsize(
                "trusting loads this workspace's hooks, skills and instruction files",
                width,
            ),
            crate::approval::ellipsize(
                "the answer is stored in trusted_workspaces.toml; niki untrust withdraws it",
                width,
            ),
        ]
    }

    /// Answer, or move the highlight and wait.
    ///
    /// `y` and `t` trust, `n` and Escape decline. Enter takes the highlight, and
    /// the highlight starts on "Not now", so a stray Enter never grants.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if key.modifiers.is_empty() {
            match key.code {
                KeyCode::Char('y' | 't') => return Outcome::Trust,
                KeyCode::Char('n') => return Outcome::Close,
                KeyCode::Char('1') => return Outcome::Close,
                KeyCode::Char('2') => return Outcome::Trust,
                _ => {}
            }
        }
        match key.code {
            KeyCode::Enter => {
                if self.trusting() {
                    Outcome::Trust
                } else {
                    Outcome::Close
                }
            }
            KeyCode::Esc => Outcome::Close,
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

    fn prompt() -> TrustModal {
        TrustModal::new("/work/demo")
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn the_highlight_starts_on_declining() {
        let modal = prompt();
        assert_eq!(modal.selected(), 0);
        assert!(!modal.trusting());
        assert_eq!(modal.options(), OPTIONS);
    }

    #[test]
    fn a_stray_enter_declines_and_never_grants() {
        let mut modal = prompt();
        assert_eq!(modal.handle_key(key(KeyCode::Enter)), Outcome::Close);
        assert_eq!(modal.handle_key(key(KeyCode::Esc)), Outcome::Close);
    }

    #[test]
    fn the_letters_answer_without_moving_the_highlight() {
        let mut modal = prompt();
        assert_eq!(modal.handle_key(key(KeyCode::Char('y'))), Outcome::Trust);
        assert_eq!(modal.handle_key(key(KeyCode::Char('t'))), Outcome::Trust);
        assert_eq!(modal.handle_key(key(KeyCode::Char('n'))), Outcome::Close);
        assert_eq!(modal.selected(), 0, "a letter must not move the highlight");
        assert_eq!(modal.handle_key(key(KeyCode::Char('1'))), Outcome::Close);
        assert_eq!(modal.handle_key(key(KeyCode::Char('2'))), Outcome::Trust);
    }

    #[test]
    fn the_arrows_walk_between_the_two_answers() {
        let mut modal = prompt();
        assert_eq!(modal.handle_key(key(KeyCode::Down)), Outcome::Stay);
        assert!(modal.trusting());
        assert_eq!(modal.handle_key(key(KeyCode::Enter)), Outcome::Trust);
        assert_eq!(modal.handle_key(key(KeyCode::Tab)), Outcome::Stay);
        assert!(!modal.trusting());
        assert_eq!(modal.handle_key(key(KeyCode::BackTab)), Outcome::Stay);
        assert!(modal.trusting());
        assert_eq!(modal.handle_key(key(KeyCode::Right)), Outcome::Stay);
        assert!(!modal.trusting());
        assert_eq!(modal.handle_key(key(KeyCode::Left)), Outcome::Stay);
        assert!(modal.trusting());
    }

    #[test]
    fn the_body_names_the_workspace_and_what_the_grant_admits() {
        let modal = prompt();
        let body = modal.body(80);
        assert_eq!(body[0], "/work/demo");
        assert!(
            body[1].contains("hooks, skills and instruction files"),
            "{body:?}"
        );
        assert!(body[2].contains("trusted_workspaces.toml"), "{body:?}");
    }

    #[test]
    fn a_long_path_is_clipped_to_the_dialog_width() {
        let modal = TrustModal::new("/a/very/long/workspace/path/that/does/not/fit/in/the/box");
        let body = modal.body(20);
        assert_eq!(niki_render::display_width(&body[0]), 20);
        assert!(body[0].ends_with('…'), "{}", body[0]);
    }
}
