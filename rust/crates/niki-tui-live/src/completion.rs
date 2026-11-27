//! Matching a draft against the slash commands while it is typed.

use niki_commands::{COMMANDS, CommandInfo};

/// How many commands the list shows at once.
pub const MAX_COMPLETIONS: usize = 6;

/// The commands one draft matches, with the one under the highlight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    matches: Vec<&'static CommandInfo>,
    selected: usize,
}

impl Completion {
    /// The list for `draft`, when it is a command name in progress.
    ///
    /// A draft matches while it is one word behind a slash: names that start
    /// with the word come first, then names that contain it elsewhere, each in
    /// the registry's order.
    #[must_use]
    pub fn of(draft: &str) -> Option<Self> {
        let matches = matching(draft);
        if matches.is_empty() {
            return None;
        }
        Some(Self {
            matches,
            selected: 0,
        })
    }

    /// The commands the list draws.
    #[must_use]
    pub fn commands(&self) -> &[&'static CommandInfo] {
        &self.matches
    }

    /// The index of the highlighted command.
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// The highlighted command.
    #[must_use]
    pub fn selected(&self) -> &'static CommandInfo {
        let last = self.matches.len().saturating_sub(1);
        self.matches[self.selected.min(last)]
    }

    /// Move the highlight up one row, stopping at the first.
    pub fn up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Move the highlight down one row, stopping at the last.
    pub fn down(&mut self) {
        let last = self.matches.len().saturating_sub(1);
        self.selected = (self.selected + 1).min(last);
    }

    /// Put the highlight back on `name` when it is still listed.
    pub fn reselect(&mut self, name: &str) {
        if let Some(index) = self.matches.iter().position(|command| command.name == name) {
            self.selected = index;
        }
    }

    /// What Tab completes to: the highlighted name and a space after it.
    #[must_use]
    pub fn completed(&self) -> String {
        format!("{} ", self.selected().name)
    }
}

fn matching(draft: &str) -> Vec<&'static CommandInfo> {
    let Some(word) = draft.strip_prefix('/') else {
        return Vec::new();
    };
    if word.chars().any(char::is_whitespace) {
        return Vec::new();
    }
    let word = word.to_lowercase();
    let mut prefix = Vec::new();
    let mut substring = Vec::new();
    for command in COMMANDS {
        let Some(name) = command.name.strip_prefix('/') else {
            continue;
        };
        if name.starts_with(&word) {
            prefix.push(command);
        } else if name.contains(word.as_str()) {
            substring.push(command);
        }
    }
    prefix.extend(substring);
    prefix.truncate(MAX_COMPLETIONS);
    prefix
}
