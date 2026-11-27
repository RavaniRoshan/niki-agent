use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use niki_render::{display_width, wrap};
use niki_tools::AskRequest;

use crate::Outcome;
use crate::approval::ellipsize;

/// Rows scrolled by one `PageUp` or `PageDown`.
const PAGE: usize = 10;

/// What the person answered on the question prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestionReply {
    /// One of the offered options, by its label.
    Choice(String),
    /// Text typed when the question allowed a free-form answer.
    Freeform(String),
    /// Escape, or a closed prompt: the question was not answered.
    Cancelled,
}

/// One line the question prompt draws.
///
/// Wrapping happens in [`QuestionModal::rows`], so a row is exactly one line at
/// the width it was built for and the overlay only has to style it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestionRow {
    /// One wrapped line of the question itself.
    Question(String),
    /// How many choices there are and how many may be picked.
    Count(String),
    /// One offered choice, with its position in the request.
    Option {
        /// Position in the request's option list.
        index: usize,
        /// The label and description, clipped to one line.
        text: String,
    },
    /// The free-text row, present only when the question allows one.
    Freeform {
        /// The hint, or the answer typed so far.
        text: String,
    },
    /// The keys the prompt accepts.
    Hint(String),
}

/// A structured question waiting for the person's answer.
///
/// The request comes from `request_user_input`. Exactly one option is returned
/// because the tool's schema offers no multi-select field; a free-text row is
/// added only when the request's `allow_freeform` is set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionModal {
    request: AskRequest,
    selected: usize,
    freeform: String,
    /// A manual scroll set by PageUp/PageDown; `None` follows the highlight.
    scroll: Option<usize>,
}

impl QuestionModal {
    /// Open the prompt with the first option highlighted.
    #[must_use]
    pub fn new(request: AskRequest) -> Self {
        Self {
            request,
            selected: 0,
            freeform: String::new(),
            scroll: None,
        }
    }

    /// The question and the choices it offers.
    #[must_use]
    pub fn request(&self) -> &AskRequest {
        &self.request
    }

    /// Index of the highlighted row: an option, or the free-text row when it is
    /// last.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Whether the free-text row is highlighted.
    #[must_use]
    pub fn is_freeform_selected(&self) -> bool {
        self.request.allow_freeform && self.selected >= self.request.options.len()
    }

    /// The text typed into the free-text row so far.
    #[must_use]
    pub fn freeform(&self) -> &str {
        &self.freeform
    }

    /// How many rows the highlight can reach.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.request.options.len() + usize::from(self.request.allow_freeform)
    }

    /// The lines the prompt draws at `width` columns.
    ///
    /// The question is wrapped; every other row is one clipped line, so a long
    /// question or many options can be scrolled without running past the frame.
    #[must_use]
    pub fn rows(&self, width: usize) -> Vec<QuestionRow> {
        let width = width.max(1);
        let mut rows: Vec<QuestionRow> = wrap(&self.request.question, width)
            .into_iter()
            .map(QuestionRow::Question)
            .collect();

        let count = self.request.options.len();
        let noun = if count == 1 { "option" } else { "options" };
        rows.push(QuestionRow::Count(format!(
            "{count} {noun} · single answer"
        )));

        let option_width = width.saturating_sub(2);
        for (index, option) in self.request.options.iter().enumerate() {
            let text = match &option.description {
                Some(description) => format!("{} — {description}", option.label),
                None => option.label.clone(),
            };
            rows.push(QuestionRow::Option {
                index,
                text: ellipsize(&text, option_width),
            });
        }

        if self.request.allow_freeform {
            let text = if self.freeform.is_empty() {
                "type your own answer".to_owned()
            } else {
                format!("your answer: {}", self.freeform)
            };
            rows.push(QuestionRow::Freeform {
                text: clip_tail(&text, option_width),
            });
        }

        let hint = if self.request.allow_freeform {
            "enter choose | type an answer | up and down move | page up and down scroll | escape cancel"
        } else {
            "enter choose | up and down move | page up and down scroll | escape cancel"
        };
        rows.push(QuestionRow::Hint(ellipsize(hint, width)));
        rows
    }

    /// The first question line to draw for a frame `visible` rows tall.
    ///
    /// `PageUp` and `PageDown` move this; the options block below the question
    /// stays pinned, so a long question scrolls without hiding the choices.
    #[must_use]
    pub fn question_offset(&self, width: usize, visible: usize) -> usize {
        if visible == 0 {
            return 0;
        }
        let total = wrap(&self.request.question, width.max(1)).len();
        self.scroll.unwrap_or(0).min(total.saturating_sub(visible))
    }

    /// The first interactive row to draw for a frame `visible` rows tall.
    ///
    /// The interactive rows are the count, the options and the free-text row;
    /// the highlight is kept in view when they do not all fit.
    #[must_use]
    pub fn interactive_offset(&self, visible: usize) -> usize {
        if visible == 0 {
            return 0;
        }
        let selected = 1 + self.selected;
        selected.saturating_sub(visible.saturating_sub(1))
    }

    /// Move the highlight, take the answer, or cancel.
    ///
    /// `Enter` returns the highlighted option, or the typed text when the
    /// free-text row is highlighted and has content. `Esc` cancels; `q` is an
    /// ordinary character and is never a close key.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if key.code == KeyCode::Esc {
            return Outcome::Question(QuestionReply::Cancelled);
        }
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Enter => return self.submit(),
            KeyCode::Up | KeyCode::Left | KeyCode::BackTab => self.step(-1),
            KeyCode::Down | KeyCode::Right | KeyCode::Tab => self.step(1),
            KeyCode::PageUp => {
                self.scroll = Some(self.scroll.unwrap_or(0).saturating_sub(PAGE));
            }
            KeyCode::PageDown => {
                self.scroll = Some(self.scroll.unwrap_or(0).saturating_add(PAGE));
            }
            KeyCode::Backspace if self.is_freeform_selected() => {
                self.freeform.pop();
                self.scroll = None;
            }
            KeyCode::Char(character) if !control && !alt && self.request.allow_freeform => {
                self.selected = self.request.options.len();
                self.freeform.push(character);
                self.scroll = None;
            }
            _ => {}
        }
        Outcome::Stay
    }

    fn submit(&mut self) -> Outcome {
        if self.is_freeform_selected() {
            let answer = self.freeform.trim();
            if answer.is_empty() {
                return Outcome::Stay;
            }
            return Outcome::Question(QuestionReply::Freeform(answer.to_owned()));
        }
        match self.request.options.get(self.selected) {
            Some(option) => Outcome::Question(QuestionReply::Choice(option.label.clone())),
            None => Outcome::Stay,
        }
    }

    fn step(&mut self, direction: isize) {
        let len = self.row_count();
        if len == 0 {
            return;
        }
        let len = len as isize;
        self.selected = ((self.selected as isize + direction).rem_euclid(len)) as usize;
        self.scroll = None;
    }
}

/// The tail of `text` that fits in `width` cells, with a leading ellipsis when
/// it is clipped, so the newest text typed into a field stays in view.
fn clip_tail(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if display_width(text) <= width {
        return text.to_owned();
    }
    let budget = width.saturating_sub(1);
    let mut clipped = String::new();
    let mut used = 0;
    for character in text.chars().rev() {
        let cells = display_width(&character.to_string());
        if used + cells > budget {
            break;
        }
        clipped.insert(0, character);
        used += cells;
    }
    clipped.insert(0, '…');
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;
    use niki_tools::AskOption;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn request(allow_freeform: bool) -> AskRequest {
        AskRequest {
            question: "Which store?".to_owned(),
            options: vec![
                AskOption {
                    label: "SQLite".to_owned(),
                    description: Some("one file, no server".to_owned()),
                },
                AskOption {
                    label: "JSON".to_owned(),
                    description: None,
                },
            ],
            allow_freeform,
        }
    }

    fn text_question(question: &str, allow_freeform: bool) -> AskRequest {
        AskRequest {
            question: question.to_owned(),
            options: vec![
                AskOption {
                    label: "one".to_owned(),
                    description: None,
                },
                AskOption {
                    label: "two".to_owned(),
                    description: None,
                },
            ],
            allow_freeform,
        }
    }

    #[test]
    fn the_question_counts_the_choices_and_says_one_answer_is_expected() {
        let modal = QuestionModal::new(request(false));
        let rows = modal.rows(60);
        assert_eq!(rows[0], QuestionRow::Question("Which store?".to_owned()));
        assert_eq!(
            rows[1],
            QuestionRow::Count("2 options · single answer".to_owned())
        );
        assert_eq!(
            rows[2],
            QuestionRow::Option {
                index: 0,
                text: "SQLite — one file, no server".to_owned()
            }
        );
        assert_eq!(
            rows[3],
            QuestionRow::Option {
                index: 1,
                text: "JSON".to_owned()
            }
        );
        assert!(matches!(rows.last(), Some(QuestionRow::Hint(_))));
        assert!(
            !rows
                .iter()
                .any(|row| matches!(row, QuestionRow::Freeform { .. })),
            "free text is not offered"
        );
    }

    #[test]
    fn down_and_enter_return_the_second_option() {
        let mut modal = QuestionModal::new(request(false));
        assert_eq!(modal.handle_key(key(KeyCode::Down)), Outcome::Stay);
        assert_eq!(modal.selected(), 1);
        assert_eq!(
            modal.handle_key(key(KeyCode::Enter)),
            Outcome::Question(QuestionReply::Choice("JSON".to_owned()))
        );
    }

    #[test]
    fn left_tab_and_back_tab_match_the_approval_prompt() {
        let mut modal = QuestionModal::new(request(false));
        assert_eq!(modal.handle_key(key(KeyCode::Down)), Outcome::Stay);
        assert_eq!(modal.handle_key(key(KeyCode::Tab)), Outcome::Stay);
        assert_eq!(modal.selected(), 0, "two options wrap");
        assert_eq!(modal.handle_key(key(KeyCode::BackTab)), Outcome::Stay);
        assert_eq!(modal.selected(), 1);
        assert_eq!(modal.handle_key(key(KeyCode::Left)), Outcome::Stay);
        assert_eq!(modal.selected(), 0);
        assert_eq!(modal.handle_key(key(KeyCode::Right)), Outcome::Stay);
        assert_eq!(modal.selected(), 1);
    }

    #[test]
    fn escape_cancels_the_question() {
        let mut modal = QuestionModal::new(request(true));
        assert_eq!(
            modal.handle_key(key(KeyCode::Esc)),
            Outcome::Question(QuestionReply::Cancelled)
        );
    }

    #[test]
    fn q_is_typed_not_a_close_key_when_free_text_is_offered() {
        let mut modal = QuestionModal::new(request(true));
        assert_eq!(modal.handle_key(key(KeyCode::Char('q'))), Outcome::Stay);
        assert_eq!(modal.freeform(), "q");
        assert!(modal.is_freeform_selected(), "typing focuses the field");
        assert_eq!(
            modal.handle_key(key(KeyCode::Enter)),
            Outcome::Question(QuestionReply::Freeform("q".to_owned()))
        );
    }

    #[test]
    fn q_does_nothing_when_free_text_is_refused() {
        let mut modal = QuestionModal::new(request(false));
        assert_eq!(modal.handle_key(key(KeyCode::Char('q'))), Outcome::Stay);
        assert_eq!(modal.selected(), 0, "the highlight stays on an option");
        assert_eq!(modal.freeform(), "");
        assert_eq!(
            modal.handle_key(key(KeyCode::Enter)),
            Outcome::Question(QuestionReply::Choice("SQLite".to_owned()))
        );
    }

    #[test]
    fn typing_and_backspace_build_the_free_text_answer() {
        let mut modal = QuestionModal::new(request(true));
        for character in "a b".chars() {
            assert_eq!(
                modal.handle_key(key(KeyCode::Char(character))),
                Outcome::Stay
            );
        }
        assert_eq!(modal.freeform(), "a b");
        assert_eq!(modal.handle_key(key(KeyCode::Backspace)), Outcome::Stay);
        assert_eq!(modal.freeform(), "a ");
        assert_eq!(
            modal.handle_key(key(KeyCode::Enter)),
            Outcome::Question(QuestionReply::Freeform("a".to_owned()))
        );
    }

    #[test]
    fn an_empty_free_text_row_does_not_submit() {
        let mut modal = QuestionModal::new(request(true));
        assert_eq!(modal.handle_key(key(KeyCode::Up)), Outcome::Stay);
        assert!(modal.is_freeform_selected());
        assert_eq!(modal.handle_key(key(KeyCode::Enter)), Outcome::Stay);
    }

    #[test]
    fn a_long_question_wraps_to_the_width() {
        let question = "Which store should this service use for the new feature, and why?";
        let modal = QuestionModal::new(text_question(question, false));
        let rows = modal.rows(20);
        let question_rows: Vec<&String> = rows
            .iter()
            .filter_map(|row| match row {
                QuestionRow::Question(line) => Some(line),
                _ => None,
            })
            .collect();
        assert!(question_rows.len() > 1, "{question_rows:?}");
        for line in question_rows {
            assert!(display_width(line) <= 20, "wider than the frame: {line:?}");
        }
    }

    #[test]
    fn a_long_question_scrolls_with_page_keys() {
        let question = "word ".repeat(80);
        let mut modal = QuestionModal::new(text_question(&question, false));
        assert_eq!(modal.question_offset(20, 4), 0, "the start is shown first");
        assert_eq!(modal.handle_key(key(KeyCode::PageDown)), Outcome::Stay);
        assert!(
            modal.question_offset(20, 4) > 0,
            "page down moves the question"
        );
        assert_eq!(modal.handle_key(key(KeyCode::PageUp)), Outcome::Stay);
        assert_eq!(modal.question_offset(20, 4), 0);
    }

    #[test]
    fn the_highlight_stays_in_view_when_the_options_do_not_fit() {
        let mut requested = request(true);
        requested.options.push(AskOption {
            label: "Postgres".to_owned(),
            description: None,
        });
        requested.options.push(AskOption {
            label: "MySQL".to_owned(),
            description: None,
        });
        let mut modal = QuestionModal::new(requested);
        for _ in 0..4 {
            assert_eq!(modal.handle_key(key(KeyCode::Down)), Outcome::Stay);
        }
        assert!(modal.is_freeform_selected());
        assert!(
            modal.interactive_offset(4) > 0,
            "the free-text row is drawn"
        );
    }

    #[test]
    fn the_free_text_row_shows_the_tail_of_a_long_answer() {
        let mut modal = QuestionModal::new(request(true));
        for character in "a long answer that will not fit in twenty columns".chars() {
            modal.handle_key(key(KeyCode::Char(character)));
        }
        let rows = modal.rows(20);
        let row = rows
            .iter()
            .find_map(|row| match row {
                QuestionRow::Freeform { text } => Some(text),
                _ => None,
            })
            .expect("a free-text row");
        assert!(display_width(row) <= 20, "{row:?}");
        assert!(row.ends_with("columns"), "{row:?}");
    }
}
