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
    let mut ranked: Vec<(u8, usize, &'static CommandInfo)> = Vec::new();
    for (index, command) in COMMANDS.iter().enumerate() {
        let Some(name) = command.name.strip_prefix('/') else {
            continue;
        };
        let name = name.to_lowercase();
        let rank = if name.starts_with(&word) {
            0
        } else if name.contains(word.as_str()) {
            1
        } else if is_subsequence(&word, &name) {
            2
        } else {
            continue;
        };
        ranked.push((rank, index, command));
    }
    ranked.sort_by_key(|(rank, index, _)| (*rank, *index));
    ranked
        .into_iter()
        .take(MAX_COMPLETIONS)
        .map(|(_, _, command)| command)
        .collect()
}

/// Whether every character of `needle` appears in `haystack` in order.
///
/// This is the loose tier: `/cmpct` reaches `/compact` without the person
/// remembering where the vowels are.
fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut haystack = haystack.chars();
    needle
        .chars()
        .all(|wanted| haystack.any(|candidate| candidate == wanted))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(draft: &str) -> Vec<&'static str> {
        matching(draft)
            .into_iter()
            .map(|command| command.name)
            .collect()
    }

    #[test]
    fn a_bare_slash_lists_the_commands_in_order() {
        let listed = names("/");
        assert_eq!(listed.len(), MAX_COMPLETIONS);
        assert_eq!(listed[0], "/auto");
    }

    #[test]
    fn a_prefix_beats_a_substring() {
        let listed = names("/co");
        let position = |name: &str| listed.iter().position(|entry| *entry == name);
        let compact = position("/compact").expect("compact matches");
        let context = position("/context").expect("context matches");
        assert!(compact < context);
    }

    #[test]
    fn a_subsequence_reaches_the_command_without_the_vowels() {
        assert!(names("/cmpct").contains(&"/compact"));
        assert!(names("/thrd").contains(&"/threads"));
        assert!(names("/mdl").contains(&"/model"));
    }

    #[test]
    fn an_unrelated_word_matches_nothing() {
        assert!(names("/zzzz").is_empty());
    }

    #[test]
    fn whitespace_ends_the_command_name() {
        assert!(names("/clear now").is_empty());
    }
}
