use std::ops::Range;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::Outcome;

/// What a session with no prompt to show previews.
pub const NO_PROMPT: &str = "(no prompt)";

/// How many rows the browser draws before it scrolls with the highlight.
pub const MAX_ROWS: usize = 12;

/// One saved session, as the browser lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadEntry {
    /// Session id, the value `/resume <id>` takes.
    pub id: String,
    /// First line of the session's title or first prompt, or [`NO_PROMPT`].
    pub preview: String,
    /// Stored messages, of every role.
    pub messages: i64,
    /// Directory the session was opened in.
    pub cwd: String,
}

impl ThreadEntry {
    /// Build a row from a session summary.
    ///
    /// Only the first line of the name is a preview: a row is one line, and a
    /// multi-line title or prompt would otherwise push every later row off the
    /// box.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: Option<&str>,
        messages: i64,
        cwd: impl Into<String>,
    ) -> Self {
        let preview = name
            .and_then(|name| name.lines().next())
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .unwrap_or(NO_PROMPT)
            .to_owned();
        Self {
            id: id.into(),
            preview,
            messages,
            cwd: cwd.into(),
        }
    }

    /// The message count in words, for the row's detail.
    #[must_use]
    pub fn message_label(&self) -> String {
        if self.messages == 1 {
            "1 message".to_owned()
        } else {
            format!("{} messages", self.messages)
        }
    }
}

/// The session browser: a filterable list of saved sessions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThreadPicker {
    entries: Vec<ThreadEntry>,
    visible: Vec<usize>,
    filter: String,
    selected: usize,
}

impl ThreadPicker {
    /// A browser over `entries`, in the order the store listed them.
    #[must_use]
    pub fn new(entries: Vec<ThreadEntry>) -> Self {
        let visible = (0..entries.len()).collect();
        Self {
            entries,
            visible,
            filter: String::new(),
            selected: 0,
        }
    }

    /// Every session the store listed, filtered or not.
    #[must_use]
    pub fn entries(&self) -> &[ThreadEntry] {
        &self.entries
    }

    /// What has been typed into the filter.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Whether the store listed any session at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many rows the filter leaves.
    #[must_use]
    pub fn len(&self) -> usize {
        self.visible.len()
    }

    /// Whether the filter leaves no row.
    #[must_use]
    pub fn is_filtered_empty(&self) -> bool {
        !self.is_empty() && self.visible.is_empty()
    }

    /// The row at a position in the filtered list.
    #[must_use]
    pub fn row(&self, index: usize) -> Option<&ThreadEntry> {
        self.visible
            .get(index)
            .and_then(|index| self.entries.get(*index))
    }

    /// Index of the highlighted row in the filtered list.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The highlighted row, when the filter leaves one.
    #[must_use]
    pub fn highlighted(&self) -> Option<&ThreadEntry> {
        self.row(self.selected)
    }

    /// The rows to draw: at most [`MAX_ROWS`], scrolled to keep the highlight
    /// in view.
    #[must_use]
    pub fn window(&self) -> Range<usize> {
        let start = self.selected.saturating_add(1).saturating_sub(MAX_ROWS);
        start..self.visible.len().min(start + MAX_ROWS)
    }

    /// Handle one key press: filter, move, choose or close.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        let chord = key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            KeyCode::Esc => Outcome::Close,
            KeyCode::Enter => self
                .highlighted()
                .map_or(Outcome::Close, |entry| Outcome::Session(entry.id.clone())),
            KeyCode::Down | KeyCode::Char('j') if !chord => {
                self.step(1);
                Outcome::Stay
            }
            KeyCode::Up | KeyCode::Char('k') if !chord => {
                self.step(-1);
                Outcome::Stay
            }
            KeyCode::Home => {
                self.selected = 0;
                Outcome::Stay
            }
            KeyCode::End => {
                self.selected = self.visible.len().saturating_sub(1);
                Outcome::Stay
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.refilter();
                Outcome::Stay
            }
            KeyCode::Char(character) if !chord => {
                self.filter.push(character);
                self.refilter();
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }

    /// Move one row and stop at either end: the list never wraps.
    fn step(&mut self, direction: isize) {
        let Some(last) = self.visible.len().checked_sub(1) else {
            self.selected = 0;
            return;
        };
        self.selected = if direction < 0 {
            self.selected.saturating_sub(1)
        } else {
            self.selected.saturating_add(1).min(last)
        };
    }

    /// Recompute the matching rows and put the highlight back on the first.
    fn refilter(&mut self) {
        let needle = self.filter.to_lowercase();
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                needle.is_empty()
                    || entry.preview.to_lowercase().contains(&needle)
                    || entry.cwd.to_lowercase().contains(&needle)
                    || entry.id.to_lowercase().contains(&needle)
            })
            .map(|(index, _)| index)
            .collect();
        self.selected = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn picker() -> ThreadPicker {
        ThreadPicker::new(vec![
            ThreadEntry::new("a1", Some("deploy the site"), 4, "/work/site"),
            ThreadEntry::new("b2", Some("fix the parser"), 2, "/work/parser"),
            ThreadEntry::new("c3", None, 0, "/srv/empty"),
        ])
    }

    fn type_text(picker: &mut ThreadPicker, text: &str) {
        for character in text.chars() {
            picker.handle_key(key(KeyCode::Char(character)));
        }
    }

    #[test]
    fn the_filter_is_case_insensitive_and_backspace_widens_it_again() {
        let mut picker = picker();
        type_text(&mut picker, "DEPLOY");
        assert_eq!(picker.filter(), "DEPLOY");
        assert_eq!(picker.len(), 1);
        assert_eq!(
            picker.highlighted().map(|entry| entry.id.as_str()),
            Some("a1")
        );

        picker.handle_key(key(KeyCode::Backspace));
        assert_eq!(picker.filter(), "DEPLO");
        assert_eq!(picker.len(), 1);

        type_text(&mut picker, "x");
        assert!(picker.is_filtered_empty());
        assert_eq!(picker.len(), 0);
        assert!(picker.highlighted().is_none());
    }

    #[test]
    fn the_filter_matches_the_cwd_too_and_the_whole_list_comes_back() {
        let mut picker = picker();
        type_text(&mut picker, "parser");
        assert_eq!(picker.len(), 1, "the preview matches");
        for _ in 0.."parser".len() {
            picker.handle_key(key(KeyCode::Backspace));
        }
        assert_eq!(picker.filter(), "");

        type_text(&mut picker, "srv/empty");
        assert_eq!(picker.len(), 1, "the cwd matches");
        assert_eq!(
            picker.highlighted().map(|entry| entry.id.as_str()),
            Some("c3")
        );
        for _ in 0.."srv/empty".len() {
            picker.handle_key(key(KeyCode::Backspace));
        }
        assert_eq!(picker.len(), 3);
    }

    #[test]
    fn the_highlight_stops_at_both_ends_and_never_wraps() {
        let mut picker = picker();
        picker.handle_key(key(KeyCode::Up));
        assert_eq!(picker.selected(), 0, "up from the top stays put");
        picker.handle_key(key(KeyCode::Char('k')));
        assert_eq!(picker.selected(), 0);

        for _ in 0..10 {
            picker.handle_key(key(KeyCode::Down));
        }
        assert_eq!(picker.selected(), 2, "down stops at the last row");
        for _ in 0..10 {
            picker.handle_key(key(KeyCode::Char('j')));
        }
        assert_eq!(picker.selected(), 2);

        picker.handle_key(key(KeyCode::Home));
        assert_eq!(picker.selected(), 0);
        picker.handle_key(key(KeyCode::End));
        assert_eq!(picker.selected(), 2);
        picker.handle_key(key(KeyCode::Up));
        assert_eq!(picker.selected(), 1);
        picker.handle_key(key(KeyCode::Down));
        assert_eq!(picker.selected(), 2);
    }

    #[test]
    fn enter_returns_the_highlighted_id_and_escape_closes() {
        let mut picker = picker();
        assert_eq!(picker.handle_key(key(KeyCode::Down)), Outcome::Stay);
        assert_eq!(
            picker.handle_key(key(KeyCode::Enter)),
            Outcome::Session("b2".to_owned())
        );
        assert_eq!(picker.handle_key(key(KeyCode::Esc)), Outcome::Close);
    }

    #[test]
    fn enter_with_no_match_closes_without_a_choice() {
        let mut picker = picker();
        type_text(&mut picker, "zzz");
        assert!(picker.is_filtered_empty());
        assert_eq!(picker.handle_key(key(KeyCode::Enter)), Outcome::Close);
    }

    #[test]
    fn a_session_without_a_prompt_previews_the_placeholder() {
        let picker = picker();
        let empty = picker.row(2).expect("the empty session");
        assert_eq!(empty.preview, NO_PROMPT);
        assert_eq!(empty.message_label(), "0 messages");
        assert_eq!(
            ThreadEntry::new("x", Some("   "), 1, "/w").preview,
            NO_PROMPT,
            "a blank prompt is no preview"
        );
        assert_eq!(
            ThreadEntry::new("x", Some("first line\nsecond line"), 1, "/w").preview,
            "first line"
        );
    }

    #[test]
    fn the_window_follows_the_highlight_past_the_last_full_page() {
        let entries: Vec<ThreadEntry> = (0..30)
            .map(|index| ThreadEntry::new(format!("id-{index}"), Some("prompt"), 1, "/w"))
            .collect();
        let mut picker = ThreadPicker::new(entries);
        assert_eq!(picker.window(), 0..MAX_ROWS);
        picker.handle_key(key(KeyCode::End));
        assert_eq!(picker.window(), 18..30);
        assert!(
            picker.window().contains(&picker.selected()),
            "the highlight stays visible"
        );
    }

    #[test]
    fn an_empty_store_has_nothing_to_choose() {
        let mut picker = ThreadPicker::new(Vec::new());
        assert!(picker.is_empty());
        assert!(!picker.is_filtered_empty());
        assert_eq!(picker.handle_key(key(KeyCode::Enter)), Outcome::Close);
        assert_eq!(picker.handle_key(key(KeyCode::Down)), Outcome::Stay);
        assert_eq!(picker.selected(), 0);
    }
}
