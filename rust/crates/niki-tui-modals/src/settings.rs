//! The `[ui]` settings, changed from the app instead of the file.
//!
//! Every row is one key in the configuration's `[ui]` table. `Enter` or space
//! cycles the value; the app applies it live and writes the file, so the next
//! launch starts with the same choice. There is no save step and no hidden
//! state: what the list shows is what the file now holds.

use crossterm::event::{KeyCode, KeyEvent};

use crate::Outcome;

/// One value the settings screen changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting {
    /// A palette name from the theme registry.
    Theme(String),
    /// `full` or `reduced`.
    Motion(String),
    /// `unicode` or `ascii`.
    Glyphs(String),
    /// Draw the welcome block on the first screen.
    Welcome(bool),
    /// Draw the spinner beside the turn state.
    Progress(bool),
    /// Animate the activity marker on a running tool call.
    Shimmer(bool),
    /// Set the terminal window title.
    Title(bool),
}

impl Setting {
    /// The `[ui]` key this value is written to.
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::Theme(_) => "theme",
            Self::Motion(_) => "motion",
            Self::Glyphs(_) => "glyphs",
            Self::Welcome(_) => "welcome",
            Self::Progress(_) => "progress",
            Self::Shimmer(_) => "shimmer",
            Self::Title(_) => "title",
        }
    }

    /// The value as a TOML literal, ready for the configuration file.
    #[must_use]
    pub fn toml(&self) -> String {
        match self {
            Self::Theme(name) | Self::Motion(name) | Self::Glyphs(name) => format!("\"{name}\""),
            Self::Welcome(on) | Self::Progress(on) | Self::Shimmer(on) | Self::Title(on) => {
                on.to_string()
            }
        }
    }
}

/// The values the screen starts from, gathered from the running app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsState {
    /// Palette in use.
    pub theme: String,
    /// `full` or `reduced`.
    pub motion: String,
    /// `unicode` or `ascii`.
    pub glyphs: String,
    /// Welcome block.
    pub welcome: bool,
    /// Spinner.
    pub progress: bool,
    /// Activity shimmer.
    pub shimmer: bool,
    /// Window title.
    pub title: bool,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            theme: "niki".to_owned(),
            motion: "full".to_owned(),
            glyphs: "unicode".to_owned(),
            welcome: true,
            progress: true,
            shimmer: true,
            title: true,
        }
    }
}

/// The settings list: fixed rows, a moving highlight, one change per key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsModal {
    state: SettingsState,
    themes: Vec<String>,
    selected: usize,
}

/// The rows, in drawing order.
const LABELS: [&str; 7] = [
    "theme", "motion", "glyphs", "welcome", "progress", "shimmer", "title",
];

impl SettingsModal {
    /// Open the list on `state`, cycling `themes` in the order given.
    #[must_use]
    pub fn new(state: SettingsState, themes: Vec<String>) -> Self {
        Self {
            state,
            themes,
            selected: 0,
        }
    }

    /// The values the list currently shows.
    #[must_use]
    pub fn state(&self) -> &SettingsState {
        &self.state
    }

    /// Which row the highlight is on.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Every row: its key and the value it holds.
    #[must_use]
    pub fn rows(&self) -> Vec<(&'static str, String)> {
        let on_off = |value: bool| if value { "on" } else { "off" }.to_owned();
        LABELS
            .iter()
            .map(|label| {
                let value = match *label {
                    "theme" => self.state.theme.clone(),
                    "motion" => self.state.motion.clone(),
                    "glyphs" => self.state.glyphs.clone(),
                    "welcome" => on_off(self.state.welcome),
                    "progress" => on_off(self.state.progress),
                    "shimmer" => on_off(self.state.shimmer),
                    _ => on_off(self.state.title),
                };
                (*label, value)
            })
            .collect()
    }

    /// Handle one key press.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Outcome::Close,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                Outcome::Stay
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(LABELS.len() - 1);
                Outcome::Stay
            }
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l') => {
                Outcome::Setting(self.cycle())
            }
            KeyCode::Left | KeyCode::Char('h') => Outcome::Setting(self.cycle_back()),
            _ => Outcome::Stay,
        }
    }

    /// The next value for the selected row.
    fn cycle(&mut self) -> Setting {
        match LABELS[self.selected] {
            "theme" => {
                let theme = next_in(&self.themes, &self.state.theme);
                self.state.theme.clone_from(&theme);
                Setting::Theme(theme)
            }
            "motion" => {
                let motion = flip(&self.state.motion, "full", "reduced");
                self.state.motion.clone_from(&motion);
                Setting::Motion(motion)
            }
            "glyphs" => {
                let glyphs = flip(&self.state.glyphs, "unicode", "ascii");
                self.state.glyphs.clone_from(&glyphs);
                Setting::Glyphs(glyphs)
            }
            "welcome" => {
                self.state.welcome = !self.state.welcome;
                Setting::Welcome(self.state.welcome)
            }
            "progress" => {
                self.state.progress = !self.state.progress;
                Setting::Progress(self.state.progress)
            }
            "shimmer" => {
                self.state.shimmer = !self.state.shimmer;
                Setting::Shimmer(self.state.shimmer)
            }
            _ => {
                self.state.title = !self.state.title;
                Setting::Title(self.state.title)
            }
        }
    }

    /// The previous value for the selected row, which is the same toggle for
    /// everything but the theme list.
    fn cycle_back(&mut self) -> Setting {
        if LABELS[self.selected] == "theme" {
            let index = self
                .themes
                .iter()
                .position(|name| *name == self.state.theme)
                .unwrap_or(0);
            let index = (index + self.themes.len().saturating_sub(1)) % self.themes.len().max(1);
            let theme = self.themes.get(index).cloned().unwrap_or_default();
            self.state.theme.clone_from(&theme);
            Setting::Theme(theme)
        } else {
            self.cycle()
        }
    }
}

/// The name after `current`, wrapping at the end.
fn next_in(names: &[String], current: &str) -> String {
    let index = names.iter().position(|name| name == current).unwrap_or(0);
    names
        .get((index + 1) % names.len().max(1))
        .cloned()
        .unwrap_or_default()
}

/// The other of two names.
fn flip(current: &str, first: &str, second: &str) -> String {
    if current == first {
        second.to_owned()
    } else {
        first.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn modal() -> SettingsModal {
        SettingsModal::new(
            SettingsState::default(),
            vec!["niki".to_owned(), "mono".to_owned()],
        )
    }

    fn press(modal: &mut SettingsModal, code: KeyCode) -> Outcome {
        modal.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn enter_flips_a_switch_and_reports_it() {
        let mut modal = modal();
        for _ in 0..3 {
            press(&mut modal, KeyCode::Down);
        }
        assert_eq!(
            press(&mut modal, KeyCode::Enter),
            Outcome::Setting(Setting::Welcome(false))
        );
        assert!(!modal.state().welcome);
        assert_eq!(modal.rows()[3].1, "off");
    }

    #[test]
    fn the_theme_row_cycles_through_the_names_it_was_given() {
        let mut modal = modal();
        assert_eq!(
            press(&mut modal, KeyCode::Enter),
            Outcome::Setting(Setting::Theme("mono".to_owned()))
        );
        assert_eq!(
            press(&mut modal, KeyCode::Enter),
            Outcome::Setting(Setting::Theme("niki".to_owned()))
        );
    }

    #[test]
    fn motion_and_glyphs_swap_between_their_two_values() {
        let mut modal = modal();
        press(&mut modal, KeyCode::Down);
        assert_eq!(
            press(&mut modal, KeyCode::Enter),
            Outcome::Setting(Setting::Motion("reduced".to_owned()))
        );
        press(&mut modal, KeyCode::Down);
        assert_eq!(
            press(&mut modal, KeyCode::Enter),
            Outcome::Setting(Setting::Glyphs("ascii".to_owned()))
        );
    }

    #[test]
    fn the_highlight_stops_at_both_ends() {
        let mut modal = modal();
        press(&mut modal, KeyCode::Up);
        assert_eq!(modal.selected(), 0);
        for _ in 0..20 {
            press(&mut modal, KeyCode::Down);
        }
        assert_eq!(modal.selected(), LABELS.len() - 1);
    }

    #[test]
    fn escape_closes() {
        let mut modal = modal();
        assert_eq!(press(&mut modal, KeyCode::Esc), Outcome::Close);
    }

    #[test]
    fn every_setting_writes_a_toml_literal() {
        assert_eq!(Setting::Theme("mono".to_owned()).toml(), "\"mono\"");
        assert_eq!(Setting::Title(false).toml(), "false");
        assert_eq!(Setting::Welcome(true).key(), "welcome");
    }
}
