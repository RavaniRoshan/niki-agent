<!--
  Mirrored from docs/niki/UI.md by docs-site/scripts/sync-docs.mjs.
  Do not edit this copy: edit the source file and run `npm run sync`.
  source-sha256: c17487fc21242ec8f1c9833e53ee923d54bb084b41258b43fd118c36bbe0ad97
-->
# Niki Agent — the interface

The full-screen app is a Rust `ratatui` program in `niki-tui-live`. This page is
the contract for what it draws: the regions, the markers, the motion schedule,
and the settings that change them. The state layer is terminal-free, so every
claim here is covered by a `TestBackend` buffer test in
`rust/crates/niki-tui-live/tests/screen.rs`.

## The screen

```text
  >_ NIKI AGENT (v2.3.0)
     ~/project

  To get started, describe a task or try one of these commands:

  /init - write an AGENTS.md for this repository
  /status - model, mode, session and token usage
  /model - choose a model
  /theme - choose a palette
  /help - every command and key binding

──────────────────────────────────────────────────────────────────────────
› Ask Niki to do anything
ollama:test-model · manual · idle · ~/project
/ commands · ? keys · ctrl+t transcript · ctrl+d quit
```text

Five regions, top to bottom:

- **Transcript.** Answer text, tool calls and results, diffs, notices, and the
  prompt queue. It scrolls with `PageUp`/`PageDown` and the wheel.
- **Busy row.** One line, drawn only while a turn runs; see below.
- **Composer.** One hairline rule, then the draft. The `›` marks where input
  begins; the cursor sits after it.
- **Command list.** When the draft starts with `/`, the matching commands and
  the highlighted one's description sit under the composer.
- **Footer.** Two lines: the state contract, then hints and counts.

On the first screen — before any prompt has been sent — the transcript region
holds the welcome block. A notice such as a trust warning is drawn *beneath* the
welcome rather than replacing it, so the first thing a new person sees is never
a warning and blank space.

The welcome block is brand mark, location, one sentence and five commands. It is
not a README: the tagline belongs to the site, and prose here pushes the command
list off a 24-row terminal. The directory is shown relative to `$HOME` so a deep
checkout still fits on one line.

A narrow terminal gets the same block, not a cropped one: the sentence wraps at
word boundaries, a command summary with no room left ends in `…`, and a state
line wider than the footer is ellipsized the same way. A footer hint that does
not fit is dropped whole, never cut mid-word.

## The footer

Line one is the state contract and never carries counts:

```text
model · approval mode · branch · turn state · working directory
```

The branch field appears only inside a repository, and never for a detached
HEAD. While a turn waits for an answer the state field carries the spinner; a
working turn does not, because the busy row already says so, one line up. When
prompts are queued the count is appended (`queued 2`).

Line two carries both halves of what is left:

```text
/ commands · ? keys · ctrl+t transcript · ctrl+d quit            ctx 13% · 1,234 tok
```text

The hints stay for the life of the session — a person who has run a hundred
turns still forgets a chord — and are ordered most useful first. They drop a
whole key at a time as the terminal narrows. The counts are right-aligned and
are the last thing to go: below twelve columns of room the counts take the row
alone, because at that width `ctx 13%` is the more useful half.

Keeping counts off line one means a long session cannot push the working
directory off the screen.

## The busy row

While a turn runs, one line sits directly above the composer:

```text
• Working (12s · esc to interrupt)
```text

The bullet is the activity marker (see Motion); the clock is seconds, then
`1m 05s`, then `1h 00m 05s`. The clock starts on the first frame that sees a
working turn and is cleared when the turn stops, so it survives steering,
approval and cancellation without any of those paths having to remember it.

The row is the answer to "is it stuck?". Without it, second two and second two
hundred look identical. It never appears while the app waits for an approval:
the approval card is the thing to read then.

## The key overlay

`?` opens the key map, drawn in up to three columns — `Compose`, `Session`,
`Transcript` — that fold to two and then one as the terminal narrows. The
collapse is by measured width, not by a breakpoint, so a chord is never cut in
half. Below the columns come the slash commands, and the last row is always
`? or escape closes; page up and page down scroll`, even when the body is
taller than the terminal.

## Glyphs

| Glyph | Where | Meaning |
| --- | --- | --- |
| `›` | composer, prompts | input starts here; `>` in ASCII mode |
| `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` | state field while waiting | a turn needs you, one frame per 100 ms |
| `◌` | state field | reduced motion: waiting, not moving |
| `•` / `◦` | busy row, a running call | the step is alive; dims every 600 ms |
| `⏺` | answer rows | the model's text |
| `⚙` | tool rows | the call the model made |
| `✓` | result rows | the call ran; `+` in ASCII mode |
| `×` | error rows | the call failed or was refused; `x` in ASCII mode |
| `-` | notice rows | a note from the app |

A result with nothing to show still gets a row: `✓ shell (no output)`. An empty
row would read as a lost event.

ASCII mode (`NIKI_ASCII=1`) replaces every marker with an ASCII one: `>`, `|`,
`*`, `o`, `#`, `+`, `x`, `-`.

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
```text

Every effect defaults to on. Turning one off removes that feature, never the
information behind it: with `progress = false` the state word still says
`working`; with `welcome = false` the footer still carries the model, mode and
directory.

`/settings` opens the same keys as a list. `Enter` or space cycles the
highlighted row, `Up`/`Down` move, `Esc` closes. Each change applies to the
running screen immediately and is written back to this file, so the next launch
starts the same way. A session started without a configuration file still
applies the change, but has nowhere to write it.

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

Effects other terminal agents ship are left out on purpose. Each was considered
and declined, not forgotten:

- **A rotating verb heading** ("thinking", "cooking", "noodling" on a 3.5 s
  cycle). The busy row says `Working` and does not change its word. A rotating
  label is the first thing a screen reader announces on every wake, and the row
  is a contract: `Working` means working.
- **A per-cell composer animation** (a drifting starfield or travelling
  highlight in the input area). It redraws the region the person is typing in,
  which is the one place in the app where motion actively costs legibility, and
  it defeats the reduced-motion switch because the effect *is* the content.
- **Collapsible activity sections.** The transcript is append-only by design
  (a cancelled turn keeps its output); collapsing needs a height model and a
  key, and is planned with the task list rather than before it.
- **A live task checklist.** It needs a model-callable plan tool, which the
  tool registry does not have yet.

The rule behind all of them: motion may show that work is happening, and may
orient someone in the window. It may not decorate the thing they are reading.

---

Source: [`docs/niki/UI.md`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/docs/niki/UI.md) in the Niki Agent repository.
