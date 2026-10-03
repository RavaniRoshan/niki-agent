# Niki Agent — Owner Verification

Everything below is **UNVERIFIED** and needs a human at a real terminal. Every
number in this repo came from Textual's headless driver, which deliberately
skips terminal modes, escape handling, mouse protocols, and anything involving
judgement about how the thing feels. I cannot claim any of it "looks as good as
Codex or Claude Code" — that call is yours.

## 0. Install and run

```bash
cd libs/code
uv sync --all-groups
uv run niki                      # Niki Agent
uv run niki --version            # expect: Niki Agent 0.1.80, built on Deep Agents
uv run dcode --version           # expect: deepagents-code 0.1.80  (must be unchanged)
```

## 1. First-run appearance (needs a real model key)

Start with no session: `uv run niki`.

- [ ] Header reads **Niki Agent**, not "Deep Agents".
- [ ] Colours are the Niki palette (warm-neutral dark, periwinkle accent
      `#8FA3F5`). Run `/theme` — `niki` and `niki-light` must both be listed.
- [ ] **KNOWN LEAK:** the welcome box still shows `dcode` instead of `niki`
      (`welcome.py:471`). Confirm it is visible; it is checklist row V2 and is
      currently not met.
- [ ] Empty-state hint reads "Type a request · / for commands · ? for help".

## 2. Colour depth and background modes

For each terminal below, confirm the UI is legible and the accent still reads as
one colour:

- [ ] **Kitty** — truecolor. `?` help, approvals, and a diff all render.
- [ ] **WezTerm** — truecolor, then `TERM=xterm-256color` to force 256.
- [ ] **iTerm2** — truecolor.
- [ ] **GNOME Terminal** — 256 by default.
- [ ] **Windows Terminal** — if the fork is expected to run there at all.
- [ ] `TERM=dumb` — expect a clear message or the upstream headless path, **not**
      a screen full of escapes.
- [ ] `NO_COLOR=1 uv run niki` — no colour, layout unchanged, still readable.
- [ ] **Light terminal** — switch to `niki-light` via `/theme`. The light palette
      is contrast-measured (tightest pair 4.12:1 against a 3:1 glyph floor) but
      has never been looked at by a human.
- [ ] **ASCII fallback** — run in a terminal that reports no Unicode. Every
      structural glyph must degrade. Upstream's `get_glyphs()` supplies the
      ASCII set; verify Niki's own chrome uses it rather than hardcoding `✓`.

## 3. Mouse

- [ ] Wheel scrolls the transcript.
- [ ] Scrolling up mid-stream is **not** snapped back to the bottom.
- [ ] A "new activity below" indicator appears; `End` or a click jumps back.
- [ ] Click focuses the composer; tool cards expand/collapse; approval rows and
      slash rows are clickable.
- [ ] Hover highlights rows and buttons **without** a visible redraw storm.
- [ ] **tmux:** mouse must work inside tmux (needs `set -g mouse on`). Report
      any region where clicks land offset — this is a known tmux class of bug and
      the headless driver cannot see it.

## 4. Keyboard feel

- [ ] Type latency while a stream is running. Aim: no perceptible lag.
- [ ] Resize feel: drag the terminal edge slowly. No flicker, no stale
      artifacts, correct final layout. Then run a **100-resize storm** — resize
      aggressively for 2 s and confirm it settles correctly (checklist S7 is
      PARTIAL: only 3 resizes are automated).
- [ ] `esc` interrupts a running task.
- [ ] `ctrl+c` clears input, else interrupts, else arms exit with a footer hint.
- [ ] `ctrl+d` exits only on empty input.
- [ ] `/` opens the filtered slash menu and never blocks typing.
- [ ] `?` shows help.
- [ ] Cross-check every advertised key against `docs/niki/KEYMAP.md`. **Help is
      still upstream's hand-maintained screen**, so this is the one place the
      registry and the UI can disagree (checklist K1 is PARTIAL).

## 5. OSC 52 and selection over ssh

- [ ] Copying a code block or the last response over ssh (e.g.
      `ssh -R` / tmux passthrough) — confirm whether OSC 52 is needed and works.
- [ ] Text selection with the mouse: does Niki capture the mouse? If so, is there
      a toggle to release it for native terminal selection? Upstream binds
      `ctrl+c` to copy; verify that path.
- [ ] Document any ssh/tmux limitation you hit.

## 6. Terminal restoration

For each of normal exit, `ctrl+c`, `SIGTERM`, `SIGHUP`, and `kill -9`:

- [ ] After exit, `stty sane` reports sane settings and echo works.
- [ ] No leftover escape sequences in the scrollback.

Checklist L1 is PARTIAL: clean exit and SIGTERM delivery are automated; SIGHUP
and the crash path are not.

## 7. Long-session stability

- [ ] Scroll through a 5,000-message transcript. Watch for stutter.
      **S5 is PARTIAL**: RSS dropped 395.8 → 254.9 MB, but scroll p95 was never
      measured and the mount-window cap (150) trades scroll-back depth for
      responsiveness. This is the taste call most worth your judgement — does it
      scroll as well as upstream's 800?
- [ ] A tool returning **10 MB** of output stays bounded in the UI with the full
      output on disk (checklist S8 is MISSING entirely).
- [ ] A tool that blocks for 3 s does not freeze keys, scrolling, or resize
      (S6 MISSING).

## 8. Real-model smoke test — needs your API key

Only run this with an explicit key. Do not commit it.

```bash
export ANTHROPIC_API_KEY=...   # or whichever provider
uv run niki
```

- [ ] Ask it to read a file; confirm tool cards render with status and elapsed.
- [ ] Ask for a diff-sized edit; confirm the approval prompt shows action,
      command/tool, scope, risk, and choices — **safest option focused**, `Esc`
      denies, outside clicks do not dismiss.
- [ ] Ask for markdown with a table, a list, a heading, and a fenced code block.
- [ ] Force an error (bad path / rate limit) and confirm it appears inline with a
      recovery action, **never as a raw traceback**.
- [ ] Paste untrusted text containing `[bold]`, `<b>x</b>`, and an OSC title-change
      sequence. None may alter rendering (checklist L4 is MISSING and unverified
      in this fork).

## Taste questions for the owner

1. **Mount window 150 vs upstream's 800.** Chosen to flatten render cost
   (2.089x → 1.205x). Does far-back scrolling feel worse, and is that a trade
   you accept?
2. **Cursor blink off by default.** It is what takes idle repaints from 10 to 0.
   Is a static cursor the right default for you?
3. **Accent `#8FA3F5` periwinkle.** Chosen to be neither orange nor
   terminal-green. Does it read as calm, or too blue?
4. **`muted` at 5.56:1** (dark, on panel) — plenty of margin, or too bright for
   secondary text?
5. **The `dcode` leak in the welcome box.** Fixing it means overriding a large
   private `_build_banner`; the alternative is a small upstream patch. Which do
   you prefer?
6. **Auto-update is off by default** for Niki (upstream: on). Implemented as the
   mission mandates and strictly less risky, but it is a default change and
   wants your explicit sign-off.