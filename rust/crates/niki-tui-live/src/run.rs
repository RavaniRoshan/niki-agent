//! The terminal event loop: raw mode, key events, redraws.
//!
//! This is the only module that needs a terminal. Everything it drives is
//! [`App`] and [`WorkerHandle`], so the interesting behaviour is tested without
//! one.

use std::io::{self, Stdout, Write};
use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::{App, Command};
use crate::ui;
use crate::worker::{WorkerEvent, WorkerHandle};

/// Redraw interval while a turn is running, so the spinner moves.
const BUSY_TICK: Duration = Duration::from_millis(80);
/// Redraw interval while nothing is happening.
const IDLE_TICK: Duration = Duration::from_millis(250);
/// Lines one wheel notch moves.
const WHEEL_LINES: usize = 3;

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
    let mut screen = Screen::enter()?;
    let outcome = event_loop(&mut screen.terminal, app, worker);
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
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
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
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
        let _ = io::stdout().flush();
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        self.leave();
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    worker: &mut WorkerHandle,
) -> io::Result<i32> {
    loop {
        while let Some(message) = worker.try_recv() {
            match message {
                WorkerEvent::Agent(event) => app.apply(event),
                // The approval prompt belongs to the modal layer; dropping the
                // reply channel declines the command, which is the safe answer.
                WorkerEvent::Approval { .. } => {}
                WorkerEvent::Finished(result) => app.finish(result),
            }
        }

        terminal.draw(|frame| ui::draw(frame, app))?;
        let busy = app.status.turn.is_busy();
        if busy {
            app.tick();
        }
        if !event::poll(if busy { BUSY_TICK } else { IDLE_TICK })? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match app.handle_key(key) {
                Some(Command::Submit(prompt)) => {
                    worker.submit(prompt);
                }
                Some(Command::Cancel) => worker.cancel(),
                Some(Command::Mode(mode)) => {
                    worker.set_mode(mode);
                }
                Some(Command::Quit) => return Ok(exit_code(app)),
                None => {}
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => app.scroll.up(WHEEL_LINES),
                MouseEventKind::ScrollDown => app.scroll.down(WHEEL_LINES),
                _ => {}
            },
            _ => {}
        }
    }
}

fn exit_code(app: &App) -> i32 {
    if app.status.turn == crate::TurnState::Cancelled {
        130
    } else {
        0
    }
}
