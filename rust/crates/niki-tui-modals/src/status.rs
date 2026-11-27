use crossterm::event::{KeyCode, KeyEvent};

use crate::Outcome;

/// One labelled fact on the status card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusField {
    /// The fixed left-hand word.
    pub label: String,
    /// The value the running session reports.
    pub value: String,
}

impl StatusField {
    /// A field from a label and its value.
    #[must_use]
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

/// A read-only card listing the running session's facts.
///
/// The state is the fields the app hands over; drawing needs a `Frame`, so the
/// card is checked by driving keys and by reading its fields back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusCard {
    fields: Vec<StatusField>,
    offset: usize,
}

impl StatusCard {
    /// A card showing `fields`, scrolled to the top.
    #[must_use]
    pub fn new(fields: Vec<StatusField>) -> Self {
        Self { fields, offset: 0 }
    }

    /// The facts the card shows, in the order they are drawn.
    #[must_use]
    pub fn fields(&self) -> &[StatusField] {
        &self.fields
    }

    /// How many rows the card is scrolled down.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Whether the card has no facts at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Close, or move the window down the fields.
    ///
    /// A card that fits on screen is drawn from the top whatever the offset, so
    /// the movement keys only matter on a terminal too short for every line.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Outcome::Close,
            KeyCode::Up => {
                self.offset = self.offset.saturating_sub(1);
                Outcome::Stay
            }
            KeyCode::Down => {
                self.offset = self.offset.saturating_add(1).min(self.last_offset());
                Outcome::Stay
            }
            KeyCode::PageUp => {
                self.offset = self.offset.saturating_sub(10);
                Outcome::Stay
            }
            KeyCode::PageDown => {
                self.offset = self.offset.saturating_add(10).min(self.last_offset());
                Outcome::Stay
            }
            KeyCode::Home => {
                self.offset = 0;
                Outcome::Stay
            }
            KeyCode::End => {
                self.offset = self.last_offset();
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }

    fn last_offset(&self) -> usize {
        self.fields.len().saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn card() -> StatusCard {
        StatusCard::new(vec![
            StatusField::new("model", "mock:mock-model"),
            StatusField::new("provider", "mock"),
            StatusField::new("mode", "manual"),
            StatusField::new("workspace", "/work"),
        ])
    }

    #[test]
    fn the_card_keeps_the_fields_it_was_given_in_order() {
        let card = card();
        let labels: Vec<&str> = card
            .fields()
            .iter()
            .map(|field| field.label.as_str())
            .collect();
        assert_eq!(labels, ["model", "provider", "mode", "workspace"]);
        assert_eq!(card.fields()[0].value, "mock:mock-model");
        assert!(!card.is_empty());
        assert!(StatusCard::default().is_empty());
    }

    #[test]
    fn escape_and_q_close_and_movement_scrolls_within_the_fields() {
        let mut card = card();
        assert_eq!(card.handle_key(key(KeyCode::Esc)), Outcome::Close);
        assert_eq!(card.handle_key(key(KeyCode::Char('q'))), Outcome::Close);

        assert_eq!(card.handle_key(key(KeyCode::Down)), Outcome::Stay);
        assert_eq!(card.offset(), 1);
        assert_eq!(card.handle_key(key(KeyCode::PageDown)), Outcome::Stay);
        assert_eq!(card.offset(), 3, "the offset stops at the last field");
        assert_eq!(card.handle_key(key(KeyCode::Down)), Outcome::Stay);
        assert_eq!(card.offset(), 3, "the offset does not run past the fields");
        assert_eq!(card.handle_key(key(KeyCode::Home)), Outcome::Stay);
        assert_eq!(card.offset(), 0);
        assert_eq!(card.handle_key(key(KeyCode::Up)), Outcome::Stay);
        assert_eq!(card.offset(), 0, "scrolling stops at the top");
        assert_eq!(card.handle_key(key(KeyCode::End)), Outcome::Stay);
        assert_eq!(card.offset(), 3);
        assert_eq!(card.handle_key(key(KeyCode::PageUp)), Outcome::Stay);
        assert_eq!(card.offset(), 0);
        assert_eq!(card.handle_key(key(KeyCode::Char('x'))), Outcome::Stay);
    }
}
