//! The screen's own state: the composer, the transcript, the status bar and the
//! keys that move between them.
//!
//! Everything here is synchronous and terminal-free, so a test can type a
//! prompt, feed [`AgentEvent`]s and assert exactly what the screen would draw
//! (RULES.md 8, 4).

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use niki_agent::{AgentEvent, Approval, ApprovalRequest, RunOutcome, StopReason, Usage};
use niki_policy::Mode;
use niki_render::Glyphs;
use niki_tui_modals::{ApprovalModal, HelpOverlay, Modal, ModelPicker, Outcome};
use niki_tui_themes::{GlyphMode, Motion, NIKI, PALETTES, Palette, next_palette, palette};

use crate::composer::Composer;
use crate::scroll::Scroll;
use crate::status::{Status, TurnState};
use crate::transcript::Transcript;

/// Spinner frames for a terminal that draws Unicode.
const UNICODE_SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
/// Spinner frames for a terminal that does not.
const ASCII_SPINNER: [char; 4] = ['|', '/', '-', '\\'];
/// Lines the composer grows to before it scrolls its own text.
const MAX_COMPOSER_LINES: u16 = 5;
/// Lines one wheel notch moves the transcript.
pub const WHEEL_LINES: usize = 3;

/// What a session starts with.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Model spec, `provider:model`.
    pub model: String,
    /// Approval mode to start in.
    pub mode: Mode,
    /// Directory the tools are rooted at.
    pub cwd: PathBuf,
}

/// What the screen asks the host to do after a key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Run this prompt as a turn.
    Submit(String),
    /// Stop the running turn; its output stays on screen.
    Cancel,
    /// Leave the application.
    Quit,
    /// The approval mode changed.
    Mode(Mode),
    /// The person answered the approval prompt.
    Answer(Approval),
    /// The person chose a model spec.
    SelectModel(String),
}

/// The whole screen state.
pub struct App {
    /// The conversation.
    pub transcript: Transcript,
    /// The prompt being typed.
    pub composer: Composer,
    /// The one-line summary.
    pub status: Status,
    /// How far the transcript is scrolled back.
    pub scroll: Scroll,
    /// Colours in use.
    pub palette: &'static Palette,
    /// Which marker registry to draw with.
    pub glyphs: GlyphMode,
    /// Whether the spinner may animate.
    pub motion: Motion,
    /// The command waiting for the person's answer, if one is.
    pub pending: Option<ApprovalRequest>,
    /// The overlay on top of the screen, if one is open.
    pub modal: Option<Modal>,
    /// The models the configuration offers, for `/model`.
    pub models: ModelPicker,
    /// Rows the transcript viewport had at the last draw, so one PageUp moves
    /// one screenful.
    pub page: usize,
    spinner: usize,
}

impl App {
    /// Start an empty session.
    #[must_use]
    pub fn new(info: SessionInfo) -> Self {
        Self {
            transcript: Transcript::default(),
            composer: Composer::default(),
            status: Status::new(info.model, info.mode, info.cwd.display().to_string()),
            scroll: Scroll::default(),
            palette: &NIKI,
            glyphs: GlyphMode::default(),
            motion: Motion::default(),
            pending: None,
            modal: None,
            models: ModelPicker::default(),
            page: 10,
            spinner: 0,
        }
    }

    /// Switch palette, which is what `/theme` does.
    pub fn set_palette(&mut self, palette: &'static Palette) {
        self.palette = palette;
    }

    /// Offer these models to the model picker.
    pub fn set_models(&mut self, models: ModelPicker) {
        self.models = models;
    }

    /// Switch the marker registry.
    pub fn set_glyphs(&mut self, glyphs: GlyphMode) {
        self.glyphs = glyphs;
    }

    /// Switch animation on or off.
    pub fn set_motion(&mut self, motion: Motion) {
        self.motion = motion;
    }

    /// The markers to draw with.
    #[must_use]
    pub fn marks(&self) -> Glyphs {
        Glyphs::for_ascii(self.glyphs.is_ascii())
    }

    /// The separator between status fields, in a character the terminal can
    /// draw.
    #[must_use]
    pub fn separator(&self) -> &'static str {
        if self.glyphs.is_ascii() {
            " | "
        } else {
            " · "
        }
    }

    /// The status bar as one line.
    #[must_use]
    pub fn status_line(&self) -> String {
        self.status.line(self.separator(), self.spinner())
    }

    /// The spinner frame, if one may animate.
    #[must_use]
    pub fn spinner(&self) -> Option<char> {
        if !self.status.turn.is_busy() || !self.motion.allows_spinner() {
            return None;
        }
        let frames: &[char] = if self.glyphs.is_ascii() {
            &ASCII_SPINNER
        } else {
            &UNICODE_SPINNER
        };
        frames.get(self.spinner % frames.len()).copied()
    }

    /// Advance the spinner; the screen calls this on every idle tick.
    pub fn tick(&mut self) {
        self.spinner = self.spinner.wrapping_add(1);
    }

    /// Rows the composer block needs: its text plus the frame around it.
    #[must_use]
    pub fn composer_height(&self) -> u16 {
        let lines = u16::try_from(self.composer.line_count()).unwrap_or(u16::MAX);
        lines.clamp(1, MAX_COMPOSER_LINES) + 2
    }

    /// Handle one key press.
    ///
    /// The composer owns the keys a person typing expects — characters,
    /// arrows, Home and End, Enter to send and Esc to clear — so the transcript
    /// is scrolled with PageUp, PageDown and the mouse wheel instead.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Command> {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('c') if control => {
                self.cancel();
                return Some(Command::Cancel);
            }
            KeyCode::Char('d') if control => return Some(Command::Quit),
            _ => {}
        }
        if self.modal.is_some() {
            return self.modal_key(key);
        }
        match key.code {
            KeyCode::Enter
                if key
                    .modifiers
                    .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) =>
            {
                self.composer.newline();
                None
            }
            KeyCode::Enter => self.submit(),
            KeyCode::Esc => self.escape(),
            KeyCode::Backspace => {
                self.composer.backspace();
                None
            }
            KeyCode::Delete => {
                self.composer.delete();
                None
            }
            KeyCode::Left => {
                self.composer.left();
                None
            }
            KeyCode::Right => {
                self.composer.right();
                None
            }
            KeyCode::Up => {
                self.composer.up();
                None
            }
            KeyCode::Down => {
                self.composer.down();
                None
            }
            KeyCode::Home => {
                self.composer.home();
                None
            }
            KeyCode::End => {
                self.composer.end();
                None
            }
            KeyCode::PageUp => {
                self.scroll.up(self.page.saturating_sub(1).max(1));
                None
            }
            KeyCode::PageDown => {
                self.scroll.down(self.page.saturating_sub(1).max(1));
                None
            }
            KeyCode::BackTab => {
                let mode = self.status.mode.cycle();
                self.status.mode = mode;
                Some(Command::Mode(mode))
            }
            KeyCode::Char('?') if self.composer.is_empty() => {
                self.open_help();
                None
            }
            KeyCode::Char(character) if !control && !key.modifiers.contains(KeyModifiers::ALT) => {
                self.composer.insert_char(character);
                None
            }
            _ => None,
        }
    }

    /// Switch palette by name, or to the next one when no name is given.
    pub fn theme(&mut self, name: Option<&str>) {
        match name {
            Some(name) => match palette(name) {
                Some(found) => {
                    self.palette = found;
                    self.transcript
                        .push_notice(format!("theme: {}", found.title));
                }
                None => {
                    let names: Vec<&str> = PALETTES.iter().map(|entry| entry.name).collect();
                    self.transcript.push_notice(format!(
                        "there is no {name} theme; try {}",
                        names.join(", ")
                    ));
                }
            },
            None => {
                let next = next_palette(self.palette.name);
                self.palette = next;
                self.transcript
                    .push_notice(format!("theme: {}", next.title));
            }
        }
    }

    /// Move the transcript one wheel notch, away from the newest line or back
    /// towards it.
    pub fn wheel(&mut self, up: bool) {
        if up {
            self.scroll.up(WHEEL_LINES);
        } else {
            self.scroll.down(WHEEL_LINES);
        }
    }

    /// Open the keymap overlay.
    pub fn open_help(&mut self) {
        self.modal = Some(Modal::Help(HelpOverlay::new()));
    }

    /// Open the model picker over the configured models.
    pub fn open_models(&mut self) {
        self.modal = Some(Modal::Models(self.models.clone()));
    }

    fn modal_key(&mut self, key: KeyEvent) -> Option<Command> {
        let outcome = self.modal.as_mut()?.handle_key(key);
        match outcome {
            Outcome::Stay => None,
            Outcome::Close => {
                self.modal = None;
                self.pending = None;
                None
            }
            Outcome::Answer(answer) => {
                self.modal = None;
                self.pending = None;
                self.status.turn = TurnState::Working;
                Some(Command::Answer(answer))
            }
            Outcome::Model(spec) => {
                self.modal = None;
                Some(Command::SelectModel(spec))
            }
        }
    }

    /// Drop the overlay and any answer it was waiting for.
    fn cancel(&mut self) {
        self.modal = None;
        self.pending = None;
        if self.status.turn == TurnState::Waiting {
            self.status.turn = TurnState::Working;
        }
    }

    /// Fold one event from the running turn into the screen.
    pub fn apply(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Text(delta) => {
                self.transcript.push_assistant(&delta);
                if self.status.turn == TurnState::Idle {
                    self.status.turn = TurnState::Working;
                }
            }
            AgentEvent::ToolStart { name, arguments } => {
                self.transcript.push_tool_call(&name, &arguments);
            }
            AgentEvent::ToolEnd {
                name,
                output,
                is_error,
            } => self.transcript.push_tool_result(&name, &output, is_error),
            AgentEvent::Usage(usage) => add_usage(&mut self.status.tokens, usage),
            AgentEvent::Notice(text) => self.transcript.push_notice(text),
            AgentEvent::StepLimit { steps } => self
                .transcript
                .push_notice(format!("stopped after {steps} model turns")),
            AgentEvent::Cancelled => {
                self.transcript.end_stream();
                self.transcript.push_notice("cancelled");
                self.status.turn = TurnState::Cancelled;
            }
        }
    }

    /// Record that the policy needs an answer before a command runs, and ask.
    pub fn request_approval(&mut self, request: ApprovalRequest) {
        self.modal = Some(Modal::Approval(ApprovalModal::new(request.clone())));
        self.pending = Some(request);
        self.status.turn = TurnState::Waiting;
    }

    /// Fold the end of a turn into the screen.
    pub fn finish(&mut self, result: Result<RunOutcome, String>) {
        self.pending = None;
        self.transcript.end_stream();
        match result {
            Ok(outcome) => {
                self.status.turn = if is_cancelled(&outcome.stop) {
                    TurnState::Cancelled
                } else {
                    TurnState::Idle
                };
            }
            Err(message) => {
                self.transcript
                    .push_error(format!("the turn failed: {message}"));
                self.status.turn = TurnState::Failed;
            }
        }
    }

    fn escape(&mut self) -> Option<Command> {
        if !self.composer.is_empty() {
            self.composer.clear();
            return None;
        }
        if self.status.turn == TurnState::Working {
            return Some(Command::Cancel);
        }
        None
    }

    fn submit(&mut self) -> Option<Command> {
        let text = self.composer.text().trim().to_owned();
        if text.is_empty() {
            return None;
        }
        if text.starts_with('/') {
            return self.slash(&text);
        }
        self.composer.clear();
        self.transcript.push_user(text.clone());
        self.scroll.reset();
        self.status.turn = TurnState::Working;
        self.spinner = 0;
        Some(Command::Submit(text))
    }

    fn slash(&mut self, text: &str) -> Option<Command> {
        self.composer.clear();
        let mut words = text.split_whitespace();
        let name = words.next().unwrap_or_default();
        let argument = words.next();
        match name {
            "/quit" | "/exit" => Some(Command::Quit),
            "/help" | "/keys" => {
                self.open_help();
                None
            }
            "/theme" => {
                self.theme(argument);
                None
            }
            "/model" => match argument {
                Some(spec) => Some(Command::SelectModel(spec.to_owned())),
                None => {
                    self.open_models();
                    None
                }
            },
            other => {
                self.transcript.push_notice(format!(
                    "there is no {other} command; try /help, /model, /theme or /quit"
                ));
                None
            }
        }
    }
}

fn add_usage(total: &mut Usage, usage: Usage) {
    total.input_tokens += usage.input_tokens;
    total.output_tokens += usage.output_tokens;
}

fn is_cancelled(stop: &StopReason) -> bool {
    matches!(stop, StopReason::Other(reason) if reason == "cancelled")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::RowKind;

    fn app() -> App {
        App::new(SessionInfo {
            model: "mock:mock-model".to_owned(),
            mode: Mode::Manual,
            cwd: PathBuf::from("/work"),
        })
    }

    fn press(app: &mut App, code: KeyCode) -> Option<Command> {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn stroke(app: &mut App, code: KeyCode) {
        let _ = press(app, code);
    }

    fn type_text(app: &mut App, text: &str) {
        for character in text.chars() {
            stroke(app, KeyCode::Char(character));
        }
    }

    #[test]
    fn typing_and_enter_sends_a_prompt() {
        let mut app = app();
        type_text(&mut app, "list the files");
        assert_eq!(app.composer.text(), "list the files");
        assert_eq!(app.status.turn, TurnState::Idle);
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::Submit("list the files".to_owned()))
        );
        assert!(app.composer.is_empty());
        assert_eq!(app.transcript.rows().len(), 1);
        assert_eq!(app.transcript.rows()[0].kind, RowKind::User);
        assert_eq!(app.status.turn, TurnState::Working);
    }

    #[test]
    fn an_empty_enter_sends_nothing() {
        let mut app = app();
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        type_text(&mut app, "   ");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert!(app.transcript.is_empty());
    }

    #[test]
    fn alt_enter_starts_a_second_line_instead_of_sending() {
        let mut app = app();
        type_text(&mut app, "first");
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
        assert_eq!(app.handle_key(key), None);
        type_text(&mut app, "second");
        assert_eq!(app.composer.line_count(), 2);
        assert_eq!(app.composer_height(), 4);
        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(Command::Submit("first\nsecond".to_owned()))
        );
    }

    #[test]
    fn escape_clears_the_draft_before_it_stops_a_turn() {
        let mut app = app();
        type_text(&mut app, "half a thought");
        assert_eq!(press(&mut app, KeyCode::Esc), None);
        assert!(app.composer.is_empty());

        app.status.turn = TurnState::Working;
        assert_eq!(press(&mut app, KeyCode::Esc), Some(Command::Cancel));
        app.status.turn = TurnState::Idle;
        assert_eq!(press(&mut app, KeyCode::Esc), None);
    }

    #[test]
    fn control_keys_quit_and_cancel() {
        let mut app = app();
        type_text(&mut app, "keep this");
        let cancel = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(cancel), Some(Command::Cancel));
        assert_eq!(
            app.composer.text(),
            "keep this",
            "a cancel must not throw away the draft"
        );
        let quit = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(quit), Some(Command::Quit));
    }

    #[test]
    fn the_quit_command_leaves_and_an_unknown_one_explains_itself() {
        let mut app = app();
        type_text(&mut app, "/quit");
        assert_eq!(press(&mut app, KeyCode::Enter), Some(Command::Quit));

        type_text(&mut app, "/nope");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        let last = app.transcript.rows().last().expect("a notice");
        assert_eq!(last.kind, RowKind::Notice);
        assert!(last.text.contains("/nope"), "{}", last.text);
        assert!(app.composer.is_empty());
    }

    #[test]
    fn backtab_cycles_the_approval_mode() {
        let mut app = app();
        assert_eq!(
            press(&mut app, KeyCode::BackTab),
            Some(Command::Mode(Mode::Auto))
        );
        assert_eq!(app.status.mode, Mode::Auto);
        assert_eq!(
            press(&mut app, KeyCode::BackTab),
            Some(Command::Mode(Mode::Yolo))
        );
        assert_eq!(
            press(&mut app, KeyCode::BackTab),
            Some(Command::Mode(Mode::Manual))
        );
    }

    #[test]
    fn page_up_scrolls_back_and_page_down_returns_to_the_newest_line() {
        let mut app = app();
        app.page = 11;
        stroke(&mut app, KeyCode::PageUp);
        assert_eq!(app.scroll.offset(), 10);
        stroke(&mut app, KeyCode::PageUp);
        assert_eq!(app.scroll.offset(), 20);
        stroke(&mut app, KeyCode::PageDown);
        assert_eq!(app.scroll.offset(), 10);
        stroke(&mut app, KeyCode::PageDown);
        assert_eq!(app.scroll.offset(), 0);
    }

    #[test]
    fn the_wheel_moves_the_transcript_a_few_lines_at_a_time() {
        let mut app = app();
        app.wheel(true);
        assert_eq!(app.scroll.offset(), WHEEL_LINES);
        app.wheel(true);
        assert_eq!(app.scroll.offset(), WHEEL_LINES * 2);
        app.wheel(false);
        assert_eq!(app.scroll.offset(), WHEEL_LINES);
        app.wheel(false);
        app.wheel(false);
        assert_eq!(app.scroll.offset(), 0, "it stops at the newest line");
    }

    #[test]
    fn answer_text_and_tool_activity_build_the_transcript() {
        let mut app = app();
        app.status.turn = TurnState::Working;
        app.apply(AgentEvent::Text("Looking at ".to_owned()));
        app.apply(AgentEvent::Text("the tree.".to_owned()));
        app.apply(AgentEvent::ToolStart {
            name: "shell".to_owned(),
            arguments: "{\"command\":\"ls\"}".to_owned(),
        });
        app.apply(AgentEvent::ToolEnd {
            name: "shell".to_owned(),
            output: "Cargo.toml\nsrc".to_owned(),
            is_error: false,
        });
        app.apply(AgentEvent::Text("Two entries.".to_owned()));

        let rows = app.transcript.rows();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].kind, RowKind::Assistant);
        assert_eq!(rows[0].text, "Looking at the tree.");
        assert_eq!(rows[1].kind, RowKind::ToolCall);
        assert_eq!(rows[1].title.as_deref(), Some("shell"));
        assert_eq!(rows[2].kind, RowKind::ToolResult);
        assert_eq!(rows[2].text, "Cargo.toml\nsrc");
        assert_eq!(rows[3].kind, RowKind::Assistant);
        assert_eq!(app.status.turn, TurnState::Working);
    }

    #[test]
    fn usage_and_notices_reach_the_status_and_the_transcript() {
        let mut app = app();
        app.apply(AgentEvent::Usage(Usage {
            input_tokens: 12,
            output_tokens: 5,
        }));
        app.apply(AgentEvent::Usage(Usage {
            input_tokens: 3,
            output_tokens: 1,
        }));
        assert_eq!(app.status.tokens.input_tokens, 15);
        assert_eq!(app.status.tokens.output_tokens, 6);
        assert!(app.status_line().contains("15 in / 6 out"));

        app.apply(AgentEvent::StepLimit { steps: 40 });
        let last = app.transcript.rows().last().expect("a notice");
        assert_eq!(last.kind, RowKind::Notice);
        assert!(last.text.contains("40"), "{}", last.text);
    }

    #[test]
    fn a_cancelled_turn_keeps_what_it_wrote() {
        let mut app = app();
        app.status.turn = TurnState::Working;
        app.apply(AgentEvent::Text("half an answer".to_owned()));
        app.apply(AgentEvent::Cancelled);
        assert_eq!(app.status.turn, TurnState::Cancelled);
        assert_eq!(app.transcript.rows()[0].text, "half an answer");
        assert_eq!(
            app.transcript.rows().last().map(|row| row.text.as_str()),
            Some("cancelled")
        );

        let mut outcome = RunOutcome {
            text: String::new(),
            steps: 1,
            tool_calls: 0,
            stop: StopReason::Other("cancelled".to_owned()),
            usage: Usage::default(),
        };
        app.finish(Ok(outcome.clone()));
        assert_eq!(app.status.turn, TurnState::Cancelled);

        outcome.stop = StopReason::EndTurn;
        app.finish(Ok(outcome));
        assert_eq!(app.status.turn, TurnState::Idle);
    }

    #[test]
    fn a_failed_turn_says_so_in_the_transcript() {
        let mut app = app();
        app.status.turn = TurnState::Working;
        app.finish(Err("provider mock failed: 503".to_owned()));
        assert_eq!(app.status.turn, TurnState::Failed);
        let last = app.transcript.rows().last().expect("an error");
        assert_eq!(last.kind, RowKind::Error);
        assert!(last.text.contains("503"), "{}", last.text);
    }

    #[test]
    fn an_approval_request_waits_for_the_person() {
        let mut app = app();
        app.request_approval(ApprovalRequest {
            command: "rm -rf build".to_owned(),
            mode: Mode::Manual,
        });
        assert_eq!(app.status.turn, TurnState::Waiting);
        assert_eq!(
            app.pending.as_ref().map(|request| request.command.as_str()),
            Some("rm -rf build")
        );
        assert!(app.status_line().contains("waiting for you"));
    }

    #[test]
    fn an_approval_request_opens_a_prompt_that_y_answers() {
        let mut app = app();
        app.request_approval(ApprovalRequest {
            command: "cargo publish".to_owned(),
            mode: Mode::Manual,
        });
        assert!(matches!(app.modal, Some(Modal::Approval(_))));
        let command = press(&mut app, KeyCode::Char('y'));
        assert_eq!(command, Some(Command::Answer(Approval::Once)));
        assert!(app.modal.is_none());
        assert!(app.pending.is_none());
        assert_eq!(app.status.turn, TurnState::Working);
    }

    #[test]
    fn an_approval_prompt_can_be_refused_with_n_or_escape() {
        let mut app = app();
        app.request_approval(ApprovalRequest {
            command: "rm -rf build".to_owned(),
            mode: Mode::Auto,
        });
        assert_eq!(
            press(&mut app, KeyCode::Char('n')),
            Some(Command::Answer(Approval::Deny))
        );

        app.request_approval(ApprovalRequest {
            command: "rm -rf build".to_owned(),
            mode: Mode::Auto,
        });
        assert_eq!(
            press(&mut app, KeyCode::Esc),
            Some(Command::Answer(Approval::Deny))
        );
        assert!(app.modal.is_none());
    }

    #[test]
    fn a_modal_keeps_the_composer_to_itself() {
        let mut app = app();
        app.open_help();
        type_text(&mut app, "hello");
        assert!(
            app.composer.is_empty(),
            "typing must not reach the composer"
        );
        assert_eq!(press(&mut app, KeyCode::Esc), None);
        assert!(app.modal.is_none());

        app.open_help();
        type_text(&mut app, "?");
        assert!(app.modal.is_none(), "? closes the keymap");
    }

    #[test]
    fn question_mark_opens_help_only_on_an_empty_draft() {
        let mut app = app();
        type_text(&mut app, "why?");
        assert!(app.modal.is_none(), "? belongs to the draft while typing");
        assert_eq!(app.composer.text(), "why?");
        app.composer.clear();
        stroke(&mut app, KeyCode::Char('?'));
        assert!(matches!(app.modal, Some(Modal::Help(_))));
    }

    #[test]
    fn the_model_commands_open_the_picker_or_switch_directly() {
        let mut app = app();
        app.set_models(ModelPicker::from_config(&niki_config::Config::default()));
        type_text(&mut app, "/model");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert!(matches!(app.modal, Some(Modal::Models(_))));
        assert_eq!(press(&mut app, KeyCode::Esc), None);
        assert!(app.modal.is_none());

        type_text(&mut app, "/model groq:llama");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::SelectModel("groq:llama".to_owned()))
        );
        assert!(app.modal.is_none());
    }

    #[test]
    fn the_theme_command_selects_a_palette_by_name_or_cycles() {
        let mut app = app();
        type_text(&mut app, "/theme mono");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert_eq!(app.palette.name, "mono");
        assert!(app.composer.is_empty());

        type_text(&mut app, "/theme");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.palette.name, "niki", "cycling wraps from the last");

        type_text(&mut app, "/theme");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.palette.name, "high-contrast");
    }

    #[test]
    fn an_unknown_theme_says_what_is_on_offer_and_changes_nothing() {
        let mut app = app();
        type_text(&mut app, "/theme purple");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert_eq!(app.palette.name, "niki");
        let last = app.transcript.rows().last().expect("a notice");
        assert_eq!(last.kind, RowKind::Notice);
        for name in ["niki", "high-contrast", "colorblind", "mono"] {
            assert!(
                last.text.contains(name),
                "{name} missing from {}",
                last.text
            );
        }
    }

    #[test]
    fn the_help_command_opens_the_keymap() {
        let mut app = app();
        type_text(&mut app, "/help");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert!(matches!(app.modal, Some(Modal::Help(_))));
        assert!(app.composer.is_empty());
    }

    #[test]
    fn cancelling_closes_a_prompt_without_leaving_the_turn_waiting() {
        let mut app = app();
        app.request_approval(ApprovalRequest {
            command: "cargo publish".to_owned(),
            mode: Mode::Manual,
        });
        let cancel = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(cancel), Some(Command::Cancel));
        assert!(app.modal.is_none());
        assert!(app.pending.is_none());
        assert_eq!(app.status.turn, TurnState::Working);
    }

    #[test]
    fn the_spinner_animates_only_when_it_may() {
        let mut app = app();
        app.status.turn = TurnState::Working;
        let first = app.spinner().expect("a frame while working");
        assert!(app.status_line().contains(first));
        app.tick();
        let second = app.spinner().expect("a frame while working");
        assert_ne!(first, second);

        app.set_motion(Motion::Reduced);
        assert_eq!(app.spinner(), None);
        assert!(!app.status_line().contains('⠋'));

        app.set_motion(Motion::Full);
        app.set_glyphs(GlyphMode::Ascii);
        let frame = app.spinner().expect("a frame while working");
        assert!(frame.is_ascii(), "{frame}");
    }

    #[test]
    fn the_status_line_uses_separators_the_terminal_can_draw() {
        let mut app = app();
        assert_eq!(app.status_line(), "mock:mock-model · manual · idle · /work");
        app.set_glyphs(GlyphMode::Ascii);
        assert_eq!(app.status_line(), "mock:mock-model | manual | idle | /work");
        app.set_glyphs(GlyphMode::Unicode);
        assert!(!app.status_line().is_ascii());
    }

    #[test]
    fn the_composer_grows_with_its_text_up_to_a_limit() {
        let mut app = app();
        assert_eq!(app.composer_height(), 3);
        for line in 0..10 {
            if line > 0 {
                let _ = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT));
            }
            type_text(&mut app, "x");
        }
        assert_eq!(app.composer_height(), MAX_COMPOSER_LINES + 2);
    }

    #[test]
    fn marks_follow_the_glyph_mode() {
        let mut app = app();
        assert_eq!(app.marks().user, "›");
        app.set_glyphs(GlyphMode::Ascii);
        assert_eq!(app.marks().user, ">");
        assert_eq!(app.palette.name, "niki");
    }
}
