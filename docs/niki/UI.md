# Niki Agent — the interface

The full-screen app is a Rust `ratatui` program in `niki-tui-live`. This page is
the contract for what it draws: the regions, the markers, the motion schedule,
and the settings that change them. The state layer is terminal-free, so every
claim here is covered by a `TestBackend` buffer test in
`rust/crates/niki-tui-live/tests/screen.rs`.

## The screen

```
NIKI AGENT  v2.2.0
one binary · sessions on your disk · an approval gate in front of every change

ollama:test-model · manual mode · /home/you/project

To get started
  /init    write an AGENTS.md for this repository
  /status  model, mode, session, and token usage
  /theme   pick a palette
  /help    every command and key binding

Type a prompt to begin. Ctrl+D quits.

──────────────────────────────────────────────────────────────────────────
› Type a prompt; / for commands, ctrl+d to quit
ollama:test-model · manual · idle · /home/you/project
/ commands · ? keys · ctrl+t transcript · ctrl+d quit
```

Four regions, top to bottom:

- **Transcript.** Answer text, tool calls and results, diffs, notices, and the
  prompt queue. It scrolls with `PageUp`/`PageDown` and the wheel.
- **Composer.** One hairline rule, then the draft. The `›` marks where input
  begins; the cursor sits after it.
- **Command list.** When the draft starts with `/`, the matching commands and
  the highlighted one's description sit under the composer.
- **Footer.** Two lines: the state contract, then the second line described
  below.

On the first screen — before any prompt has been sent — the transcript region
holds the welcome block. A notice such as a trust warning is drawn *beneath* the
welcome rather than replacing it, so the first thing a new person sees is never
a warning and blank space.

## The footer

Line one is the state contract and never carries counts:

```
model · approval mode · turn state · working directory
```

While a turn runs the state field carries the spinner. When prompts are queued
the count is appended (`queued 2`).

Line two changes with the session:

- **Before the first prompt:** key hints, ordered most useful first and dropped
  a whole key at a time as the terminal narrows:
  `/ commands · ? keys · ctrl+t transcript · ctrl+d quit`.
- **After a turn has started:** context and token accounting,
  `ctx 13% · 1,234 tok`, once the agent has reported them.

Keeping counts off line one means a long session cannot push the working
directory off the screen, and the second line teaches keys exactly once — when
they are needed.

## Glyphs

| Glyph | Where | Meaning |
| --- | --- | --- |
| `›` | composer, prompts | input starts here; `>` in ASCII mode |
| `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` | state field | a turn is running, one frame per 100 ms |
| `◌` | state field | reduced motion: still working, not moving |
| `•` / `◦` | a running tool call | the step is alive; dims every 600 ms |
| `⏺` | answer rows | the model's text |
| `⚙` | tool rows | a call that has finished or is not the newest |
| `!` | error rows | a failed call or turn |
| `-` | notice rows | a note from the app |

ASCII mode (`NIKI_ASCII=1`) replaces every marker with an ASCII one: `>`, `|`,
`/`, `-`, `\`, `o`, `*`, `#`.

## Motion

Every time-varying glyph lives in `rust/crates/niki-tui-live/src/motion.rs`:

| Effect | Schedule | Reduced motion |
| --- | --- | --- |
| Loading spinner | 100 ms per frame, ten frames | static `◌` (`o` in ASCII) |
| Activity marker | `•`⇄`◦` every 600 ms | static `•` |
| Window title blink | 500 ms while waiting for an answer | static marker |

The event loop wakes once per 100 ms **only while a turn runs**. An idle session
draws no frames and writes no title.

`NIKI_REDUCED_MOTION=1` is the master switch. It never hides the information
that something is happening; it replaces movement with a still glyph. It also
wins over the configuration file so a recording or a CI run is deterministic.

## The terminal title

The window title is `project — state`, for example `niki-agent — working`.
States are `idle`, `working`, `needs you`, `cancelled` and `failed`. While the
turn waits for an answer the title leads with a `●`/`○` marker that blinks every
half second, so an unfocused terminal still says it needs you. ASCII mode uses
` - ` and `*`/`o`. Control characters are stripped from the project name, so a
path cannot inject a terminal escape. The title is cleared when the app leaves.

## Settings

All of it is configuration, in `$NIKI_HOME/config.toml` (or `~/.niki/config.toml`):

```toml
[ui]
theme = "niki"          # niki, high-contrast, colorblind, mono
motion = "full"         # full or reduced
welcome = true          # the first-screen block
progress = true         # the spinner beside the turn state
shimmer = true          # the running-tool marker
title = true            # the terminal window title
```

Every effect defaults to on. Turning one off removes that feature, never the
information behind it: with `progress = false` the state word still says
`working`; with `welcome = false` the footer still carries the model, mode and
directory.

## Accessibility

- **Contrast.** Every palette is gated on WCAG AA by a test that computes the
  real ratio for body text, muted text, accents, errors, confirmation, and panel
  boundaries. `mono` carries meaning by weight and marker rather than hue.
- **Colour-blind safe.** The `colorblind` palette uses Okabe–Ito hues.
- **Reduced motion.** One switch stops every animation and leaves a static
  marker in its place.
- **ASCII.** One switch removes every non-ASCII character from the screen,
  including the composer rule and the title separator.
- **No animation before the first frame.** Nothing is drawn on a schedule an
  idle session cannot see.

## Deliberately not done

Two effects other terminal agents ship are left out on purpose. Both were
considered and declined, not forgotten:

- **A rotating verb above the composer** ("thinking", "cooking", "noodling" on a
  3.5 s cycle). It occupies a row to say what the spinner already says, and a
  rotating word is the first thing a screen reader announces on every wake. The
  state field is a contract: `working` means working.
- **A per-cell composer animation** (a drifting starfield or travelling
  highlight in the input area). It redraws the region the person is typing in,
  which is the one place in the app where motion actively costs legibility, and
  it defeats the reduced-motion switch because the effect *is* the content.

The rule behind both: motion may show that work is happening, and may orient
someone in the window. It may not decorate the thing they are reading.
