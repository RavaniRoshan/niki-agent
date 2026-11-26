//! Keymap, reader mode, and view state for the Niki terminal UI.
//!
//! The Python keymap is the reference (`docs/niki/KEYMAP.md`). This crate keeps
//! the same app-level bindings so a user's muscle memory survives the port,
//! and it holds the state the renderer needs without depending on a terminal.

use std::sync::LazyLock;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use niki_render::{Glyphs, TranscriptModel};

/// App-level actions from the Python binding registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    QuitOrInterrupt,
    Quit,
    Interrupt,
    CommandPalette,
    Help,
    ToggleAutoApprove,
    ToggleToolOutput,
    ToggleSubagents,
    OpenNotifications,
    OpenEditor,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    ScrollHome,
    ScrollEnd,
    ApprovalYes,
    ApprovalNo,
}

/// One key chord and the action it dispatches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
    pub action: Action,
}

const fn chord(key: KeyCode, modifiers: KeyModifiers, action: Action) -> Binding {
    Binding {
        key,
        modifiers,
        action,
    }
}

/// The app-level keymap, matching the Python registry.
///
/// Built once at first use: `KeyModifiers` combines with a non-const `BitOr`,
/// so the table cannot be a `const` item.
pub static BINDINGS: LazyLock<Vec<Binding>> = LazyLock::new(|| {
    vec![
        chord(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            Action::QuitOrInterrupt,
        ),
        chord(KeyCode::Char('d'), KeyModifiers::CONTROL, Action::Quit),
        chord(KeyCode::Esc, KeyModifiers::NONE, Action::Interrupt),
        chord(
            KeyCode::Char('p'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            Action::CommandPalette,
        ),
        chord(KeyCode::Char('?'), KeyModifiers::NONE, Action::Help),
        chord(
            KeyCode::BackTab,
            KeyModifiers::NONE,
            Action::ToggleAutoApprove,
        ),
        chord(
            KeyCode::Char('o'),
            KeyModifiers::CONTROL,
            Action::ToggleToolOutput,
        ),
        chord(
            KeyCode::Char('t'),
            KeyModifiers::CONTROL,
            Action::ToggleSubagents,
        ),
        chord(
            KeyCode::Char('n'),
            KeyModifiers::CONTROL,
            Action::OpenNotifications,
        ),
        chord(
            KeyCode::Char('g'),
            KeyModifiers::CONTROL,
            Action::OpenEditor,
        ),
        chord(KeyCode::Up, KeyModifiers::NONE, Action::ScrollUp),
        chord(KeyCode::Down, KeyModifiers::NONE, Action::ScrollDown),
        chord(KeyCode::PageUp, KeyModifiers::NONE, Action::PageUp),
        chord(KeyCode::PageDown, KeyModifiers::NONE, Action::PageDown),
        chord(KeyCode::Home, KeyModifiers::NONE, Action::ScrollHome),
        chord(KeyCode::End, KeyModifiers::NONE, Action::ScrollEnd),
        chord(KeyCode::Char('y'), KeyModifiers::NONE, Action::ApprovalYes),
        chord(KeyCode::Char('n'), KeyModifiers::NONE, Action::ApprovalNo),
    ]
});

/// Resolve a key event to its app action.
#[must_use]
pub fn action_for(event: KeyEvent) -> Option<Action> {
    BINDINGS
        .iter()
        .find(|binding| {
            binding.key == event.code
                && event.modifiers.contains(binding.modifiers)
                && binding.modifiers.contains(event.modifiers)
        })
        .map(|binding| binding.action)
}

/// Parse a binding label such as `ctrl+shift+p` or `escape`.
#[must_use]
pub fn parse_chord(label: &str) -> Option<(KeyCode, KeyModifiers)> {
    let mut modifiers = KeyModifiers::NONE;
    let mut key = None;
    for part in label.split('+') {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" => modifiers |= KeyModifiers::CONTROL,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            "alt" => modifiers |= KeyModifiers::ALT,
            "super" => modifiers |= KeyModifiers::SUPER,
            "escape" | "esc" => key = Some(KeyCode::Esc),
            "enter" => key = Some(KeyCode::Enter),
            "tab" => key = Some(KeyCode::Tab),
            "backtab" => key = Some(KeyCode::BackTab),
            "up" => key = Some(KeyCode::Up),
            "down" => key = Some(KeyCode::Down),
            "left" => key = Some(KeyCode::Left),
            "right" => key = Some(KeyCode::Right),
            "pageup" => key = Some(KeyCode::PageUp),
            "pagedown" => key = Some(KeyCode::PageDown),
            "home" => key = Some(KeyCode::Home),
            "end" => key = Some(KeyCode::End),
            "space" => key = Some(KeyCode::Char(' ')),
            other if other.chars().count() == 1 => {
                key = other.chars().next().map(KeyCode::Char);
            }
            _ => return None,
        }
    }
    key.map(|key| (key, modifiers))
}

/// Screen-reader mode is opt-in through the same env var the Python client
/// uses, plus an explicit flag for tests.
#[must_use]
pub fn screen_reader_enabled(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

/// Every piece of view state a frame needs.
#[derive(Debug, Clone)]
pub struct AppState {
    pub model: TranscriptModel,
    pub composer: String,
    pub scroll: usize,
    pub auto_approve: bool,
    pub show_tool_output: bool,
    pub subagents_open: bool,
    pub reader: bool,
    pub ascii: bool,
    /// Header corner label: `replay` offline, `live` for an engine session.
    pub label: &'static str,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            model: TranscriptModel::default(),
            composer: String::new(),
            scroll: 0,
            auto_approve: false,
            show_tool_output: false,
            subagents_open: false,
            reader: false,
            ascii: false,
            label: "replay",
        }
    }
}

impl AppState {
    /// Apply an action, returning whether the app should exit.
    pub fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::Quit | Action::QuitOrInterrupt => return true,
            Action::Interrupt => self.model.status = Some("cancelled".to_owned()),
            Action::CommandPalette => self.composer = "/".to_owned(),
            Action::Help => self.composer = "/help".to_owned(),
            Action::ToggleAutoApprove => self.auto_approve = !self.auto_approve,
            Action::ToggleToolOutput => self.show_tool_output = !self.show_tool_output,
            Action::ToggleSubagents => self.subagents_open = !self.subagents_open,
            Action::OpenNotifications | Action::OpenEditor => {}
            Action::ScrollUp | Action::PageUp => {
                self.scroll = self.scroll.saturating_add(1);
            }
            Action::ScrollDown | Action::PageDown => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            Action::ScrollHome => self.scroll = 0,
            Action::ScrollEnd => {}
            Action::ApprovalYes | Action::ApprovalNo => {}
        }
        false
    }

    #[must_use]
    pub fn glyphs(&self) -> Glyphs {
        Glyphs::for_ascii(self.ascii)
    }

    /// Render the current frame.
    #[must_use]
    pub fn render(&self, width: usize, height: usize) -> niki_render::Screen {
        self.model
            .render_labelled(width, height, self.glyphs(), self.label)
    }

    /// Reader mode: a linear transcript with no terminal control sequences and
    /// no composer chrome, suitable for a screen reader or a pipe.
    #[must_use]
    pub fn render_reader(&self, width: usize) -> String {
        let mut out = String::new();
        out.push_str("Niki Agent\n\n");
        for message in &self.model.messages {
            let label = match message.role {
                niki_render::Role::User => "You",
                niki_render::Role::Assistant => "Niki",
                niki_render::Role::Tool => "Tool",
                niki_render::Role::System => "System",
            };
            out.push_str(label);
            out.push_str(": ");
            out.push_str(&message.text);
            out.push_str("\n\n");
        }
        if let Some(status) = &self.model.status {
            out.push_str(status);
            out.push_str("\n\n");
        }
        let _ = width;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn every_advertised_binding_parses_back_to_itself() {
        let labels = [
            ("ctrl+c", Action::QuitOrInterrupt),
            ("ctrl+d", Action::Quit),
            ("escape", Action::Interrupt),
            ("ctrl+shift+p", Action::CommandPalette),
            ("?", Action::Help),
            ("backtab", Action::ToggleAutoApprove),
            ("ctrl+o", Action::ToggleToolOutput),
            ("ctrl+t", Action::ToggleSubagents),
            ("ctrl+n", Action::OpenNotifications),
            ("ctrl+g", Action::OpenEditor),
            ("up", Action::ScrollUp),
            ("down", Action::ScrollDown),
            ("pageup", Action::PageUp),
            ("pagedown", Action::PageDown),
            ("home", Action::ScrollHome),
            ("end", Action::ScrollEnd),
            ("y", Action::ApprovalYes),
            ("n", Action::ApprovalNo),
        ];
        for (label, action) in labels {
            let (code, modifiers) = parse_chord(label).unwrap_or_else(|| panic!("{label}"));
            assert_eq!(action_for(event(code, modifiers)), Some(action), "{label}");
        }
    }

    #[test]
    fn plain_n_is_approval_not_ctrl_n() {
        assert_eq!(
            action_for(event(KeyCode::Char('n'), KeyModifiers::NONE)),
            Some(Action::ApprovalNo)
        );
        assert_eq!(
            action_for(event(KeyCode::Char('n'), KeyModifiers::CONTROL)),
            Some(Action::OpenNotifications)
        );
    }

    #[test]
    fn control_c_exits_and_escape_interrupts() {
        let mut state = AppState::default();
        assert!(!state.apply(Action::Interrupt));
        assert_eq!(state.model.status.as_deref(), Some("cancelled"));
        assert!(state.apply(Action::QuitOrInterrupt));
    }

    #[test]
    fn toggle_actions_flip_one_flag_each() {
        let mut state = AppState::default();
        for action in [
            Action::ToggleAutoApprove,
            Action::ToggleToolOutput,
            Action::ToggleSubagents,
        ] {
            state.apply(action);
        }
        assert!(state.auto_approve);
        assert!(state.show_tool_output);
        assert!(state.subagents_open);
    }

    #[test]
    fn reader_mode_reads_the_same_transcript() {
        use niki_proto::Transcript;
        let transcript =
            Transcript::from_json(include_str!("../../niki-proto/fixtures/hello.json")).unwrap();
        let state = AppState {
            model: TranscriptModel::from_transcript(&transcript),
            reader: true,
            ..AppState::default()
        };
        let text = state.render_reader(80);
        assert!(text.contains("You: Say hello"));
        assert!(text.contains("Niki: Hello from the Niki protocol fixture."));
        assert!(!text.contains('\u{1b}'));
    }

    #[test]
    fn screen_reader_truthy_values_match_the_python_client() {
        for value in ["1", "true", "yes", "on", "TRUE"] {
            assert!(screen_reader_enabled(Some(value)), "{value}");
        }
        for value in ["0", "false", "no", "off", ""] {
            assert!(!screen_reader_enabled(Some(value)), "{value}");
        }
        assert!(!screen_reader_enabled(None));
    }
}
