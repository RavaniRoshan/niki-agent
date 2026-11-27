//! The terminal event loop: raw mode, key events, redraws.
//!
//! This is the only module that needs a terminal. Everything it drives is
//! [`App`] and [`WorkerHandle`], so the interesting behaviour is tested without
//! one.

use std::io::{self, Stdout, Write};
use std::time::Duration;

use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use niki_agent::Approval;
use niki_tools::{AskAnswer, AskError};
use niki_tui_modals::QuestionReply;
use tokio::sync::oneshot;

use crate::app::{App, Command};
use crate::clipboard;
use crate::editor::{self, TempPrompt};
use crate::history::History;
use crate::ui;
use crate::worker::{WorkerEvent, WorkerHandle};

/// Redraw interval while a turn is running, so the spinner moves.
const BUSY_TICK: Duration = Duration::from_millis(80);
/// How long an idle loop waits for input before looking at the worker again.
/// A timeout redraws nothing: an idle session paints only when something
/// changed.
const IDLE_WAIT: Duration = Duration::from_secs(60);
/// Turns `/compact` keeps verbatim when the agent summarises the rest.
const COMPACT_KEEP_TURNS: usize = 6;

/// Run the application until the person leaves it.
///
/// Returns the process exit code: `0` after a normal quit, `130` when the last
/// turn was cancelled, as the headless mode does.
///
/// # Errors
///
/// Returns the terminal failure that stopped the screen. The terminal is
/// restored before the error reaches the caller.
pub fn run(app: &mut App, worker: &mut WorkerHandle) -> io::Result<i32> {
    app.attach_history(History::from_env());
    let mut screen = Screen::enter()?;
    let outcome = event_loop(&mut screen, app, worker);
    screen.leave();
    let code = outcome?;
    Ok(code)
}

struct Screen {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    active: bool,
}

impl Screen {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        enable_modes(&mut stdout)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self {
            terminal,
            active: true,
        })
    }

    fn leave(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        let _ = disable_raw_mode();
        let _ = disable_modes(self.terminal.backend_mut());
        let _ = self.terminal.show_cursor();
        let _ = io::stdout().flush();
    }

    /// Give the terminal to another program until the returned guard is
    /// dropped.
    ///
    /// The exit steps are best effort: a failure to leave one mode does not
    /// stop the others, and the guard still tries to restore everything on the
    /// way back.
    fn suspend(&mut self) -> Suspended<'_> {
        let _ = disable_raw_mode();
        let _ = disable_modes(self.terminal.backend_mut());
        let _ = self.terminal.show_cursor();
        let _ = io::stdout().flush();
        Suspended { screen: self }
    }

    /// Take the terminal back after [`Self::suspend`], repainting from scratch.
    fn resume(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        enable_modes(self.terminal.backend_mut())?;
        // The other program painted the primary screen while the alternate one
        // was gone, so the frame ratatui has cached no longer matches the
        // terminal.
        self.terminal.clear()
    }
}

/// A suspended screen. Dropping it restores the terminal, whatever happened
/// while it was suspended — the editor failing is not a reason to leave the
/// person in a cooked tty with no prompt.
struct Suspended<'a> {
    screen: &'a mut Screen,
}

impl Drop for Suspended<'_> {
    fn drop(&mut self) {
        let _ = self.screen.resume();
    }
}

/// The terminal modes the screen needs, in the order they are entered.
///
/// Bracketed paste makes a terminal deliver a paste as one [`Event::Paste`]
/// instead of a burst of key presses, so a pasted newline cannot send the
/// prompt.
fn enable_modes(out: &mut impl Write) -> io::Result<()> {
    execute!(
        out,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )
}

/// Undo [`enable_modes`], whatever happened to the screen.
fn disable_modes(out: &mut impl Write) -> io::Result<()> {
    execute!(
        out,
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste
    )
}

impl Drop for Screen {
    fn drop(&mut self) {
        self.leave();
    }
}

fn event_loop(screen: &mut Screen, app: &mut App, worker: &mut WorkerHandle) -> io::Result<i32> {
    // The channel the turn waits on while the person decides. Dropping it
    // without an answer makes the turn read a refusal, which is the safe way to
    // leave a question nobody answered.
    let mut reply: Option<oneshot::Sender<Approval>> = None;
    // The channel a blocked `request_user_input` waits on. Dropping it without
    // an answer makes the tool report a dismissal, which is the safe way to
    // leave a question nobody answered.
    let mut question_reply: Option<std::sync::mpsc::Sender<Result<AskAnswer, AskError>>> = None;
    screen.terminal.draw(|frame| ui::draw(frame, app))?;
    loop {
        let mut changed = false;
        while let Some(message) = worker.try_recv() {
            match message {
                WorkerEvent::Agent(event) => app.apply(event),
                WorkerEvent::Approval {
                    request,
                    reply: answer,
                } => {
                    reply = Some(answer);
                    app.request_approval(request);
                }
                WorkerEvent::Question {
                    request,
                    reply: answer,
                } => {
                    question_reply = Some(answer);
                    app.request_question(request);
                }
                WorkerEvent::Finished(result) => {
                    reply = None;
                    question_reply = None;
                    if let Some(Command::Submit(prompt)) = app.finish(result) {
                        worker.submit(prompt);
                    }
                }
                WorkerEvent::Context {
                    report,
                    usage,
                    announce,
                } => {
                    if announce {
                        app.show_context(report, usage);
                    } else {
                        app.set_context(report, usage);
                    }
                }
                WorkerEvent::Compacted { result, fallback } => app.compacted(result, fallback),
            }
            changed = true;
        }

        let busy = app.status.turn.is_busy();
        if busy {
            app.tick();
            changed = true;
        }
        if changed {
            screen.terminal.draw(|frame| ui::draw(frame, app))?;
        }

        if !event::poll(if busy { BUSY_TICK } else { IDLE_WAIT })? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match app.handle_key(key) {
                Some(Command::Submit(prompt)) => {
                    worker.submit(prompt);
                }
                Some(Command::Steer(text)) => {
                    worker.steer(text);
                }
                Some(Command::Shell(command)) => {
                    worker.shell(command, app.status.cwd.clone());
                }
                Some(Command::Rewind) => {
                    worker.rewind();
                }
                Some(Command::Cancel) => {
                    reply = None;
                    question_reply = None;
                    worker.cancel();
                }
                Some(Command::Mode(mode)) => {
                    worker.set_mode(mode);
                }
                Some(Command::Answer(answer)) => {
                    if let Some(answer_to) = reply.take() {
                        let _ = answer_to.send(answer);
                    }
                }
                Some(Command::AnswerQuestion(answer)) => {
                    if let Some(answer_to) = question_reply.take() {
                        let result = match answer {
                            QuestionReply::Choice(label) => Ok(AskAnswer::Choice(label)),
                            QuestionReply::Freeform(text) => Ok(AskAnswer::Freeform(text)),
                            QuestionReply::Cancelled => Err(AskError::Failed(
                                "the question was cancelled; ask again or continue without it"
                                    .to_owned(),
                            )),
                        };
                        let _ = answer_to.send(result);
                    }
                }
                Some(Command::SelectModel(spec)) => {
                    app.status.model.clone_from(&spec);
                    worker.select_model(spec);
                }
                Some(Command::Compact) => {
                    worker.compact(COMPACT_KEEP_TURNS);
                }
                Some(Command::Context) => {
                    worker.context();
                }
                Some(Command::RefreshSystem(system)) => {
                    worker.set_system(system);
                }
                Some(Command::ClearApprovals) => {
                    worker.clear_approvals();
                }
                Some(Command::Copy(text)) => {
                    let mut out = io::stdout();
                    let _ = clipboard::copy(&mut out, &text);
                }
                Some(Command::EditPrompt) => edit_prompt(screen, app),
                Some(Command::Quit) => return Ok(exit_code(app)),
                None => {}
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => app.wheel(true),
                MouseEventKind::ScrollDown => app.wheel(false),
                _ => {}
            },
            Event::Paste(text) => app.handle_paste(&text),
            _ => {}
        }
        screen.terminal.draw(|frame| ui::draw(frame, app))?;
    }
}

/// Hand the draft to the person's editor and take back what it left.
///
/// The file is written before the screen is suspended, so a failure to write it
/// leaves the UI untouched. The [`Suspended`] guard keeps the terminal out of
/// the app's hands for exactly the editor's lifetime, including the path where
/// the editor cannot start.
fn edit_prompt(screen: &mut Screen, app: &mut App) {
    let draft = app.composer.text().to_owned();
    let file = match TempPrompt::create(&draft) {
        Ok(file) => file,
        Err(error) => {
            app.finish_edit(Err(format!("could not write the editor file: {error}")));
            return;
        }
    };
    let command = editor::resolve_editor_from_env();
    let result = {
        let _suspended = screen.suspend();
        editor::edit_file(&command, file.path())
    };
    app.finish_edit(result.map_err(|error| error.warning()));
}

fn exit_code(app: &App) -> i32 {
    if app.status.turn == crate::TurnState::Cancelled {
        130
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_enables_bracketed_paste_and_disables_it_on_the_way_out() {
        let mut entered = Vec::new();
        enable_modes(&mut entered).expect("enable the terminal modes");
        let entered = String::from_utf8(entered).expect("ANSI text");
        assert!(entered.contains("\x1b[?2004h"), "{entered:?}");
        assert!(
            entered.contains("\x1b[?1049h"),
            "the alternate screen: {entered:?}"
        );

        let mut left = Vec::new();
        disable_modes(&mut left).expect("disable the terminal modes");
        let left = String::from_utf8(left).expect("ANSI text");
        assert!(left.contains("\x1b[?2004l"), "{left:?}");
        assert!(
            left.contains("\x1b[?1049l"),
            "the alternate screen is left: {left:?}"
        );
    }
}
