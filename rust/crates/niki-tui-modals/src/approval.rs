use crossterm::event::{KeyCode, KeyEvent};
use niki_agent::{Approval, ApprovalAction, ApprovalRequest};

use crate::Outcome;

/// The answers, in the order they are drawn.
pub const OPTIONS: [(Approval, &str); 3] = [
    (Approval::Once, "Allow this once"),
    (Approval::Always, "Allow for this session"),
    (Approval::Deny, "Refuse it"),
];

/// A gated action waiting for the person's answer.
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

    /// The action and the mode that asked.
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

    /// The lines that say exactly what would run or change.
    ///
    /// A shell command shows its command line. A write shows the absolute
    /// target and whether it resolves inside the workspace. Anything else shows
    /// the tool name and a one-line summary of its arguments. Each line is
    /// clipped to `width` display cells and ends in an ellipsis when it does not
    /// fit.
    #[must_use]
    pub fn body(&self, width: usize) -> Vec<String> {
        let width = width.max(1);
        match &self.request.action {
            ApprovalAction::Command(command) => vec![ellipsize(command, width)],
            ApprovalAction::Write { tool, path, inside } => {
                let boundary = if *inside {
                    "resolves inside the workspace"
                } else {
                    "resolves outside the workspace"
                };
                vec![
                    ellipsize(&path.display().to_string(), width),
                    ellipsize(&format!("{tool} · {boundary}"), width),
                ]
            }
            ApprovalAction::Tool { name, summary } => {
                vec![ellipsize(name, width), ellipsize(summary, width)]
            }
        }
    }

    /// Answer explicitly, or move the highlight and wait.
    ///
    /// `y` and `a` allow once, `s` allows for the rest of the session, and `d`,
    /// `n` and Escape refuse. Escape refuses rather than leaving the turn
    /// waiting forever, and refusing is the safe way to close it.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if key.modifiers.is_empty() {
            match key.code {
                KeyCode::Char('y' | 'a') => return Outcome::Answer(Approval::Once),
                KeyCode::Char('s') => return Outcome::Answer(Approval::Always),
                KeyCode::Char('d' | 'n') => return Outcome::Answer(Approval::Deny),
                _ => {}
            }
        }
        match key.code {
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

/// Clip one line to `width` display cells, ending in an ellipsis when it does
/// not fit. A line break in the action's own text is drawn as a space, because
/// the prompt is one line per detail.
pub fn ellipsize(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let text = text.replace(['\n', '\r'], " ");
    if niki_render::display_width(&text) <= width {
        return text;
    }
    let budget = width.saturating_sub(1);
    let mut clipped = String::new();
    let mut used = 0;
    for character in text.chars() {
        let cells = niki_render::display_width(&character.to_string());
        if used + cells > budget {
            break;
        }
        clipped.push(character);
        used += cells;
    }
    clipped.push('…');
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use niki_agent::Mode;
    use std::path::PathBuf;

    fn prompt() -> ApprovalModal {
        ApprovalModal::new(ApprovalRequest::command("cargo publish", Mode::Manual))
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn the_letters_answer_with_the_documented_outcomes() {
        let mut modal = prompt();
        for code in [KeyCode::Char('y'), KeyCode::Char('a')] {
            assert_eq!(
                modal.handle_key(key(code)),
                Outcome::Answer(Approval::Once),
                "{code:?}"
            );
        }
        assert_eq!(
            modal.handle_key(key(KeyCode::Char('s'))),
            Outcome::Answer(Approval::Always)
        );
        for code in [KeyCode::Char('d'), KeyCode::Char('n'), KeyCode::Esc] {
            assert_eq!(
                modal.handle_key(key(code)),
                Outcome::Answer(Approval::Deny),
                "{code:?}"
            );
        }
        assert_eq!(modal.selected(), 0, "a letter must not move the highlight");
    }

    #[test]
    fn enter_takes_the_highlighted_answer() {
        let mut modal = prompt();
        assert_eq!(modal.highlighted(), Approval::Once);
        assert_eq!(modal.handle_key(key(KeyCode::Down)), Outcome::Stay);
        assert_eq!(modal.selected(), 1);
        assert_eq!(modal.highlighted(), Approval::Always);
        assert_eq!(
            modal.handle_key(key(KeyCode::Tab)),
            Outcome::Stay,
            "tab walks forward"
        );
        assert_eq!(modal.selected(), 2);
        assert_eq!(modal.handle_key(key(KeyCode::Up)), Outcome::Stay);
        assert_eq!(modal.selected(), 1);
        assert_eq!(
            modal.handle_key(key(KeyCode::Enter)),
            Outcome::Answer(Approval::Always)
        );

        let mut down = prompt();
        down.handle_key(key(KeyCode::Down));
        down.handle_key(key(KeyCode::Down));
        assert_eq!(
            down.handle_key(key(KeyCode::Enter)),
            Outcome::Answer(Approval::Deny)
        );
        assert_eq!(down.selected(), 2);

        assert_eq!(
            modal.handle_key(key(KeyCode::BackTab)),
            Outcome::Stay,
            "back tab walks back"
        );
        assert_eq!(modal.selected(), 0);
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
    fn a_command_body_is_the_command_line() {
        let modal = prompt();
        assert_eq!(modal.body(60), vec!["cargo publish".to_owned()]);
    }

    #[test]
    fn a_write_body_is_the_absolute_path_and_the_boundary() {
        let path = PathBuf::from("/home/shiva/outside/secret.txt");
        let modal = ApprovalModal::new(ApprovalRequest::write(
            "write_file",
            path,
            false,
            Mode::Manual,
        ));
        assert_eq!(
            modal.body(60),
            vec![
                "/home/shiva/outside/secret.txt".to_owned(),
                "write_file · resolves outside the workspace".to_owned(),
            ]
        );

        let inside = ApprovalModal::new(ApprovalRequest::write(
            "edit_file",
            PathBuf::from("/work/src/lib.rs"),
            true,
            Mode::Manual,
        ));
        assert!(inside.body(60)[1].contains("resolves inside the workspace"));
    }

    #[test]
    fn any_other_tool_shows_its_name_and_argument_summary() {
        let modal = ApprovalModal::new(ApprovalRequest::tool(
            "mcp__fixture__write",
            r#"{"path":"note.txt"}"#,
            Mode::Manual,
        ));
        assert_eq!(
            modal.body(60),
            vec![
                "mcp__fixture__write".to_owned(),
                r#"{"path":"note.txt"}"#.to_owned(),
            ]
        );
    }

    #[test]
    fn a_long_detail_is_clipped_to_the_dialog_width() {
        let modal = ApprovalModal::new(ApprovalRequest::command(
            "cargo publish --dry-run --token a-very-long-token-value",
            Mode::Manual,
        ));
        let body = modal.body(24);
        assert_eq!(body.len(), 1);
        assert_eq!(niki_render::display_width(&body[0]), 24);
        assert!(body[0].ends_with('…'), "{}", body[0]);
    }

    #[test]
    fn the_prompt_names_the_action_and_the_mode() {
        let modal = prompt();
        assert_eq!(modal.request().command, "cargo publish");
        assert_eq!(modal.request().mode, Mode::Manual);
    }
}
