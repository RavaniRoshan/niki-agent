//! The screen's own state: the composer, the transcript, the status bar and the
//! keys that move between them.
//!
//! Everything here is synchronous and terminal-free, so a test can type a
//! prompt, feed [`AgentEvent`]s and assert exactly what the screen would draw
//! (RULES.md 8, 4).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use niki_agent::{
    AgentEvent, Approval, ApprovalRequest, Compaction, ContextReport, RunOutcome, SessionUsage,
    StopReason, Usage,
};
use niki_commands::SlashCommand;
use niki_config::{Config, Protocol};
use niki_policy::{AllowList, Decision, Mode, ProgramAllow, decide};
use niki_render::{DiffTheme, Glyphs};
use niki_session::{SessionId, SessionStore, Summary};
use niki_skills::SkillCatalog;
use niki_tools::AskRequest;
use niki_tui_modals::{
    ApprovalModal, ConnectModal, ConnectRequest, HelpOverlay, Modal, ModelPicker, Outcome,
    QuestionModal, QuestionReply, StatusCard, StatusField, ThreadEntry, ThreadPicker,
};
use niki_tui_themes::{Effects, GlyphMode, Motion, NIKI, PALETTES, Palette, next_palette, palette};

use crate::completion::Completion;
use crate::composer::Composer;
use crate::diff_source::{self, DiffSection};
use crate::history::History;
use crate::motion;
use crate::scroll::Scroll;
use crate::status::{Status, TurnState, context_percentage, describe_context, format_tokens};
use crate::transcript::{Row, RowKind, Transcript};
use crate::transcript_overlay::TranscriptOverlay;

/// Lines the composer grows to before it scrolls its own text.
const MAX_COMPOSER_LINES: u16 = 5;
/// Lines one wheel notch moves the transcript.
pub const WHEEL_LINES: usize = 3;
/// Newest rows `/compact` keeps in the view.
const COMPACT_KEEP_ROWS: usize = 12;
/// How many sessions `/resume` lists.
const RESUME_LIMIT: usize = 20;
/// How long after one Esc a second one restores the last prompt.
const BACKTRACK_WINDOW: Duration = Duration::from_secs(2);

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
    /// Hand this line to the turn that is running.
    Steer(String),
    /// Run this `!` line locally, without the model.
    Shell(String),
    /// Drop the newest prompt and its answer from the agent's history.
    Rewind,
    /// Stop the running turn; its output stays on screen.
    Cancel,
    /// Leave the application.
    Quit,
    /// The approval mode changed.
    Mode(Mode),
    /// The person answered the approval prompt.
    Answer(Approval),
    /// The person answered, or dismissed, a structured question.
    AnswerQuestion(QuestionReply),
    /// Start a new session: drop the in-memory "for this session" rules.
    ClearApprovals,
    /// The person chose a model spec.
    SelectModel(String),
    /// Compact the agent's history through its provider.
    Compact,
    /// Ask the agent for its context report.
    Context,
    /// Replace the agent's system prompt, recomposed after `/init`.
    RefreshSystem(String),
    /// Put this text on the terminal clipboard.
    Copy(String),
    /// Hand the draft to the person's editor.
    EditPrompt,
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
    /// Which named effects are drawn at all.
    pub effects: Effects,
    /// The command waiting for the person's answer, if one is.
    pub pending: Option<ApprovalRequest>,
    /// The structured question waiting for the person's answer, if one is.
    pub question: Option<AskRequest>,
    /// The overlay on top of the screen, if one is open.
    pub modal: Option<Modal>,
    /// The whole conversation on one screen, opened by `Ctrl+T`.
    pub transcript_overlay: Option<TranscriptOverlay>,
    /// The models the configuration offers, for `/model`.
    pub models: ModelPicker,
    /// The configuration the session started from, for `/connect`.
    config: Option<Config>,
    /// The file it came from, when the host named one.
    config_path: Option<PathBuf>,
    /// The skills discovery found for this session, shown by `/skills`.
    skills: SkillCatalog,
    /// Recomposes the system prompt after `/init` writes a file.
    refresh_system: Option<Arc<dyn Fn() -> String + Send + Sync>>,
    /// Rows the transcript viewport had at the last draw, so one PageUp moves
    /// one screenful.
    pub page: usize,
    /// The session store, when the host opened one.
    sessions: Option<SessionStore>,
    session: Option<SessionId>,
    /// First transcript row of the turn in flight, so only that turn is saved.
    turn_start: Option<usize>,
    /// The file the session store writes, when the host named it.
    session_file: Option<PathBuf>,
    /// Whether any turn has been written to the store yet.
    saved_turn: bool,
    /// Tool names the registry holds, for `/tools`.
    tools: Vec<String>,
    /// Prompts the arrows walk back through.
    history: History,
    /// Prompts typed while a turn was running, oldest first.
    queued: VecDeque<String>,
    /// Shell allow-list the live mode consults for a `!` line.
    allow: AllowList,
    /// Programs allowed for the rest of this session by the approval dialog.
    /// Cleared when a new session starts; never written to configuration.
    session_allow: ProgramAllow,
    /// Whether the host can put a `!` command's question to the person.
    approver: bool,
    /// A `!` command waiting for its approval answer.
    local_shell: Option<String>,
    /// When Esc was last pressed with an empty composer.
    last_escape: Option<Instant>,
    /// The command list under the composer, when the draft matches one.
    completion: Option<Completion>,
    /// The draft whose command list Esc closed, until the draft changes.
    completion_closed: Option<String>,
    /// Whether `Ctrl+Y` may write the clipboard escape.
    clipboard: bool,
    spinner: usize,
    /// When the running turn was first seen, for the clock on the busy row.
    turn_clock: Option<Instant>,
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
            effects: Effects::default(),
            pending: None,
            question: None,
            modal: None,
            transcript_overlay: None,
            models: ModelPicker::default(),
            config: None,
            config_path: None,
            skills: SkillCatalog::default(),
            refresh_system: None,
            page: 10,
            sessions: None,
            session: None,
            turn_start: None,
            session_file: None,
            saved_turn: false,
            tools: Vec::new(),
            history: History::default(),
            queued: VecDeque::new(),
            allow: AllowList::default(),
            session_allow: ProgramAllow::default(),
            approver: false,
            local_shell: None,
            last_escape: None,
            completion: None,
            completion_closed: None,
            clipboard: !clipboard_disabled(),
            spinner: 0,
            turn_clock: None,
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

    /// Remember the configuration and the file it came from, which `/connect`
    /// reads and writes.
    pub fn set_config(&mut self, config: Config, path: PathBuf) {
        self.models = ModelPicker::from_config(&config);
        self.config = Some(config);
        self.config_path = Some(path);
    }

    /// Offer the skills discovery found, which `/skills` prints.
    pub fn set_skills(&mut self, skills: SkillCatalog) {
        self.skills = skills;
    }

    /// Name the checked-out branch, which the state line shows next to the mode.
    pub fn set_branch(&mut self, branch: Option<String>) {
        self.status.set_branch(branch);
    }

    /// Let `/init` recompose the system prompt once it has written a file.
    ///
    /// The host owns the composition, so the screen only asks for the new text
    /// and hands it to the worker; a session that never runs `/init` never
    /// calls it.
    pub fn set_system_refresh(&mut self, refresh: impl Fn() -> String + Send + Sync + 'static) {
        self.refresh_system = Some(Arc::new(refresh));
    }

    /// Save turns and resume earlier ones through this store.
    ///
    /// A session row is created immediately, so quitting before the first turn
    /// still leaves a session `/resume` can show.
    pub fn attach_sessions(&mut self, store: SessionStore) {
        self.sessions = Some(store);
        self.new_session();
    }

    /// The session the screen is writing to, when the host opened a store.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.session.as_ref().map(SessionId::as_str)
    }

    /// Tell the status card where the session store writes.
    ///
    /// The host opens the database, so it owns the path. A host that never
    /// calls this leaves the card printing `unknown` for that line.
    pub fn set_session_file(&mut self, path: impl Into<PathBuf>) {
        self.session_file = Some(path.into());
    }

    /// The tool names `/tools` lists.
    pub fn set_tools(&mut self, tools: Vec<String>) {
        self.tools = tools;
    }

    /// Read and append the prompt history at this file from now on.
    ///
    /// The screen starts with none so a test never touches the person's file;
    /// the event loop attaches the one under `$NIKI_HOME` at startup.
    pub fn attach_history(&mut self, history: History) {
        self.history = history;
    }

    /// The prompt history, newest last.
    #[must_use]
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Allow or refuse `Ctrl+Y`, which a test uses in place of the environment.
    pub fn set_clipboard(&mut self, enabled: bool) {
        self.clipboard = enabled;
    }

    /// Give `!` lines the shell allow-list the agent was built with.
    pub fn set_allow_list(&mut self, allow: AllowList) {
        self.allow = allow;
    }

    /// Say whether the host can put a `!` command's question to the person.
    ///
    /// Without an approver such a command is refused, as a headless shell call
    /// is, rather than waiting for an answer nobody can give.
    pub fn set_approver(&mut self, available: bool) {
        self.approver = available;
    }

    /// The prompts waiting for the turn in flight to end, oldest first.
    #[must_use]
    pub fn queued(&self) -> &VecDeque<String> {
        &self.queued
    }

    /// The command list under the composer, when one is open.
    #[must_use]
    pub fn completion(&self) -> Option<&Completion> {
        self.completion.as_ref()
    }

    /// Insert pasted text literally.
    ///
    /// A bracketed paste is text, not keys: newlines and colons stay in the
    /// draft and only Enter sends it. While a modal is open the modal owns the
    /// keyboard, so the paste is dropped rather than reaching the composer.
    pub fn handle_paste(&mut self, text: &str) {
        if self.modal.is_some() || self.transcript_overlay.is_some() {
            return;
        }
        self.composer.insert_str(text);
        self.history.reset();
        self.refresh_completion();
    }

    /// Switch the marker registry.
    pub fn set_glyphs(&mut self, glyphs: GlyphMode) {
        self.glyphs = glyphs;
    }

    /// Switch animation on or off.
    pub fn set_motion(&mut self, motion: Motion) {
        self.motion = motion;
    }

    /// Switch the named effects on or off.
    pub fn set_effects(&mut self, effects: Effects) {
        self.effects = effects;
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

    /// The status bar's state line: what the session is, with the queue depth
    /// when one is waiting.
    ///
    /// Counts live on [`Self::accounting_line`] instead, so the directory never
    /// scrolls off a long session.
    #[must_use]
    pub fn status_line(&self) -> String {
        // While a turn runs the live row above the composer already carries the
        // spinner and the state word; repeating them here would say "working"
        // twice, one line apart.
        let line = if self.status.turn == TurnState::Working {
            self.status.context_line(self.separator())
        } else {
            self.status.state_line(self.separator(), self.spinner())
        };
        if self.queued.is_empty() {
            line
        } else {
            format!("{line}{}queued {}", self.separator(), self.queued.len())
        }
    }

    /// The context and token accounting, once the agent has reported any.
    #[must_use]
    pub fn accounting_line(&self) -> Option<String> {
        self.status.accounting(self.separator())
    }

    /// Start the clock the first time a busy turn is drawn, and clear it once
    /// the turn ends.
    ///
    /// Called from the draw path so every transition — submit, steer, approve,
    /// cancel, fail — is covered without each one having to remember.
    pub fn sync_turn_clock(&mut self) {
        if !self.status.turn.is_busy() {
            self.turn_clock = None;
        } else if self.turn_clock.is_none() {
            self.turn_clock = Some(Instant::now());
        }
    }

    /// The live row above the composer while a turn runs.
    ///
    /// An elapsed clock is the difference between "something is happening" and
    /// "something happened two hundred seconds ago and may be wedged". The
    /// interrupt key is named here rather than only in the key list because
    /// this is the moment a person looks for it.
    #[must_use]
    pub fn working_line(&self) -> Option<String> {
        let started = self
            .turn_clock
            .filter(|_| self.status.turn == TurnState::Working)?;
        let elapsed = Instant::now().saturating_duration_since(started);
        Some(format!(
            "{} Working ({}{}esc to interrupt)",
            self.activity_glyph(),
            crate::status::format_elapsed(elapsed),
            self.separator()
        ))
    }

    /// Whether anything real has happened: a prompt, a turn, or a token count.
    ///
    /// A notice — a trust warning, a theme change — is not a start.
    #[must_use]
    pub fn started(&self) -> bool {
        self.transcript.has_conversation()
            || self.status.turn != TurnState::Idle
            || self.status.tokens != Usage::default()
    }

    /// The key hints for the footer's second line, shortened to fit `width`.
    ///
    /// The list runs most useful first, so a narrow terminal keeps `/` and `?`
    /// and drops the rest rather than clipping one in half.
    #[must_use]
    pub fn hints(&self, width: usize) -> String {
        const HINTS: [(&str, &str); 4] = [
            ("/", "commands"),
            ("?", "keys"),
            ("ctrl+t", "transcript"),
            ("ctrl+d", "quit"),
        ];
        let separator = self.separator();
        let mut line = String::new();
        for (index, (key, action)) in HINTS.iter().enumerate() {
            let hint = format!("{key} {action}");
            let candidate = if index == 0 {
                hint
            } else {
                format!("{line}{separator}{hint}")
            };
            if niki_render::display_width(&candidate) > width {
                break;
            }
            line = candidate;
        }
        line
    }

    /// The spinner frame, if one may animate.
    ///
    /// Reduced motion answers a static `◌` rather than nothing: the turn is
    /// still visibly working. With the `progress` effect off there is no
    /// glyph at all, and the state word alone carries the information.
    #[must_use]
    pub fn spinner(&self) -> Option<char> {
        if !self.status.turn.is_busy() || !self.effects.progress {
            return None;
        }
        if !self.motion.allows_spinner() {
            return Some(if self.glyphs.is_ascii() {
                motion::RESTING_ASCII
            } else {
                motion::RESTING_UNICODE
            });
        }
        let frames: &[char] = if self.glyphs.is_ascii() {
            &motion::ASCII_LOADING
        } else {
            &motion::UNICODE_LOADING
        };
        frames.get(self.spinner % frames.len()).copied()
    }

    /// The marker on a tool call that is still running.
    ///
    /// Without motion or without the `shimmer` effect it stays on the first
    /// frame, so a long step is still marked as alive rather than frozen.
    #[must_use]
    pub fn activity_glyph(&self) -> char {
        let frames = if self.glyphs.is_ascii() {
            motion::ACTIVITY_ASCII
        } else {
            motion::ACTIVITY_UNICODE
        };
        if !self.effects.shimmer || !self.motion.allows_spinner() {
            return frames[0];
        }
        frames[(self.spinner / motion::ACTIVITY_TICKS) % frames.len()]
    }

    /// Advance the spinner; the screen calls this on every idle tick.
    pub fn tick(&mut self) {
        self.spinner = self.spinner.wrapping_add(1);
    }

    /// The terminal window title: the project, then what the turn is doing.
    ///
    /// A terminal the person is not looking at still says whether the turn is
    /// working, and while it waits for an answer the marker blinks every half
    /// second. Reduced motion or the `title` effect off makes it static; ASCII
    /// mode uses ` - ` and `*`/`o`. Control characters are stripped so a path
    /// cannot inject a terminal escape.
    #[must_use]
    pub fn title(&self) -> String {
        let project = Path::new(&self.status.cwd)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("niki");
        let separator = if self.glyphs.is_ascii() {
            " - "
        } else {
            " — "
        };
        let (state, marker) = match self.status.turn {
            TurnState::Idle => ("idle", None),
            TurnState::Working => ("working", None),
            TurnState::Waiting => {
                let (on, off) = if self.glyphs.is_ascii() {
                    ('*', 'o')
                } else {
                    ('●', '○')
                };
                let lit = !self.motion.allows_spinner() || (self.spinner / 5) % 2 == 0;
                ("needs you", Some(if lit { on } else { off }))
            }
            TurnState::Cancelled => ("cancelled", None),
            TurnState::Failed => ("failed", None),
        };
        let title = match marker {
            Some(marker) => format!("{marker} {project}{separator}{state}"),
            None => format!("{project}{separator}{state}"),
        };
        title
            .chars()
            .filter(|character| !character.is_control())
            .collect()
    }

    /// Rows the composer block needs: its text plus the rule above it.
    #[must_use]
    pub fn composer_height(&self) -> u16 {
        let lines = u16::try_from(self.composer.line_count()).unwrap_or(u16::MAX);
        lines.clamp(1, MAX_COMPOSER_LINES) + 1
    }

    /// Rows the command list needs under the composer: one per match and one
    /// for the highlighted command's description.
    #[must_use]
    pub fn completion_height(&self) -> u16 {
        match &self.completion {
            Some(completion) => {
                let rows = completion.commands().len() + 1;
                u16::try_from(rows).unwrap_or(u16::MAX)
            }
            None => 0,
        }
    }

    /// Handle one key press.
    ///
    /// The composer owns the keys a person typing expects — characters,
    /// arrows, Home and End, Enter to send and Esc to clear — so the transcript
    /// is scrolled with PageUp, PageDown and the mouse wheel instead. The
    /// command list and the prompt history take the arrows while each is open.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Command> {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('c') if control => {
                self.cancel();
                return Some(Command::Cancel);
            }
            KeyCode::Char('d') if control => {
                self.queued.clear();
                return Some(Command::Quit);
            }
            _ => {}
        }
        if self.modal.is_some() {
            return self.modal_key(key);
        }
        if self.transcript_overlay.is_some() {
            let stays = self
                .transcript_overlay
                .as_mut()
                .is_some_and(|overlay| overlay.handle_key(key));
            if !stays {
                self.transcript_overlay = None;
            }
            return None;
        }
        let command = match key.code {
            KeyCode::Enter => self.enter(key.modifiers),
            KeyCode::Esc => self.escape(),
            KeyCode::Tab if !key.modifiers.contains(KeyModifiers::SHIFT) => self.complete(),
            KeyCode::Backspace => {
                self.composer.backspace();
                self.history.reset();
                None
            }
            KeyCode::Delete => {
                self.composer.delete();
                self.history.reset();
                None
            }
            KeyCode::Left => {
                self.composer.left();
                self.history.reset();
                None
            }
            KeyCode::Right => {
                self.composer.right();
                self.history.reset();
                None
            }
            KeyCode::Up => {
                self.up();
                None
            }
            KeyCode::Down => {
                self.down();
                None
            }
            KeyCode::Home => {
                self.composer.home();
                self.history.reset();
                None
            }
            KeyCode::End => {
                self.composer.end();
                self.history.reset();
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
            KeyCode::Char('y') if control => self.copy(),
            KeyCode::Char('g') if control => Some(Command::EditPrompt),
            KeyCode::Char('t') if control => {
                self.open_transcript();
                None
            }
            KeyCode::Char('?') if self.composer.is_empty() => {
                self.open_help();
                None
            }
            KeyCode::Char(character) if !control && !key.modifiers.contains(KeyModifiers::ALT) => {
                self.composer.insert_char(character);
                self.history.reset();
                None
            }
            _ => None,
        };
        self.refresh_completion();
        command
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

    /// Print the discovered skills into the transcript.
    ///
    /// One row per skill, with the description and the path to read: the same
    /// three things the model is given, so a person can tell why the model
    /// reached for a skill and where its instructions live.
    fn show_skills(&mut self) {
        if self.skills.is_empty() {
            self.transcript
                .push_notice("no skills found; add one at .niki/skills/<name>/SKILL.md");
            return;
        }
        for skill in self.skills.list() {
            self.transcript.push_notice(format!(
                "{}: {}\n  {}",
                skill.name(),
                skill.description(),
                skill.location()
            ));
        }
        for warning in self.skills.warnings() {
            self.transcript.push_notice(format!("skipped: {warning}"));
        }
    }

    /// Move the transcript one wheel notch, away from the newest line or back
    /// towards it. While the whole transcript is open, the wheel moves it.
    pub fn wheel(&mut self, up: bool) {
        if let Some(overlay) = self.transcript_overlay.as_mut() {
            if up {
                overlay.scroll_up(WHEEL_LINES);
            } else {
                overlay.scroll_down(WHEEL_LINES);
            }
            return;
        }
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

    /// Open the status card, or close it when it is already open.
    ///
    /// The card is read-only: opening it sends nothing to the model and writes
    /// no transcript row.
    pub fn open_status(&mut self) {
        if matches!(self.modal, Some(Modal::Status(_))) {
            self.modal = None;
            return;
        }
        self.modal = Some(Modal::Status(StatusCard::new(self.status_fields())));
    }

    /// The status card's labelled lines, read from the state the screen holds.
    fn status_fields(&self) -> Vec<StatusField> {
        let spec = &self.status.model;
        let provider = spec
            .split_once(':')
            .map_or(spec.as_str(), |(provider, _)| provider);
        let mut fields = vec![
            StatusField::new("model", spec.clone()),
            StatusField::new("provider", provider),
            StatusField::new("mode", self.status.mode.label()),
            StatusField::new("session", self.session_label()),
            StatusField::new("session file", self.session_file_label()),
            StatusField::new("workspace", self.status.cwd.clone()),
            StatusField::new("tokens", self.tokens_label()),
            StatusField::new("skills", self.skills.list().len().to_string()),
        ];
        let mcp = self.mcp_tool_count();
        if mcp > 0 {
            fields.push(StatusField::new("mcp tools", mcp.to_string()));
        }
        fields.push(StatusField::new(
            "client",
            format!("niki {}", env!("CARGO_PKG_VERSION")),
        ));
        fields
    }

    fn session_label(&self) -> String {
        let Some(id) = &self.session else {
            return "not saved".to_owned();
        };
        if self.saved_turn {
            id.as_str().to_owned()
        } else {
            format!("{} new", id.as_str())
        }
    }

    fn session_file_label(&self) -> String {
        self.session_file
            .as_ref()
            .map_or_else(|| "unknown".to_owned(), |path| path.display().to_string())
    }

    fn tokens_label(&self) -> String {
        let Some(report) = self.status.context else {
            return "not measured yet".to_owned();
        };
        let used = format_tokens(u64::try_from(report.estimated_tokens).unwrap_or(u64::MAX));
        match self.status.context_window {
            Some(window) if window > 0 => {
                let limit = format_tokens(u64::try_from(window).unwrap_or(u64::MAX));
                match context_percentage(report.estimated_tokens, window) {
                    Some(percent) => format!("{used} / {limit} ({percent}%)"),
                    None => format!("{used} / {limit}"),
                }
            }
            _ => format!("{used} / unknown"),
        }
    }

    fn mcp_tool_count(&self) -> usize {
        self.tools
            .iter()
            .filter(|name| name.starts_with("mcp__"))
            .count()
    }

    /// Open the whole conversation, starting at its newest line.
    pub fn open_transcript(&mut self) {
        self.transcript_overlay = Some(TranscriptOverlay::new());
    }

    /// Open the model picker over the configured models.
    pub fn open_models(&mut self) {
        self.modal = Some(Modal::Models(self.models.clone()));
    }

    /// Open the session browser over the live sessions the store holds.
    ///
    /// The rows come from the same listing `/resume` prints, so choosing one
    /// takes the same path `/resume <id>` does. Archived sessions are left
    /// out; an archived id still opens through `/resume <id>`.
    pub fn open_threads(&mut self) {
        if self.refuse_while_busy("/threads") {
            return;
        }
        let Some(store) = &self.sessions else {
            self.transcript
                .push_notice("no sessions are saved in this build");
            return;
        };
        let summaries = match store.list_active() {
            Ok(summaries) => summaries,
            Err(error) => {
                self.transcript
                    .push_notice(format!("could not list the sessions: {error}"));
                return;
            }
        };
        if summaries.is_empty() {
            self.transcript.push_notice("no sessions are saved yet");
            return;
        }
        let entries = summaries.iter().map(thread_entry).collect();
        self.modal = Some(Modal::Threads(ThreadPicker::new(entries)));
    }

    fn modal_key(&mut self, key: KeyEvent) -> Option<Command> {
        let outcome = self.modal.as_mut()?.handle_key(key);
        match outcome {
            Outcome::Stay => None,
            Outcome::Close => {
                self.modal = None;
                self.pending = None;
                self.question = None;
                self.local_shell = None;
                None
            }
            Outcome::Answer(answer) => {
                self.modal = None;
                self.pending = None;
                self.question = None;
                if let Some(command) = self.local_shell.take() {
                    return self.local_answer(command, answer);
                }
                self.status.turn = TurnState::Working;
                Some(Command::Answer(answer))
            }
            Outcome::Question(reply) => {
                self.modal = None;
                self.question = None;
                self.status.turn = TurnState::Working;
                Some(Command::AnswerQuestion(reply))
            }
            Outcome::Model(spec) => {
                self.modal = None;
                Some(Command::SelectModel(spec))
            }
            Outcome::Connected(request) => {
                self.modal = None;
                self.apply_connect(request)
            }
            Outcome::Session(id) => {
                self.modal = None;
                self.resume(Some(&id));
                None
            }
        }
    }

    /// Drop the overlay and any answer it was waiting for, and the queue with
    /// it: a stopped turn is not going to be followed by what waited on it.
    fn cancel(&mut self) {
        self.modal = None;
        self.pending = None;
        self.question = None;
        self.local_shell = None;
        self.queued.clear();
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

    /// Show a structured question from `request_user_input` and wait for one
    /// answer.
    pub fn request_question(&mut self, request: AskRequest) {
        self.modal = Some(Modal::Question(QuestionModal::new(request.clone())));
        self.question = Some(request);
        self.status.turn = TurnState::Waiting;
    }

    /// Fold the end of a turn into the screen, then start the first queued
    /// prompt.
    ///
    /// Returns the prompt the host must hand to the worker, when one was
    /// queued and the turn ended normally. A cancelled turn discards its queue
    /// before it gets here, and a failed one leaves the queue on screen for the
    /// person to decide about.
    pub fn finish(&mut self, result: Result<RunOutcome, String>) -> Option<Command> {
        self.pending = None;
        self.question = None;
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
        self.persist_turn();
        if self.status.turn == TurnState::Idle {
            if let Some(prompt) = self.queued.pop_front() {
                return Some(self.begin_turn(prompt));
            }
        }
        None
    }

    /// Enter and its modifiers: send, queue, add a line, or steer.
    ///
    /// A turn in flight changes what the keys mean: Enter queues the draft for
    /// when the turn ends, and Alt+Enter or Ctrl+Enter hands it to the turn that
    /// is running. With nothing running they keep their composer meanings.
    fn enter(&mut self, modifiers: KeyModifiers) -> Option<Command> {
        if modifiers.intersects(KeyModifiers::ALT | KeyModifiers::CONTROL) {
            if self.status.turn.is_busy() {
                return self.steer();
            }
            self.composer.newline();
            self.history.reset();
            return None;
        }
        if modifiers.contains(KeyModifiers::SHIFT) {
            self.composer.newline();
            self.history.reset();
            return None;
        }
        self.submit()
    }

    /// Esc, Esc: the first keeps its meaning, the second restores the prompt
    /// the composer sent last.
    fn escape(&mut self) -> Option<Command> {
        if self.completion.is_some() {
            self.completion_closed = Some(self.composer.text().to_owned());
            self.completion = None;
            return None;
        }
        if !self.composer.is_empty() {
            self.composer.clear();
            self.history.reset();
            self.last_escape = None;
            return None;
        }
        let now = Instant::now();
        if self.status.turn == TurnState::Working {
            self.queued.clear();
            self.last_escape = Some(now);
            return Some(Command::Cancel);
        }
        if self
            .last_escape
            .is_some_and(|first| now.duration_since(first) < BACKTRACK_WINDOW)
        {
            self.last_escape = None;
            return self.backtrack();
        }
        self.last_escape = Some(now);
        None
    }

    /// Put the newest prompt back in the composer and drop what answered it.
    ///
    /// The view and the agent's history lose the prompt; the session store is
    /// left alone, because a turn that was already saved is still a turn that
    /// happened.
    fn backtrack(&mut self) -> Option<Command> {
        let Some(index) = self.transcript.last_user() else {
            self.transcript
                .push_notice("there is no earlier prompt to restore");
            return None;
        };
        let prompt = self.transcript.rows()[index].text.clone();
        self.transcript.truncate(index);
        self.turn_start = None;
        self.set_draft(&prompt);
        self.history.reset();
        self.scroll.reset();
        Some(Command::Rewind)
    }

    /// Run the command the open list highlights when the draft is still a
    /// fragment of it.
    ///
    /// Typing `/con` and pressing Enter runs `/connect`: the list already says
    /// which command the fragment means, so asking for a second keystroke would
    /// be bookkeeping. A draft with an argument is sent as typed, and an exact
    /// name is left alone.
    fn adopt_completion(&mut self, text: &str) -> String {
        if text.chars().any(char::is_whitespace) {
            return text.to_owned();
        }
        let Some(selected) = self
            .completion
            .as_ref()
            .map(|completion| completion.selected().name)
        else {
            return text.to_owned();
        };
        if selected == text {
            return text.to_owned();
        }
        self.set_draft(selected);
        self.completion = None;
        selected.to_owned()
    }

    fn submit(&mut self) -> Option<Command> {
        let text = self.composer.text().trim().to_owned();
        if text.is_empty() {
            return None;
        }
        if let Some(command) = text.strip_prefix('!') {
            return self.passthrough(command.trim().to_owned());
        }
        if text.starts_with('/') {
            self.history.reset();
            let text = self.adopt_completion(&text);
            return self.slash(&text);
        }
        if self.status.turn.is_busy() {
            return self.queue(text);
        }
        self.history.record(&text);
        Some(self.begin_turn(text))
    }

    /// Hold a prompt until the turn in flight ends.
    fn queue(&mut self, text: String) -> Option<Command> {
        self.composer.clear();
        self.history.record(&text);
        self.queued.push_back(text);
        self.scroll.reset();
        None
    }

    /// Hand the draft to the running turn, without starting another one.
    fn steer(&mut self) -> Option<Command> {
        let text = self.composer.text().trim().to_owned();
        if text.is_empty() {
            return None;
        }
        self.composer.clear();
        self.history.reset();
        self.transcript.push_user(text.clone());
        self.scroll.reset();
        Some(Command::Steer(text))
    }

    /// Run a `!` line locally under the same policy a shell call obeys.
    fn passthrough(&mut self, command: String) -> Option<Command> {
        if self.status.turn.is_busy() {
            self.transcript
                .push_notice("! waits until the turn finishes; press ctrl+c to stop it");
            return None;
        }
        self.composer.clear();
        self.history.reset();
        if command.is_empty() {
            return None;
        }
        match decide(self.status.mode, &self.allow, &command) {
            Decision::Allow => Some(self.start_shell(command)),
            Decision::Ask if self.session_allow.permits(&command) => {
                Some(self.start_shell(command))
            }
            Decision::Ask if self.approver => {
                self.local_shell = Some(command.clone());
                self.modal = Some(Modal::Approval(ApprovalModal::new(
                    ApprovalRequest::command(command, self.status.mode),
                )));
                None
            }
            Decision::Ask => {
                let message = format!(
                    "refused: `{command}` needs approval and this run has nobody to ask \
                     (mode: {}). Set the mode to auto, add the command to the allow-list, \
                     or run it yourself.",
                    self.status.mode.label()
                );
                self.refuse_shell(&command, &message);
                None
            }
            Decision::Deny => {
                self.refuse_shell(&command, &format!("refused by policy: `{command}`"));
                None
            }
        }
    }

    /// Record a shell line a person asked for, in the shape the model's calls
    /// leave, and ask the host to run it.
    fn start_shell(&mut self, command: String) -> Command {
        let arguments = shell_arguments(&command);
        self.transcript.push_tool_call("shell", &arguments);
        self.scroll.reset();
        Command::Shell(command)
    }

    /// Record a `!` line that never ran, the way a refused tool call reads.
    fn refuse_shell(&mut self, command: &str, message: &str) {
        let arguments = shell_arguments(command);
        self.transcript.push_tool_call("shell", &arguments);
        self.transcript.push_tool_result("shell", message, true);
        self.scroll.reset();
    }

    /// Answer a `!` command's own approval question.
    fn local_answer(&mut self, command: String, answer: Approval) -> Option<Command> {
        match answer {
            Approval::Once => Some(self.start_shell(command)),
            Approval::Always => {
                self.session_allow.add(&command);
                Some(self.start_shell(command))
            }
            Approval::Deny => {
                self.refuse_shell(&command, &format!("refused: `{command}` was not approved"));
                None
            }
        }
    }

    /// Record a prompt as the turn in flight and hand it to the host.
    fn begin_turn(&mut self, text: String) -> Command {
        self.composer.clear();
        self.last_escape = None;
        self.turn_start = Some(self.transcript.rows().len());
        self.transcript.push_user(text.clone());
        self.scroll.reset();
        self.status.turn = TurnState::Working;
        self.spinner = 0;
        Command::Submit(text)
    }

    /// Tab: put the highlighted command in the draft, ready for its argument.
    fn complete(&mut self) -> Option<Command> {
        let completion = self.completion.as_ref()?;
        let completed = completion.completed();
        self.composer.clear();
        self.composer.insert_str(&completed);
        self.history.reset();
        None
    }

    /// Up: the list takes it first, then the history, then the composer cursor.
    fn up(&mut self) {
        if let Some(completion) = self.completion.as_mut() {
            completion.up();
            return;
        }
        let browsing = self.history.is_browsing();
        let single_line = self.composer.line_count() == 1;
        if self.composer.is_empty() || (browsing && single_line) {
            let current = self.composer.text().to_owned();
            if let Some(prompt) = self.history.older(&current) {
                self.set_draft(&prompt);
            }
            return;
        }
        self.composer.up();
        self.history.reset();
    }

    /// Down: the list takes it first, then the history, then the cursor.
    fn down(&mut self) {
        if let Some(completion) = self.completion.as_mut() {
            completion.down();
            return;
        }
        let single_line = self.composer.line_count() == 1;
        if self.history.is_browsing() && single_line {
            if let Some(prompt) = self.history.newer() {
                self.set_draft(&prompt);
            }
            return;
        }
        self.composer.down();
        self.history.reset();
    }

    fn set_draft(&mut self, text: &str) {
        self.composer.clear();
        if !text.is_empty() {
            self.composer.insert_str(text);
        }
    }

    /// Take back what the editor left: the new draft, or the reason the old
    /// one stays.
    ///
    /// The text replaces the draft rather than joining it, and [`Self::set_draft`]
    /// leaves the cursor after the last character, so the person can keep
    /// typing or send it straight away.
    pub fn finish_edit(&mut self, result: Result<String, String>) {
        match result {
            Ok(text) => {
                self.set_draft(&text);
                self.last_escape = None;
                self.history.reset();
                self.refresh_completion();
            }
            Err(message) => self.transcript.push_notice(message),
        }
    }

    /// Recompute the command list from the draft, keeping a highlight the
    /// person moved on the command it is on while that one is still listed.
    ///
    /// A highlight on the first row is the list's default rather than a
    /// choice, so when the first row changes the narrowed list starts on its
    /// own first row instead of pinning a command that is merely still
    /// matched.
    fn refresh_completion(&mut self) {
        let draft = self.composer.text();
        if self.completion_closed.as_deref() == Some(draft) {
            self.completion = None;
            return;
        }
        self.completion_closed = None;
        let selected = self
            .completion
            .as_ref()
            .filter(|completion| completion.selected_index() > 0)
            .map(|completion| completion.selected().name);
        self.completion = Completion::of(draft);
        if let (Some(completion), Some(name)) = (self.completion.as_mut(), selected) {
            completion.reselect(name);
        }
    }

    /// Copy the newest answer to the terminal clipboard, or say why not.
    fn copy(&mut self) -> Option<Command> {
        let answer = self
            .transcript
            .rows()
            .iter()
            .rev()
            .find(|row| row.kind == RowKind::Assistant)
            .map(|row| row.text.clone());
        let Some(answer) = answer else {
            self.transcript.push_notice("no answer to copy yet");
            return None;
        };
        let characters = answer.chars().count();
        if !self.clipboard {
            self.transcript.push_notice(format!(
                "copy: {characters} characters (clipboard disabled)"
            ));
            return None;
        }
        self.transcript
            .push_notice(format!("copy: {characters} characters"));
        Some(Command::Copy(answer))
    }

    fn slash(&mut self, text: &str) -> Option<Command> {
        self.composer.clear();
        let command = match niki_commands::parse(text) {
            Ok(command) => command,
            Err(error) => {
                self.transcript.push_notice(error.to_string());
                return None;
            }
        };
        match command {
            SlashCommand::Mode(mode) => {
                self.status.mode = mode;
                self.transcript
                    .push_notice(format!("approval mode: {}", mode.label()));
                Some(Command::Mode(mode))
            }
            SlashCommand::Quit => {
                self.queued.clear();
                Some(Command::Quit)
            }
            SlashCommand::Help => {
                self.open_help();
                None
            }
            SlashCommand::Theme(name) => {
                self.theme(name.as_deref());
                None
            }
            SlashCommand::Model(Some(spec)) => Some(Command::SelectModel(spec)),
            SlashCommand::Model(None) => {
                self.open_models();
                None
            }
            SlashCommand::Skills => {
                self.show_skills();
                None
            }
            SlashCommand::Status => {
                self.open_status();
                None
            }
            SlashCommand::Init => {
                if self.refuse_while_busy("/init") {
                    return None;
                }
                match niki_config::instructions::write_scaffold(Path::new(&self.status.cwd)) {
                    Ok(outcome) => self.transcript.push_notice(outcome.message()),
                    Err(error) => self
                        .transcript
                        .push_error(format!("could not write AGENTS.md: {error}")),
                }
                self.refresh_system
                    .as_ref()
                    .map(|refresh| Command::RefreshSystem(refresh()))
            }
            SlashCommand::Clear => self.clear_session("/clear"),
            SlashCommand::New => self.clear_session("/new"),
            SlashCommand::Connect => {
                match &self.config {
                    Some(config) => {
                        self.modal = Some(Modal::Connect(ConnectModal::new(config)));
                    }
                    None => self.transcript.push_notice(
                        "connect needs a configuration file; start niki with --config",
                    ),
                }
                None
            }
            SlashCommand::Compact => {
                if self.refuse_while_busy("/compact") {
                    return None;
                }
                self.status.turn = TurnState::Working;
                Some(Command::Compact)
            }
            SlashCommand::Context => {
                if self.refuse_while_busy("/context") {
                    return None;
                }
                self.status.turn = TurnState::Working;
                Some(Command::Context)
            }
            SlashCommand::Diff(argument) => {
                self.show_diff(argument.as_deref());
                None
            }
            SlashCommand::Resume(target) => {
                self.resume(target.as_deref());
                None
            }
            SlashCommand::Threads => {
                self.open_threads();
                None
            }
            SlashCommand::Export(path) => {
                if self.refuse_while_busy("/export") {
                    return None;
                }
                let line = match (&self.sessions, &self.session) {
                    (Some(store), Some(session)) => match niki_session::export::write(
                        store,
                        session,
                        Path::new(&self.status.cwd),
                        path.as_deref(),
                    ) {
                        Ok(written) => format!("wrote {}", written.display()),
                        Err(error) => error.to_string(),
                    },
                    _ => "no session is saved in this build".to_owned(),
                };
                self.transcript.push_notice(line);
                None
            }
            SlashCommand::Rename(title) => {
                if self.refuse_while_busy("/rename") {
                    return None;
                }
                let line = match (&self.sessions, &self.session) {
                    (Some(store), Some(session)) => match title.as_deref() {
                        Some(raw) => match store.set_title(session, Some(raw)) {
                            Ok(Some(title)) => format!("renamed to {title}"),
                            Ok(None) => "title cleared".to_owned(),
                            Err(error) => format!("could not rename the session: {error}"),
                        },
                        None => match store.title(session) {
                            Ok(Some(title)) => title,
                            Ok(None) => "untitled".to_owned(),
                            Err(error) => format!("could not read the session title: {error}"),
                        },
                    },
                    _ => "no session is saved in this build".to_owned(),
                };
                self.transcript.push_notice(line);
                None
            }
            SlashCommand::Tools => {
                self.list_tools();
                None
            }
            SlashCommand::Version => {
                self.version();
                None
            }
        }
    }

    /// `/clear`: empty the view and start a session of its own.
    /// Store the provider's credential and default model, then switch to it.
    ///
    /// The key goes to `credentials.toml` beside the configuration; everything
    /// else is a line edit of `config.toml` itself. Both land before the screen
    /// reports success, so a failed write leaves the session unchanged.
    fn apply_connect(&mut self, request: ConnectRequest) -> Option<Command> {
        let (Some(config), Some(path)) = (&self.config, &self.config_path) else {
            self.transcript
                .push_error("connect needs a configuration file; start niki with --config");
            return None;
        };
        let name = if request.name == "custom" {
            unique_connect_name(config)
        } else {
            request.name.clone()
        };

        if let Some(key) = &request.key {
            let keys = niki_config::credentials::path_for(path);
            if let Err(error) = niki_config::credentials::set(&keys, &name, key) {
                self.transcript
                    .push_error(format!("could not store the key: {error}"));
                return None;
            }
        }

        let mut text = std::fs::read_to_string(path).unwrap_or_default();
        if let Some(base) = &request.base_url {
            let protocol = match request.protocol {
                Protocol::Anthropic => "anthropic",
                Protocol::OpenAi => "openai",
            };
            text = niki_config::edit::set_provider(&text, &name, base, protocol, request.keyless);
        }
        let spec = format!("{name}:{}", request.model);
        text = niki_config::edit::set_default_model(&text, &spec);

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(error) = std::fs::create_dir_all(parent) {
                    self.transcript
                        .push_error(format!("could not create {}: {error}", parent.display()));
                    return None;
                }
            }
        }
        if let Err(error) = std::fs::write(path, &text) {
            self.transcript
                .push_error(format!("could not write {}: {error}", path.display()));
            return None;
        }

        match Config::load(path) {
            Ok(reloaded) => {
                self.models = ModelPicker::from_config(&reloaded);
                self.config = Some(reloaded);
            }
            Err(error) => self.transcript.push_error(format!(
                "the configuration was written but does not read back: {error}"
            )),
        }
        self.status.model.clone_from(&spec);
        self.transcript
            .push_notice(format!("connected {name}; the default model is {spec}"));
        Some(Command::SelectModel(spec))
    }

    fn clear_session(&mut self, name: &str) -> Option<Command> {
        if self.refuse_while_busy(name) {
            return None;
        }
        self.transcript.clear();
        self.scroll.reset();
        self.turn_start = None;
        self.queued.clear();
        self.new_session();
        self.transcript.push_notice("started a fresh session");
        Some(Command::ClearApprovals)
    }

    /// Fold the view after the agent reported a real compaction.
    ///
    /// The view changes only when the agent replaced something. A provider
    /// failure leaves every row in place and prints an error line.
    pub fn compacted(&mut self, result: Result<Compaction, String>, fallback: bool) {
        self.status.turn = TurnState::Idle;
        match result {
            Ok(compaction) if compaction.replaced > 0 => {
                let dropped = self.transcript.fold(COMPACT_KEEP_ROWS);
                self.scroll.reset();
                let model = if fallback { " without a model" } else { "" };
                self.transcript.push_notice(format!(
                    "compacted {} messages{} into a {} token summary; folded {} earlier rows out of the view",
                    compaction.replaced, model, compaction.summary_tokens, dropped
                ));
            }
            Ok(_) => self.transcript.push_notice("nothing to compact"),
            Err(error) => self
                .transcript
                .push_error(format!("compaction failed: {error}")),
        }
    }

    /// Remember the agent's context report without printing it.
    pub fn set_context(&mut self, report: ContextReport, usage: SessionUsage) {
        self.status.set_context(report, usage);
    }

    /// Print the report `/context` asked for and remember it.
    pub fn show_context(&mut self, report: ContextReport, usage: SessionUsage) {
        self.status.set_context(report, usage);
        self.status.turn = TurnState::Idle;
        self.transcript.push_notice(describe_context(report, usage));
    }

    /// `/resume`: list live saved sessions, or rebuild one into the view.
    ///
    /// The list leaves archived sessions out, but a full id still opens one,
    /// because the person named it rather than picking it from the list.
    fn resume(&mut self, target: Option<&str>) {
        if self.refuse_while_busy("/resume") {
            return;
        }
        let Some(store) = &self.sessions else {
            self.transcript
                .push_notice("no sessions are saved in this build");
            return;
        };
        let summaries = match store.list_active() {
            Ok(summaries) => summaries,
            Err(error) => {
                self.transcript
                    .push_notice(format!("could not list the sessions: {error}"));
                return;
            }
        };
        let Some(target) = target else {
            self.list_sessions(&summaries);
            return;
        };
        let chosen = if let Ok(index) = target.parse::<usize>() {
            index
                .checked_sub(1)
                .and_then(|index| summaries.get(index))
                .map(|summary| summary.id.clone())
        } else if let Some(summary) = summaries
            .iter()
            .find(|summary| summary.id.as_str().starts_with(target))
        {
            Some(summary.id.clone())
        } else {
            match store.find(target) {
                Ok(summary) => summary.map(|summary| summary.id),
                Err(error) => {
                    self.transcript
                        .push_notice(format!("could not look up the session: {error}"));
                    return;
                }
            }
        };
        let Some(id) = chosen else {
            self.transcript.push_notice(format!(
                "there is no saved session {target}; /resume lists them"
            ));
            return;
        };
        match store.load(&id) {
            Ok(messages) => {
                self.transcript.clear();
                for message in &messages {
                    self.replay(&message.role, &message.content);
                }
                let count = messages.len();
                self.transcript.push_notice(format!(
                    "resumed {} ({})",
                    id.as_str(),
                    plural(count, "message")
                ));
                self.session = Some(id);
                self.saved_turn = count > 0;
                self.turn_start = None;
                self.scroll.reset();
                self.status.turn = TurnState::Idle;
            }
            Err(error) => self
                .transcript
                .push_notice(format!("could not resume {target}: {error}")),
        }
    }

    fn list_sessions(&mut self, summaries: &[Summary]) {
        if summaries.is_empty() {
            self.transcript.push_notice("no sessions are saved yet");
            return;
        }
        self.transcript
            .push_notice("recent sessions; /resume <index> or /resume <id> opens one".to_owned());
        for (index, summary) in summaries.iter().take(RESUME_LIMIT).enumerate() {
            self.transcript.push_notice(format!(
                "{}. {} ({}); {}",
                index + 1,
                summary.id,
                plural(
                    usize::try_from(summary.messages).unwrap_or(usize::MAX),
                    "message"
                ),
                summary.cwd
            ));
        }
        if summaries.len() > RESUME_LIMIT {
            self.transcript.push_notice(format!(
                "and {} more",
                plural(summaries.len() - RESUME_LIMIT, "session")
            ));
        }
    }

    /// `/diff`: the working tree's staged, unstaged and untracked changes.
    ///
    /// One row per file under its section heading, then what the cap kept back
    /// and the section summary. A clean tree is one row.
    fn show_diff(&mut self, argument: Option<&str>) {
        if self.refuse_while_busy("/diff") {
            return;
        }
        let sections = match argument {
            Some(argument) => match DiffSection::from_argument(argument) {
                Some(section) => vec![section],
                None => {
                    self.transcript.push_notice(format!(
                        "there is no {argument} section; try staged, unstaged, untracked"
                    ));
                    return;
                }
            },
            None => DiffSection::ALL.to_vec(),
        };
        let root = PathBuf::from(&self.status.cwd);
        let theme = diff_theme(self.palette);
        let views = match diff_source::collect(&root, &sections, diff_source::RENDER_WIDTH, &theme)
        {
            Ok(views) => views,
            Err(error) => {
                self.transcript.push_notice(error.to_string());
                return;
            }
        };
        if views.iter().all(|view| view.files.is_empty()) {
            self.transcript.push_notice("no changes");
            return;
        }
        for view in views {
            if view.files.is_empty() {
                continue;
            }
            self.transcript.push_notice(view.section.name());
            for file in view.files {
                match file.note {
                    Some(note) => self
                        .transcript
                        .push_notice(format!("{}: {note}", file.path)),
                    None => self.transcript.push_diff(&file.path, file.rendered),
                }
            }
            if let Some(withheld) =
                diff_source::withheld_line(view.withheld_files, view.withheld_lines)
            {
                self.transcript.push_notice(withheld);
            }
            self.transcript.push_notice(diff_source::summary_line(
                view.file_count,
                view.additions,
                view.deletions,
            ));
        }
    }

    /// `/tools`: every registered tool, MCP ones included, one row each.
    fn list_tools(&mut self) {
        if self.tools.is_empty() {
            self.transcript.push_notice("no tools are registered");
            return;
        }
        for name in &self.tools {
            self.transcript.push_notice(name.clone());
        }
    }

    /// `/version`: the line the person sees, in the product's name.
    fn version(&mut self) {
        self.transcript.push_notice(format!(
            "Niki Agent {} (Rust client)",
            env!("CARGO_PKG_VERSION")
        ));
    }

    /// Create the session row a fresh view writes into.
    ///
    /// A new session also forgets the approvals granted "for this session": the
    /// rules live only in memory and must not outlive the session they were
    /// given in. The configured shell allow-list is untouched.
    fn new_session(&mut self) {
        self.session = None;
        self.saved_turn = false;
        self.session_allow = ProgramAllow::default();
        let Some(store) = &self.sessions else {
            return;
        };
        match store.create(&self.status.cwd) {
            Ok(id) => self.session = Some(id),
            Err(error) => self
                .transcript
                .push_notice(format!("warning: this session will not be saved: {error}")),
        }
    }

    /// Refuse a command that would pull rows out from under a running turn.
    fn refuse_while_busy(&mut self, command: &str) -> bool {
        if !self.status.turn.is_busy() {
            return false;
        }
        self.transcript.push_notice(format!(
            "{command} waits until the turn finishes; press ctrl+c to stop it"
        ));
        true
    }

    /// Append the rows of the turn that just finished to the session store.
    ///
    /// Only the rows of this turn are written, and a failed or cancelled turn
    /// is kept as far as it got (RULES.md 6).
    fn persist_turn(&mut self) {
        let Some(start) = self.turn_start.take() else {
            return;
        };
        let (Some(store), Some(session)) = (&self.sessions, &self.session) else {
            return;
        };
        let mut failure = None;
        let mut saved = false;
        {
            let rows = self.transcript.rows();
            for row in rows.iter().skip(start) {
                let Some((role, content)) = persisted(row) else {
                    continue;
                };
                match store.append(session, role, &content) {
                    Ok(_) => saved = true,
                    Err(error) => {
                        failure = Some(error.to_string());
                        break;
                    }
                }
            }
        }
        if saved {
            self.saved_turn = true;
        }
        if let Some(error) = failure {
            self.transcript
                .push_notice(format!("warning: the session was not saved: {error}"));
        }
    }

    /// Rebuild one stored message into the view.
    fn replay(&mut self, role: &str, content: &str) {
        match role {
            "user" => self.transcript.push_user(content),
            "assistant" => {
                self.transcript.push_assistant(content);
                self.transcript.end_stream();
            }
            "tool_call" => {
                let (name, arguments) = split_record(content);
                self.transcript.push_tool_call(name, arguments);
            }
            "tool_result" => {
                let (name, output) = split_record(content);
                self.transcript.push_tool_result(name, output, false);
            }
            "tool_error" => {
                let (name, output) = split_record(content);
                self.transcript.push_tool_result(name, output, true);
            }
            "error" => self.transcript.push_error(content),
            other => self.transcript.push_notice(format!("{other}: {content}")),
        }
    }
}

/// How one transcript row is written to the store, if it is written at all.
///
/// Roles are `user`, `assistant`, `tool_call`, `tool_result`, `tool_error` and
/// `error`; a tool row's first line is its name and the rest is its arguments or
/// output, so a resumed transcript shows the same rows. Notices are view
/// furniture and stay out of the store.
fn persisted(row: &Row) -> Option<(&'static str, String)> {
    let record = match row.kind {
        RowKind::User => ("user", row.text.clone()),
        RowKind::Assistant => ("assistant", row.text.clone()),
        RowKind::ToolCall => ("tool_call", titled(row)),
        RowKind::ToolResult => ("tool_result", titled(row)),
        RowKind::Error if row.title.is_some() => ("tool_error", titled(row)),
        RowKind::Error => ("error", row.text.clone()),
        RowKind::Notice | RowKind::Diff => return None,
    };
    Some(record)
}

fn titled(row: &Row) -> String {
    match &row.title {
        Some(title) => format!("{title}\n{}", row.text),
        None => row.text.clone(),
    }
}

fn split_record(content: &str) -> (&str, &str) {
    content.split_once('\n').unwrap_or((content, ""))
}

/// One session browser row from a store summary.
fn thread_entry(summary: &Summary) -> ThreadEntry {
    ThreadEntry::new(
        summary.id.as_str(),
        summary.title.as_deref().or(summary.first_prompt.as_deref()),
        summary.messages,
        summary.cwd.as_str(),
    )
}

/// The argument string a shell call shows in its transcript row.
fn shell_arguments(command: &str) -> String {
    serde_json::json!({ "command": command }).to_string()
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

fn add_usage(total: &mut Usage, usage: Usage) {
    total.input_tokens += usage.input_tokens;
    total.output_tokens += usage.output_tokens;
}

fn is_cancelled(stop: &StopReason) -> bool {
    matches!(stop, StopReason::Other(reason) if reason == "cancelled")
}

/// A name for a custom provider that no table already uses.
fn unique_connect_name(config: &Config) -> String {
    if !config.providers.contains_key("custom") {
        return "custom".to_owned();
    }
    for suffix in 2..1000 {
        let candidate = format!("custom{suffix}");
        if !config.providers.contains_key(&candidate) {
            return candidate;
        }
    }
    "custom".to_owned()
}

fn clipboard_disabled() -> bool {
    matches!(std::env::var("NIKI_NO_CLIPBOARD").as_deref(), Ok("1"))
}

/// The diff colours the current palette paints with.
fn diff_theme(palette: &Palette) -> DiffTheme {
    let rgb = |color: niki_tui_themes::Rgb| niki_render::Rgb::new(color.r, color.g, color.b);
    DiffTheme::new(
        rgb(palette.ok),
        rgb(palette.error),
        rgb(palette.accent),
        rgb(palette.muted),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use niki_tools::{AskOption, AskRequest};

    fn app() -> App {
        App::new(SessionInfo {
            model: "mock:mock-model".to_owned(),
            mode: Mode::Manual,
            cwd: PathBuf::from("/work"),
        })
    }

    fn store(temp: &tempfile::TempDir) -> SessionStore {
        SessionStore::open(temp.path().join("sessions.db")).expect("session store")
    }

    fn outcome() -> RunOutcome {
        RunOutcome {
            text: String::new(),
            steps: 1,
            tool_calls: 0,
            stop: StopReason::EndTurn,
            usage: Usage::default(),
        }
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
    fn control_g_hands_the_draft_to_the_editor_without_starting_a_turn() {
        let mut app = app();
        let edit = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert_eq!(
            app.handle_key(edit),
            Some(Command::EditPrompt),
            "an empty composer can still be edited"
        );
        assert_eq!(app.status.turn, TurnState::Idle);
        assert!(app.transcript.is_empty(), "no turn is recorded");

        type_text(&mut app, "half a thought");
        assert_eq!(app.handle_key(edit), Some(Command::EditPrompt));
        assert_eq!(app.composer.text(), "half a thought");
        assert_eq!(app.status.turn, TurnState::Idle);
        assert!(app.transcript.is_empty(), "ctrl+g never starts a turn");
    }

    #[test]
    fn a_finished_edit_replaces_the_draft_and_leaves_the_cursor_at_the_end() {
        let mut app = app();
        type_text(&mut app, "replace me");
        app.finish_edit(Ok("a longer\nreplacement".to_owned()));
        assert_eq!(app.composer.text(), "a longer\nreplacement");
        assert_eq!(app.composer.row_col(), (1, 11));
        assert!(app.transcript.is_empty());
    }

    #[test]
    fn a_failed_edit_keeps_the_draft_and_says_why() {
        let mut app = app();
        type_text(&mut app, "half a thought");
        app.finish_edit(Err("editor exited with status 3".to_owned()));
        assert_eq!(app.composer.text(), "half a thought");
        let last = app.transcript.rows().last().expect("a warning");
        assert_eq!(last.kind, RowKind::Notice);
        assert_eq!(last.text, "editor exited with status 3");
    }

    #[test]
    fn alt_enter_starts_a_second_line_instead_of_sending() {
        let mut app = app();
        type_text(&mut app, "first");
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
        assert_eq!(app.handle_key(key), None);
        type_text(&mut app, "second");
        assert_eq!(app.composer.line_count(), 2);
        assert_eq!(app.composer_height(), 3);
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
    fn the_mode_commands_set_the_status_line_and_reach_the_host() {
        let mut app = app();
        for (command, mode) in [
            ("/auto", Mode::Auto),
            ("/yolo", Mode::Yolo),
            ("/manual", Mode::Manual),
        ] {
            type_text(&mut app, command);
            assert_eq!(
                press(&mut app, KeyCode::Enter),
                Some(Command::Mode(mode)),
                "{command}"
            );
            assert_eq!(app.status.mode, mode, "{command}");
            assert!(
                app.status_line().contains(mode.label()),
                "{command}: {}",
                app.status_line()
            );
            assert!(app.composer.is_empty());
            assert_eq!(app.status.turn, TurnState::Idle, "a mode is not a turn");
        }
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
        assert!(
            app.accounting_line()
                .is_some_and(|line| line.contains("21 tok")),
            "{:?}",
            app.accounting_line()
        );

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
        assert_eq!(app.finish(Ok(outcome.clone())), None);
        assert_eq!(app.status.turn, TurnState::Cancelled);

        outcome.stop = StopReason::EndTurn;
        assert_eq!(app.finish(Ok(outcome)), None);
        assert_eq!(app.status.turn, TurnState::Idle);
    }

    #[test]
    fn a_failed_turn_says_so_in_the_transcript() {
        let mut app = app();
        app.status.turn = TurnState::Working;
        assert_eq!(
            app.finish(Err("provider mock failed: 503".to_owned())),
            None
        );
        assert_eq!(app.status.turn, TurnState::Failed);
        let last = app.transcript.rows().last().expect("an error");
        assert_eq!(last.kind, RowKind::Error);
        assert!(last.text.contains("503"), "{}", last.text);
    }

    #[test]
    fn an_approval_request_waits_for_the_person() {
        let mut app = app();
        app.request_approval(ApprovalRequest::command("rm -rf build", Mode::Manual));
        assert_eq!(app.status.turn, TurnState::Waiting);
        assert_eq!(
            app.pending.as_ref().map(|request| request.command.as_str()),
            Some("rm -rf build")
        );
        assert!(app.status_line().contains("waiting for you"));
    }

    #[test]
    fn an_approval_request_opens_a_prompt_that_y_and_a_answer_once() {
        let mut app = app();
        app.request_approval(ApprovalRequest::command("cargo publish", Mode::Manual));
        assert!(matches!(app.modal, Some(Modal::Approval(_))));
        let command = press(&mut app, KeyCode::Char('y'));
        assert_eq!(command, Some(Command::Answer(Approval::Once)));
        assert!(app.modal.is_none());
        assert!(app.pending.is_none());
        assert_eq!(app.status.turn, TurnState::Working);

        app.request_approval(ApprovalRequest::command("cargo publish", Mode::Manual));
        assert_eq!(
            press(&mut app, KeyCode::Char('a')),
            Some(Command::Answer(Approval::Once)),
            "a is the other allow-once key"
        );
    }

    #[test]
    fn s_allows_for_the_session() {
        let mut app = app();
        app.request_approval(ApprovalRequest::command("cargo publish", Mode::Manual));
        assert_eq!(
            press(&mut app, KeyCode::Char('s')),
            Some(Command::Answer(Approval::Always))
        );
        assert!(app.modal.is_none());
        assert_eq!(app.status.turn, TurnState::Working);
    }

    #[test]
    fn an_approval_prompt_can_be_refused_with_d_n_or_escape() {
        for code in [KeyCode::Char('d'), KeyCode::Char('n'), KeyCode::Esc] {
            let mut app = app();
            app.request_approval(ApprovalRequest::command("rm -rf build", Mode::Auto));
            assert_eq!(
                press(&mut app, code),
                Some(Command::Answer(Approval::Deny)),
                "{code:?}"
            );
            assert!(app.modal.is_none());
        }
    }

    #[test]
    fn arrows_and_tab_move_the_highlight_and_enter_takes_it() {
        let mut app = app();
        app.request_approval(ApprovalRequest::command("cargo publish", Mode::Manual));
        assert_eq!(press(&mut app, KeyCode::Down), None);
        assert_eq!(press(&mut app, KeyCode::Tab), None);
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::Answer(Approval::Deny))
        );

        let mut app = self::app();
        app.request_approval(ApprovalRequest::command("cargo publish", Mode::Manual));
        assert_eq!(press(&mut app, KeyCode::Down), None);
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::Answer(Approval::Always))
        );
    }

    fn question(allow_freeform: bool) -> AskRequest {
        AskRequest {
            question: "Which store?".to_owned(),
            options: vec![
                AskOption {
                    label: "SQLite".to_owned(),
                    description: None,
                },
                AskOption {
                    label: "JSON".to_owned(),
                    description: None,
                },
            ],
            allow_freeform,
        }
    }

    #[test]
    fn a_question_opens_a_prompt_and_down_enter_answers_the_second_option() {
        let mut app = app();
        app.request_question(question(false));
        assert!(matches!(app.modal, Some(Modal::Question(_))));
        assert_eq!(app.status.turn, TurnState::Waiting);
        assert!(app.status_line().contains("waiting for you"));

        assert_eq!(press(&mut app, KeyCode::Down), None);
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::AnswerQuestion(QuestionReply::Choice(
                "JSON".to_owned()
            )))
        );
        assert!(app.modal.is_none());
        assert!(app.question.is_none());
        assert_eq!(app.status.turn, TurnState::Working);
    }

    #[test]
    fn escape_cancels_a_question() {
        let mut app = app();
        app.request_question(question(false));
        assert_eq!(
            press(&mut app, KeyCode::Esc),
            Some(Command::AnswerQuestion(QuestionReply::Cancelled))
        );
        assert!(app.modal.is_none());
        assert!(app.question.is_none());
    }

    #[test]
    fn q_is_typed_into_the_free_text_row_instead_of_closing_it() {
        let mut app = app();
        app.request_question(question(true));
        assert_eq!(press(&mut app, KeyCode::Char('q')), None);
        assert!(
            matches!(app.modal, Some(Modal::Question(_))),
            "q must not close the prompt"
        );
        let Some(Modal::Question(modal)) = &app.modal else {
            unreachable!("the prompt is open")
        };
        assert_eq!(modal.freeform(), "q");
        assert!(modal.is_freeform_selected());
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::AnswerQuestion(QuestionReply::Freeform(
                "q".to_owned()
            )))
        );
    }

    #[test]
    fn q_is_ignored_when_the_question_refuses_free_text() {
        let mut app = app();
        app.request_question(question(false));
        assert_eq!(press(&mut app, KeyCode::Char('q')), None);
        assert!(matches!(app.modal, Some(Modal::Question(_))));
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::AnswerQuestion(QuestionReply::Choice(
                "SQLite".to_owned()
            )))
        );
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
    fn the_skills_command_lists_name_description_and_path() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let directory = project.path().join(".niki/skills/deploy");
        std::fs::create_dir_all(&directory).expect("skill directory");
        std::fs::write(
            directory.join("SKILL.md"),
            "---\nname: deploy\ndescription: Ship a release safely.\n---\nbody\n",
        )
        .expect("skill");

        let mut app = app();
        app.set_skills(SkillCatalog::discover(project.path(), user.path()));
        type_text(&mut app, "/skills");
        assert_eq!(press(&mut app, KeyCode::Enter), None);

        let texts: Vec<&str> = app
            .transcript
            .rows()
            .iter()
            .map(|row| row.text.as_str())
            .collect();
        assert!(
            texts.iter().any(|text| text.contains("deploy")
                && text.contains("Ship a release safely.")
                && text.contains("deploy/SKILL.md")),
            "{texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text.starts_with("commit:") && text.contains("built-in")),
            "the built-in skills are listed too: {texts:?}"
        );
        assert!(app.composer.is_empty());
        assert_eq!(app.status.turn, TurnState::Idle, "a command is not a turn");
    }

    #[test]
    fn init_writes_the_scaffold_once_and_refreshes_the_system_prompt() {
        let project = tempfile::tempdir().expect("project");
        let mut app = App::new(SessionInfo {
            model: "mock:mock-model".to_owned(),
            mode: Mode::Manual,
            cwd: project.path().to_path_buf(),
        });
        app.set_system_refresh(|| "fresh system prompt".to_owned());

        type_text(&mut app, "/init");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::RefreshSystem("fresh system prompt".to_owned()))
        );
        let path = project.path().join("AGENTS.md");
        assert_eq!(
            std::fs::read_to_string(&path).expect("scaffold"),
            niki_config::instructions::SCAFFOLD
        );
        let first = app.transcript.rows().last().expect("a notice").text.clone();
        assert!(first.contains("wrote"), "{first}");

        std::fs::write(&path, "hand written").expect("edit");
        type_text(&mut app, "/init");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::RefreshSystem("fresh system prompt".to_owned()))
        );
        let second = app.transcript.rows().last().expect("a notice").text.clone();
        assert!(second.contains("already exists"), "{second}");
        assert_eq!(
            std::fs::read_to_string(&path).expect("kept"),
            "hand written"
        );
    }

    #[test]
    fn the_skills_command_reports_a_skill_it_could_not_load() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let directory = project.path().join(".niki/skills/broken");
        std::fs::create_dir_all(&directory).expect("skill directory");
        std::fs::write(directory.join("SKILL.md"), "# no front matter\n").expect("skill");

        let mut app = app();
        app.set_skills(SkillCatalog::discover(project.path(), user.path()));
        type_text(&mut app, "/skills");
        assert_eq!(press(&mut app, KeyCode::Enter), None);

        assert!(
            app.transcript
                .rows()
                .iter()
                .any(|row| row.text.starts_with("skipped: ") && row.text.contains("broken")),
            "{:?}",
            app.transcript.rows()
        );
    }

    #[test]
    fn skills_without_a_catalog_say_where_to_add_one() {
        let mut app = app();
        type_text(&mut app, "/skills");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        let last = app.transcript.rows().last().expect("a notice");
        assert_eq!(last.kind, RowKind::Notice);
        assert!(last.text.contains(".niki/skills"), "{}", last.text);
    }

    #[test]
    fn cancelling_closes_a_prompt_without_leaving_the_turn_waiting() {
        let mut app = app();
        app.request_approval(ApprovalRequest::command("cargo publish", Mode::Manual));
        let cancel = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(cancel), Some(Command::Cancel));
        assert!(app.modal.is_none());
        assert!(app.pending.is_none());
        assert_eq!(app.status.turn, TurnState::Working);
    }

    #[test]
    fn the_spinner_animates_only_when_it_may() {
        let mut app = app();
        // A waiting turn keeps the state word and the spinner in the footer; a
        // working one hands both to the live row above the composer.
        app.status.turn = TurnState::Waiting;
        let first = app.spinner().expect("a frame while working");
        assert!(app.status_line().contains(first));
        app.tick();
        let second = app.spinner().expect("a frame while working");
        assert_ne!(first, second);

        app.set_motion(Motion::Reduced);
        assert_eq!(
            app.spinner(),
            Some('◌'),
            "reduced motion keeps a static marker"
        );
        assert!(app.status_line().contains('◌'));
        assert!(!app.status_line().contains('⠋'));

        app.set_motion(Motion::Full);
        app.set_glyphs(GlyphMode::Ascii);
        let frame = app.spinner().expect("a frame while working");
        assert!(frame.is_ascii(), "{frame}");
        app.set_motion(Motion::Reduced);
        assert_eq!(app.spinner(), Some('o'), "the ASCII resting glyph");
    }

    #[test]
    fn the_progress_effect_can_switch_the_spinner_off() {
        let mut app = app();
        app.status.turn = TurnState::Waiting;
        app.set_effects(Effects {
            progress: false,
            ..Effects::ALL
        });
        assert_eq!(app.spinner(), None);
        assert!(app.status_line().contains("waiting"), "the state remains");
    }

    #[test]
    fn the_activity_marker_blinks_and_holds_still_without_motion() {
        let mut app = app();
        let first = app.activity_glyph();
        assert_eq!(first, '•');
        for _ in 0..motion::ACTIVITY_TICKS {
            app.tick();
        }
        assert_eq!(app.activity_glyph(), '◦', "the bullet dims after 600 ms");

        app.set_motion(Motion::Reduced);
        assert_eq!(app.activity_glyph(), '•', "reduced motion holds it lit");

        app.set_motion(Motion::Full);
        app.set_effects(Effects {
            shimmer: false,
            ..Effects::ALL
        });
        assert_eq!(
            app.activity_glyph(),
            '•',
            "the effect off is still a marker"
        );

        app.set_effects(Effects::ALL);
        app.set_glyphs(GlyphMode::Ascii);
        app.spinner = 0;
        assert_eq!(app.activity_glyph(), '*');
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
    fn the_hints_drop_whole_keys_to_fit_the_width() {
        let mut app = app();
        let wide = app.hints(120);
        for hint in ["/ commands", "? keys", "ctrl+t transcript", "ctrl+d quit"] {
            assert!(wide.contains(hint), "{hint}: {wide}");
        }
        let narrow = app.hints(12);
        assert!(
            niki_render::display_width(&narrow) <= 12,
            "{narrow:?} is wider than the terminal"
        );
        assert!(narrow.contains('/'), "{narrow:?}");
        assert!(!narrow.contains("ctrl+d"), "{narrow:?}");
        assert_eq!(app.hints(0), "");

        app.set_glyphs(GlyphMode::Ascii);
        assert!(app.hints(120).is_ascii());
    }

    #[test]
    fn the_footer_starts_with_keys_then_shows_the_accounting() {
        let mut app = app();
        assert!(!app.started());
        assert!(app.accounting_line().is_none());

        type_text(&mut app, "hi");
        press(&mut app, KeyCode::Enter);
        assert!(app.started(), "sending a prompt begins the session");

        app.apply(AgentEvent::Usage(Usage {
            input_tokens: 4,
            output_tokens: 2,
        }));
        assert_eq!(app.accounting_line().as_deref(), Some("6 tok"));
    }

    #[test]
    fn a_notice_does_not_start_the_session() {
        let mut app = app();
        app.apply(AgentEvent::Notice("warning: not trusted".to_owned()));
        assert!(!app.started());
    }

    #[test]
    fn the_title_names_the_project_and_the_state() {
        let mut app = App::new(SessionInfo {
            model: "mock:mock-model".to_owned(),
            mode: Mode::Manual,
            cwd: PathBuf::from("/work/demo"),
        });
        assert_eq!(app.title(), "demo — idle");
        app.status.turn = TurnState::Working;
        assert_eq!(app.title(), "demo — working");

        app.status.turn = TurnState::Waiting;
        assert_eq!(app.title(), "● demo — needs you");
        for _ in 0..5 {
            app.tick();
        }
        assert_eq!(app.title(), "○ demo — needs you", "the marker blinks");

        app.set_motion(Motion::Reduced);
        assert_eq!(
            app.title(),
            "● demo — needs you",
            "reduced motion holds the marker lit"
        );
        app.set_glyphs(GlyphMode::Ascii);
        assert_eq!(app.title(), "* demo - needs you");
    }

    #[test]
    fn the_title_strips_control_characters_from_the_path() {
        let app = App::new(SessionInfo {
            model: "mock:mock-model".to_owned(),
            mode: Mode::Manual,
            cwd: PathBuf::from("/work/bad\u{1b}]0;pwned\u{7}"),
        });
        assert!(!app.title().contains('\u{1b}'), "{:?}", app.title());
        assert!(!app.title().contains('\u{7}'), "{:?}", app.title());
    }

    #[test]
    fn the_composer_grows_with_its_text_up_to_a_limit() {
        let mut app = app();
        assert_eq!(app.composer_height(), 2);
        for line in 0..10 {
            if line > 0 {
                let _ = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT));
            }
            type_text(&mut app, "x");
        }
        assert_eq!(app.composer_height(), MAX_COMPOSER_LINES + 1);
    }

    #[test]
    fn marks_follow_the_glyph_mode() {
        let mut app = app();
        assert_eq!(app.marks().user, "›");
        app.set_glyphs(GlyphMode::Ascii);
        assert_eq!(app.marks().user, ">");
        assert_eq!(app.palette.name, "niki");
    }

    #[test]
    fn clear_starts_a_new_session_without_dropping_the_store() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = app();
        app.attach_sessions(store(&temp));
        let first = app.session_id().expect("first session").to_owned();
        let id = app.sessions.as_ref().expect("store").list().expect("list")[0]
            .id
            .clone();
        app.sessions
            .as_ref()
            .expect("store")
            .append(&id, "user", "keep me")
            .expect("append");

        type_text(&mut app, "/clear");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::ClearApprovals)
        );

        let second = app.session_id().expect("second session").to_owned();
        assert_ne!(first, second, "a new session id");
        assert_eq!(app.transcript.rows().len(), 1, "the view is empty again");
        assert_eq!(app.transcript.rows()[0].kind, RowKind::Notice);
        assert!(
            app.transcript.rows()[0].text.contains("fresh session"),
            "{}",
            app.transcript.rows()[0].text
        );
        let store = app.sessions.as_ref().expect("store");
        assert_eq!(store.list().expect("list").len(), 2, "the store keeps both");
        let kept = store.load(&id).expect("load");
        assert_eq!(kept.len(), 1, "clearing the view deletes nothing");
        assert_eq!(kept[0].content, "keep me");
    }

    #[test]
    fn compact_folds_older_rows_after_the_agent_reports_a_compaction() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = app();
        app.attach_sessions(store(&temp));
        let id = app.sessions.as_ref().expect("store").list().expect("list")[0]
            .id
            .clone();
        app.sessions
            .as_ref()
            .expect("store")
            .append(&id, "user", "stored")
            .expect("append");
        for index in 0..(COMPACT_KEEP_ROWS + 8) {
            app.transcript.push_user(format!("row {index}"));
        }

        app.compacted(
            Ok(Compaction {
                replaced: 8,
                summary_tokens: 42,
            }),
            false,
        );

        let rows = app.transcript.rows();
        assert_eq!(rows.len(), COMPACT_KEEP_ROWS + 1, "kept rows and a notice");
        assert_eq!(rows[0].text, "row 8", "the oldest rows left the view");
        let last = rows.last().expect("notice");
        assert_eq!(last.kind, RowKind::Notice);
        assert!(last.text.contains("compacted 8 messages"), "{}", last.text);
        assert!(last.text.contains("42 token summary"), "{}", last.text);
        assert_eq!(
            app.sessions
                .as_ref()
                .expect("store")
                .load(&id)
                .expect("load")
                .len(),
            1,
            "compaction never writes or deletes stored messages"
        );
    }

    #[test]
    fn a_compaction_that_replaced_nothing_leaves_the_view_untouched() {
        let mut app = app();
        for index in 0..(COMPACT_KEEP_ROWS + 8) {
            app.transcript.push_user(format!("row {index}"));
        }
        let before: Vec<String> = app
            .transcript
            .rows()
            .iter()
            .map(|row| row.text.clone())
            .collect();

        app.compacted(Ok(Compaction::default()), false);

        let after: Vec<String> = app
            .transcript
            .rows()
            .iter()
            .map(|row| row.text.clone())
            .collect();
        assert_eq!(after[..before.len()], before[..], "no row moved");
        let last = app.transcript.rows().last().expect("notice");
        assert_eq!(last.kind, RowKind::Notice);
        assert!(last.text.contains("nothing to compact"), "{}", last.text);
    }

    #[test]
    fn a_provider_error_keeps_the_view_and_shows_the_error() {
        let mut app = app();
        for index in 0..(COMPACT_KEEP_ROWS + 8) {
            app.transcript.push_user(format!("row {index}"));
        }
        let before = app.transcript.rows().len();

        app.compacted(Err("provider scripted failed: 503".to_owned()), false);

        assert_eq!(app.transcript.rows().len(), before + 1);
        let last = app.transcript.rows().last().expect("error");
        assert_eq!(last.kind, RowKind::Error);
        assert!(last.text.contains("503"), "{}", last.text);
    }

    #[test]
    fn a_fallback_compaction_says_it_had_no_model() {
        let mut app = app();
        for index in 0..(COMPACT_KEEP_ROWS + 2) {
            app.transcript.push_user(format!("row {index}"));
        }
        app.compacted(
            Ok(Compaction {
                replaced: 4,
                summary_tokens: 9,
            }),
            true,
        );
        let last = app.transcript.rows().last().expect("notice");
        assert!(
            last.text.contains("compacted 4 messages without a model"),
            "{}",
            last.text
        );
        assert!(last.text.contains("9 token summary"), "{}", last.text);
    }

    #[test]
    fn the_context_command_asks_for_a_report_and_prints_it() {
        let mut app = app();
        type_text(&mut app, "/context");
        assert_eq!(press(&mut app, KeyCode::Enter), Some(Command::Context));
        assert_eq!(app.status.turn, TurnState::Working);

        app.show_context(
            ContextReport {
                messages: 3,
                estimated_tokens: 1_234,
                last_input_tokens: 900,
                last_output_tokens: 120,
                steps: 2,
            },
            SessionUsage {
                input_tokens: 1_000,
                output_tokens: 234,
                steps: 2,
            },
        );

        assert_eq!(app.status.turn, TurnState::Idle);
        let last = app.transcript.rows().last().expect("notice");
        assert_eq!(last.kind, RowKind::Notice);
        for expected in [
            "3 messages",
            "1,234 estimated tokens",
            "2 steps",
            "1,234 session tokens",
        ] {
            assert!(last.text.contains(expected), "{expected}: {}", last.text);
        }
        let accounting = app.accounting_line().expect("accounting after /context");
        assert!(accounting.contains("ctx 1,234 tok"), "{accounting}");
        assert!(accounting.contains("1,234 tok"), "{accounting}");
    }

    #[test]
    fn resume_lists_sessions_and_rebuilds_the_selected_one() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut earlier = app();
        earlier.attach_sessions(store(&temp));
        let id = earlier.session_id().expect("session").to_owned();

        type_text(&mut earlier, "hello");
        assert_eq!(
            press(&mut earlier, KeyCode::Enter),
            Some(Command::Submit("hello".to_owned()))
        );
        earlier.apply(AgentEvent::Text("hi there".to_owned()));
        assert_eq!(earlier.finish(Ok(outcome())), None);

        type_text(&mut earlier, "/resume");
        assert_eq!(press(&mut earlier, KeyCode::Enter), None);
        assert!(
            earlier
                .transcript
                .rows()
                .iter()
                .any(|row| row.kind == RowKind::Notice && row.text.contains(&id)),
            "the listing names the session"
        );

        let mut next = app();
        next.attach_sessions(store(&temp));
        assert_ne!(
            next.session_id(),
            Some(id.as_str()),
            "a fresh launch starts its own session"
        );
        type_text(&mut next, "/resume 2");
        assert_eq!(press(&mut next, KeyCode::Enter), None);

        assert_eq!(next.session_id(), Some(id.as_str()));
        let rows = next.transcript.rows();
        assert!(
            rows.iter()
                .any(|row| row.kind == RowKind::User && row.text == "hello"),
            "{rows:?}"
        );
        assert!(
            rows.iter()
                .any(|row| row.kind == RowKind::Assistant && row.text == "hi there"),
            "{rows:?}"
        );
        assert!(
            rows.last().expect("notice").text.contains("resumed"),
            "{}",
            rows.last().expect("notice").text
        );
    }

    #[test]
    fn resume_names_a_session_it_cannot_find() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = app();
        app.attach_sessions(store(&temp));
        let current = app.session_id().expect("session").to_owned();

        type_text(&mut app, "/resume 9");
        assert_eq!(press(&mut app, KeyCode::Enter), None);

        assert_eq!(app.session_id(), Some(current.as_str()));
        let last = app.transcript.rows().last().expect("notice");
        assert!(last.text.contains("no saved session 9"), "{}", last.text);
    }

    #[test]
    fn threads_browses_filters_and_resumes_the_chosen_session() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut earlier = app();
        earlier.attach_sessions(store(&temp));

        let deploy = earlier.session_id().expect("session").to_owned();
        type_text(&mut earlier, "deploy the site");
        assert_eq!(
            press(&mut earlier, KeyCode::Enter),
            Some(Command::Submit("deploy the site".to_owned()))
        );
        earlier.apply(AgentEvent::Text("done".to_owned()));
        assert_eq!(earlier.finish(Ok(outcome())), None);

        type_text(&mut earlier, "/clear");
        assert_eq!(
            press(&mut earlier, KeyCode::Enter),
            Some(Command::ClearApprovals)
        );
        let parser = earlier.session_id().expect("second session").to_owned();
        type_text(&mut earlier, "fix the parser");
        assert_eq!(
            press(&mut earlier, KeyCode::Enter),
            Some(Command::Submit("fix the parser".to_owned()))
        );
        earlier.apply(AgentEvent::Text("patched".to_owned()));
        assert_eq!(earlier.finish(Ok(outcome())), None);

        let mut next = app();
        next.attach_sessions(store(&temp));
        let own = next.session_id().expect("own session").to_owned();

        type_text(&mut next, "/threads");
        assert_eq!(press(&mut next, KeyCode::Enter), None);
        let entries = match &next.modal {
            Some(Modal::Threads(picker)) => picker.entries().to_vec(),
            other => panic!("expected the session browser, got {other:?}"),
        };
        assert_eq!(entries.len(), 3, "the own session and the two earlier ones");
        let row = entries
            .iter()
            .find(|entry| entry.id == deploy)
            .expect("the deploy session");
        assert_eq!(row.preview, "deploy the site");
        assert_eq!(row.message_label(), "2 messages");
        assert_eq!(row.cwd, "/work");
        assert!(
            entries
                .iter()
                .any(|entry| entry.id == own && entry.preview == niki_tui_modals::NO_PROMPT),
            "a session with no prompt shows the placeholder: {entries:?}"
        );
        assert!(entries.iter().any(|entry| entry.id == parser));

        assert_eq!(press(&mut next, KeyCode::Esc), None);
        assert!(next.modal.is_none(), "escape closes without choosing");
        assert_eq!(next.session_id(), Some(own.as_str()));

        type_text(&mut next, "/threads");
        assert_eq!(press(&mut next, KeyCode::Enter), None);
        type_text(&mut next, "DePlOy");
        assert_eq!(press(&mut next, KeyCode::Enter), None);

        assert!(next.modal.is_none());
        assert_eq!(next.session_id(), Some(deploy.as_str()));
        let rows = next.transcript.rows();
        assert!(
            rows.iter()
                .any(|row| row.kind == RowKind::User && row.text == "deploy the site"),
            "{rows:?}"
        );
        assert!(
            rows.iter()
                .any(|row| row.kind == RowKind::Assistant && row.text == "done"),
            "{rows:?}"
        );
    }

    #[test]
    fn archived_sessions_are_hidden_from_the_pickers_but_resume_by_id_still_works() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = app();
        app.attach_sessions(store(&temp));
        let own = app.session_id().expect("session").to_owned();

        let archived = {
            let store = store(&temp);
            let archived = store.create("/work").expect("session");
            store.append(&archived, "user", "old news").expect("append");
            store.set_archived(&archived, true).expect("archive");
            archived
        };

        type_text(&mut app, "/threads");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        let entries = match &app.modal {
            Some(Modal::Threads(picker)) => picker.entries().to_vec(),
            other => panic!("expected the session browser, got {other:?}"),
        };
        assert_eq!(
            entries.len(),
            1,
            "only the live session is offered: {entries:?}"
        );
        assert!(entries.iter().any(|entry| entry.id == own));
        assert!(!entries.iter().any(|entry| entry.id == archived.as_str()));
        assert_eq!(press(&mut app, KeyCode::Esc), None);

        type_text(&mut app, "/resume");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        let notices: Vec<&str> = app
            .transcript
            .rows()
            .iter()
            .filter(|row| row.kind == RowKind::Notice)
            .map(|row| row.text.as_str())
            .collect();
        assert!(
            notices.iter().any(|text| text.contains(&own)),
            "{notices:?}"
        );
        assert!(
            !notices.iter().any(|text| text.contains(archived.as_str())),
            "{notices:?}"
        );

        type_text(&mut app, &format!("/resume {archived}"));
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert_eq!(app.session_id(), Some(archived.as_str()));
        assert!(
            app.transcript
                .rows()
                .iter()
                .any(|row| row.kind == RowKind::User && row.text == "old news"),
            "{:?}",
            app.transcript.rows()
        );
    }

    #[test]
    fn tools_lists_every_registered_name() {
        let mut app = app();
        app.set_tools(vec![
            "read_file".to_owned(),
            "shell".to_owned(),
            "mcp__fixture__echo".to_owned(),
        ]);
        type_text(&mut app, "/tools");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        let listed: Vec<&str> = app
            .transcript
            .rows()
            .iter()
            .map(|row| row.text.as_str())
            .collect();
        assert_eq!(listed, ["read_file", "shell", "mcp__fixture__echo"]);
        assert!(
            app.transcript
                .rows()
                .iter()
                .all(|row| row.kind == RowKind::Notice)
        );
    }

    #[test]
    fn version_names_the_product() {
        let mut app = app();
        type_text(&mut app, "/version");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        let last = app.transcript.rows().last().expect("notice");
        assert_eq!(last.kind, RowKind::Notice);
        assert!(last.text.starts_with("Niki Agent "), "{}", last.text);
        assert!(
            last.text.contains(env!("CARGO_PKG_VERSION")),
            "{}",
            last.text
        );
    }

    #[test]
    fn a_finished_turn_is_appended_to_the_session_store() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = app();
        app.attach_sessions(store(&temp));
        let id = app.sessions.as_ref().expect("store").list().expect("list")[0]
            .id
            .clone();

        type_text(&mut app, "list the files");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::Submit("list the files".to_owned()))
        );
        app.apply(AgentEvent::Text("Looking.".to_owned()));
        app.apply(AgentEvent::ToolStart {
            name: "shell".to_owned(),
            arguments: "{\"command\":\"ls\"}".to_owned(),
        });
        app.apply(AgentEvent::ToolEnd {
            name: "shell".to_owned(),
            output: "a.txt".to_owned(),
            is_error: false,
        });
        assert_eq!(app.finish(Ok(outcome())), None);

        let messages = app
            .sessions
            .as_ref()
            .expect("store")
            .load(&id)
            .expect("load");
        let records: Vec<(&str, &str)> = messages
            .iter()
            .map(|message| (message.role.as_str(), message.content.as_str()))
            .collect();
        assert_eq!(
            records,
            [
                ("user", "list the files"),
                ("assistant", "Looking."),
                ("tool_call", "shell\n{\"command\":\"ls\"}"),
                ("tool_result", "shell\na.txt"),
            ]
        );
    }

    #[test]
    fn a_paste_keeps_its_text_literally_and_never_sends() {
        let mut app = app();
        let pasted = "fn main() {\n    println!(\"hi\");\n}\n:wq";
        app.handle_paste(pasted);
        assert!(app.transcript.is_empty(), "a paste is not a turn");
        assert_eq!(app.status.turn, TurnState::Idle);
        assert_eq!(app.composer.text(), pasted);
        assert_eq!(app.composer.line_count(), 4);
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::Submit(pasted.to_owned()))
        );
    }

    #[test]
    fn a_paste_during_a_turn_waits_in_the_composer_like_typed_text() {
        let mut app = app();
        app.status.turn = TurnState::Working;
        app.handle_paste("next\nprompt");
        assert_eq!(app.composer.text(), "next\nprompt");
        assert!(app.transcript.is_empty());
        assert_eq!(app.status.turn, TurnState::Working);
    }

    #[test]
    fn a_paste_is_dropped_while_a_modal_owns_the_keys() {
        let mut app = app();
        app.open_help();
        app.handle_paste("ignored");
        assert!(app.composer.is_empty());
    }

    #[test]
    fn enter_queues_while_a_turn_runs_and_the_turn_end_starts_the_first_one() {
        let mut app = app();
        app.status.turn = TurnState::Working;
        type_text(&mut app, "first queued");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert!(
            app.composer.is_empty(),
            "the queued prompt leaves the draft"
        );
        type_text(&mut app, "second queued");
        assert_eq!(press(&mut app, KeyCode::Enter), None);

        assert_eq!(app.queued().len(), 2);
        assert!(
            app.status_line().contains("queued 2"),
            "{}",
            app.status_line()
        );
        assert!(
            app.transcript
                .rows()
                .iter()
                .all(|row| row.kind != RowKind::User),
            "a queued prompt is not a sent message: {:?}",
            app.transcript.rows()
        );
        assert_eq!(app.status.turn, TurnState::Working, "nothing was started");

        assert_eq!(
            app.finish(Ok(outcome())),
            Some(Command::Submit("first queued".to_owned())),
            "the turn ended, so the first queued prompt starts"
        );
        assert_eq!(app.queued().len(), 1);
        assert_eq!(app.status.turn, TurnState::Working);
        assert_eq!(
            app.transcript.rows().last().map(|row| row.text.as_str()),
            Some("first queued")
        );
        assert!(
            app.status_line().contains("queued 1"),
            "{}",
            app.status_line()
        );
    }

    #[test]
    fn a_cancelled_turn_discards_the_queue() {
        let mut app = app();
        app.status.turn = TurnState::Working;
        type_text(&mut app, "waiting");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert_eq!(app.queued().len(), 1);

        assert_eq!(press(&mut app, KeyCode::Esc), Some(Command::Cancel));
        assert!(app.queued().is_empty(), "a stopped turn discards its queue");
        assert!(
            !app.status_line().contains("queued"),
            "{}",
            app.status_line()
        );
    }

    #[test]
    fn clear_and_quit_discard_the_queue() {
        let mut app = app();
        app.status.turn = TurnState::Working;
        type_text(&mut app, "waiting");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        type_text(&mut app, "/quit");
        assert_eq!(press(&mut app, KeyCode::Enter), Some(Command::Quit));
        assert!(app.queued().is_empty());

        let mut app = self::app();
        app.status.turn = TurnState::Working;
        type_text(&mut app, "waiting");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert_eq!(app.finish(Err("boom".to_owned())), None);
        assert_eq!(app.status.turn, TurnState::Failed);
        assert_eq!(app.queued().len(), 1, "a failed turn keeps the queue");

        type_text(&mut app, "/clear");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::ClearApprovals)
        );
        assert!(app.queued().is_empty());
    }

    #[test]
    fn alt_enter_steers_a_running_turn_without_starting_a_second_one() {
        let mut app = app();
        app.status.turn = TurnState::Working;
        app.apply(AgentEvent::Text("half an answer".to_owned()));

        type_text(&mut app, "also check the tests");
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
        assert_eq!(
            app.handle_key(key),
            Some(Command::Steer("also check the tests".to_owned()))
        );
        assert!(app.composer.is_empty());
        assert_eq!(app.status.turn, TurnState::Working, "no second turn starts");
        assert!(app.queued().is_empty(), "a steer is not queued");
        let rows = app.transcript.rows();
        assert_eq!(rows[1].kind, RowKind::User);
        assert_eq!(rows[1].text, "also check the tests");

        let control = KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL);
        type_text(&mut app, "and the docs");
        assert_eq!(
            app.handle_key(control),
            Some(Command::Steer("and the docs".to_owned())),
            "ctrl+enter steers when the terminal reports it"
        );
    }

    #[test]
    fn a_second_escape_restores_the_last_prompt_and_drops_its_rows() {
        let mut app = app();
        type_text(&mut app, "fix the parser");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::Submit("fix the parser".to_owned()))
        );
        app.apply(AgentEvent::Text("patched".to_owned()));
        assert_eq!(app.finish(Ok(outcome())), None);
        assert_eq!(app.transcript.rows().len(), 2);

        assert_eq!(press(&mut app, KeyCode::Esc), None, "the first one arms it");
        assert!(app.composer.is_empty());
        assert_eq!(app.transcript.rows().len(), 2, "one press changes nothing");

        assert_eq!(press(&mut app, KeyCode::Esc), Some(Command::Rewind));
        assert_eq!(app.composer.text(), "fix the parser");
        assert_eq!(app.composer.row_col(), (0, 14), "the cursor ends after it");
        assert!(
            app.transcript
                .rows()
                .iter()
                .all(|row| row.kind != RowKind::User),
            "{:?}",
            app.transcript.rows()
        );
        assert_eq!(app.transcript.rows().len(), 0, "the answer goes with it");
        assert_eq!(app.status.turn, TurnState::Idle);
    }

    #[test]
    fn a_single_escape_does_not_backtrack() {
        let mut app = app();
        type_text(&mut app, "keep me");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::Submit("keep me".to_owned()))
        );
        assert_eq!(app.finish(Ok(outcome())), None);

        assert_eq!(press(&mut app, KeyCode::Esc), None);
        assert!(app.composer.is_empty(), "the draft stays empty");
        assert_eq!(
            app.transcript.rows().len(),
            1,
            "the row is still there: {:?}",
            app.transcript.rows()
        );
    }

    #[test]
    fn a_second_escape_after_the_window_only_arms_the_backtrack_again() {
        let mut app = app();
        type_text(&mut app, "too late");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::Submit("too late".to_owned()))
        );
        app.status.turn = TurnState::Idle;
        app.last_escape = Some(Instant::now() - BACKTRACK_WINDOW - Duration::from_millis(1));

        assert_eq!(press(&mut app, KeyCode::Esc), None);
        assert_eq!(app.transcript.rows().len(), 1);
        assert!(app.composer.is_empty());
    }

    #[test]
    fn a_bang_line_runs_under_the_policy_without_the_model() {
        let mut app = app();
        app.status.mode = Mode::Yolo;
        type_text(&mut app, "!printf hello");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::Shell("printf hello".to_owned()))
        );
        assert!(app.composer.is_empty());
        assert_eq!(
            app.status.turn,
            TurnState::Idle,
            "a shell line is not a turn"
        );
        let rows = app.transcript.rows();
        assert_eq!(rows.len(), 1, "the call row; the result is the host's");
        assert_eq!(rows[0].kind, RowKind::ToolCall);
        assert_eq!(rows[0].title.as_deref(), Some("shell"));
        assert_eq!(rows[0].text, r#"{"command":"printf hello"}"#);

        app.apply(AgentEvent::ToolEnd {
            name: "shell".to_owned(),
            output: "exit_code: 0\n--- stdout ---\nhello\n".to_owned(),
            is_error: false,
        });
        assert_eq!(app.transcript.rows().len(), 2);
        assert_eq!(app.transcript.rows()[1].kind, RowKind::ToolResult);
    }

    #[test]
    fn a_bang_line_in_auto_runs_only_when_the_allow_list_permits_it() {
        let mut app = app();
        app.status.mode = Mode::Auto;
        type_text(&mut app, "!printf hi");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert_eq!(
            app.transcript.rows().last().map(|row| row.kind),
            Some(RowKind::Error),
            "auto asks for a command it cannot permit, and nobody can answer"
        );

        let mut permitted = self::app();
        permitted.status.mode = Mode::Auto;
        permitted.set_allow_list(AllowList::parse("printf *"));
        type_text(&mut permitted, "!printf hi");
        assert_eq!(
            press(&mut permitted, KeyCode::Enter),
            Some(Command::Shell("printf hi".to_owned()))
        );
    }

    #[test]
    fn a_bang_line_in_manual_asks_when_someone_can_answer() {
        let mut app = app();
        app.set_approver(true);
        type_text(&mut app, "!cargo publish");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert!(matches!(app.modal, Some(Modal::Approval(_))));
        assert_eq!(app.status.turn, TurnState::Idle, "no turn is waiting");
        assert!(
            app.transcript.is_empty(),
            "the call is shown only once it may run: {:?}",
            app.transcript.rows()
        );

        assert_eq!(
            press(&mut app, KeyCode::Char('y')),
            Some(Command::Shell("cargo publish".to_owned()))
        );
        assert!(app.modal.is_none());
        let rows = app.transcript.rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, RowKind::ToolCall);
    }

    #[test]
    fn a_session_answer_covers_the_program_until_clear() {
        let mut app = app();
        app.set_approver(true);
        type_text(&mut app, "!cargo publish");
        assert_eq!(press(&mut app, KeyCode::Enter), None, "manual asks first");
        assert_eq!(
            press(&mut app, KeyCode::Char('s')),
            Some(Command::Shell("cargo publish".to_owned()))
        );

        type_text(&mut app, "!cargo test");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::Shell("cargo test".to_owned())),
            "the same program does not ask again"
        );
        assert!(app.modal.is_none());
        assert!(
            !app.allow.permits("cargo test"),
            "the session rule never reaches the configured allow-list"
        );

        type_text(&mut app, "!git status");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert!(
            matches!(app.modal, Some(Modal::Approval(_))),
            "a different program still asks"
        );
        assert_eq!(press(&mut app, KeyCode::Char('d')), None);
        assert!(app.modal.is_none());
        assert!(
            app.transcript
                .rows()
                .last()
                .is_some_and(|row| row.kind == RowKind::Error && row.text.contains("not approved")),
            "the denial is recorded like any refused `!` line"
        );

        type_text(&mut app, "/clear");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::ClearApprovals)
        );
        type_text(&mut app, "!cargo publish");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert!(
            matches!(app.modal, Some(Modal::Approval(_))),
            "/clear forgets the session rule"
        );
    }

    #[test]
    fn a_bang_line_in_manual_with_no_approver_is_refused() {
        let mut app = app();
        type_text(&mut app, "!touch refused.txt");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert!(app.modal.is_none(), "nobody could answer a prompt");
        let rows = app.transcript.rows();
        assert_eq!(rows.len(), 2, "the call and the refusal: {rows:?}");
        assert_eq!(rows[0].kind, RowKind::ToolCall);
        assert_eq!(rows[1].kind, RowKind::Error);
        assert!(rows[1].text.contains("nobody to ask"), "{}", rows[1].text);
        assert_eq!(app.status.turn, TurnState::Idle);
    }

    #[test]
    fn a_bang_with_only_whitespace_does_nothing() {
        let mut app = app();
        type_text(&mut app, "!   ");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert!(app.transcript.is_empty());
        assert!(app.modal.is_none());
        assert_eq!(app.status.turn, TurnState::Idle);
    }

    #[test]
    fn view_commands_wait_for_the_running_turn() {
        let mut app = app();
        app.status.turn = TurnState::Working;
        for command in ["/clear", "/compact", "/context", "/resume", "/threads"] {
            type_text(&mut app, command);
            assert_eq!(press(&mut app, KeyCode::Enter), None);
            let last = app.transcript.rows().last().expect("notice");
            assert!(
                last.text.contains("waits until the turn finishes"),
                "{command}: {}",
                last.text
            );
        }
        assert_eq!(app.session_id(), None, "nothing was cleared or resumed");
    }

    fn card_fields(app: &mut App) -> Vec<(String, String)> {
        app.open_status();
        let Some(Modal::Status(card)) = app.modal.take() else {
            panic!("the status card is open");
        };
        card.fields()
            .iter()
            .map(|field| (field.label.clone(), field.value.clone()))
            .collect()
    }

    fn value<'a>(fields: &'a [(String, String)], label: &str) -> Option<&'a str> {
        fields
            .iter()
            .find(|(name, _)| name == label)
            .map(|(_, value)| value.as_str())
    }

    #[test]
    fn the_status_command_opens_and_closes_the_card_without_a_transcript_row() {
        let mut app = app();
        let before = app.transcript.rows().len();
        type_text(&mut app, "/status");
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert!(matches!(app.modal, Some(Modal::Status(_))));
        assert!(app.composer.is_empty());
        assert_eq!(app.transcript.rows().len(), before, "no row was added");
        assert_eq!(app.status.turn, TurnState::Idle);

        assert_eq!(press(&mut app, KeyCode::Esc), None);
        assert!(app.modal.is_none(), "escape closes the card");

        app.open_status();
        stroke(&mut app, KeyCode::Char('q'));
        assert!(app.modal.is_none(), "q closes the card");

        app.open_status();
        assert!(matches!(app.modal, Some(Modal::Status(_))));
        app.open_status();
        assert!(app.modal.is_none(), "opening it again closes it");
        assert_eq!(app.transcript.rows().len(), before);
    }

    #[test]
    fn the_status_card_reports_the_state_the_app_holds() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let directory = project.path().join(".niki/skills/deploy");
        std::fs::create_dir_all(&directory).expect("skill directory");
        std::fs::write(
            directory.join("SKILL.md"),
            "---\nname: deploy\ndescription: Ship a release safely.\n---\nbody\n",
        )
        .expect("skill");

        let mut app = app();
        app.attach_sessions(store(&temp));
        let session = format!("{} new", app.session_id().expect("session"));
        let skills = SkillCatalog::discover(project.path(), user.path());
        let skill_count = skills.list().len().to_string();
        app.set_skills(skills);
        app.set_tools(vec![
            "read_file".to_owned(),
            "mcp__fixture__echo".to_owned(),
            "mcp__fixture__clock".to_owned(),
        ]);
        app.set_session_file("/home/me/.niki/sessions.db");
        app.status.context_window = Some(8_000);
        app.set_context(
            ContextReport {
                messages: 2,
                estimated_tokens: 1_000,
                last_input_tokens: 900,
                last_output_tokens: 100,
                steps: 1,
            },
            SessionUsage {
                input_tokens: 900,
                output_tokens: 100,
                steps: 1,
            },
        );
        let client = format!("niki {}", env!("CARGO_PKG_VERSION"));

        let fields = card_fields(&mut app);
        assert_eq!(value(&fields, "model"), Some("mock:mock-model"));
        assert_eq!(value(&fields, "provider"), Some("mock"));
        assert_eq!(value(&fields, "mode"), Some("manual"));
        assert_eq!(value(&fields, "session"), Some(session.as_str()));
        assert_eq!(
            value(&fields, "session file"),
            Some("/home/me/.niki/sessions.db")
        );
        assert_eq!(value(&fields, "workspace"), Some("/work"));
        assert_eq!(value(&fields, "tokens"), Some("1,000 / 8,000 (13%)"));
        assert_eq!(value(&fields, "skills"), Some(skill_count.as_str()));
        assert_ne!(
            skill_count, "0",
            "the catalog the test discovered is not empty"
        );
        assert_eq!(value(&fields, "mcp tools"), Some("2"));
        assert_eq!(value(&fields, "client"), Some(client.as_str()));
    }

    #[test]
    fn the_status_card_says_what_it_does_not_know_and_omits_absent_mcp_tools() {
        let mut app = app();
        app.set_tools(vec!["read_file".to_owned(), "shell".to_owned()]);
        let fields = card_fields(&mut app);
        assert!(
            !fields.iter().any(|(label, _)| label == "mcp tools"),
            "no MCP tools, so no line: {fields:?}"
        );
        assert_eq!(value(&fields, "tokens"), Some("not measured yet"));
        assert_eq!(value(&fields, "session"), Some("not saved"));
        assert_eq!(value(&fields, "session file"), Some("unknown"));
    }

    #[test]
    fn a_saved_turn_drops_the_new_marker_from_the_session() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = app();
        app.attach_sessions(store(&temp));
        let id = app.session_id().expect("session").to_owned();
        type_text(&mut app, "hello");
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Command::Submit("hello".to_owned()))
        );
        assert_eq!(app.finish(Ok(outcome())), None);
        let fields = card_fields(&mut app);
        assert_eq!(value(&fields, "session"), Some(id.as_str()));
    }
}
