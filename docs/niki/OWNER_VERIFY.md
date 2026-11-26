# Owner Verify

Real-terminal checks that the app cannot settle by itself. Everything else is
decided by the test suite or a measured probe, and `CHECKLIST.md` names which.

## ANSI color fallback

- Launch `NIKI_COLOR_DEPTH=16 niki` in a terminal configured for 16 ANSI colors.
- Confirm the default dark theme keeps body text, muted labels, borders, and tool/status colors legible against that terminal's ANSI palette.
- Run `/theme niki-light` and confirm the light ANSI palette remains readable.
- In a terminal with truecolor support, launch without `NIKI_COLOR_DEPTH=16` and confirm the measured RGB palette is active.

## Clipboard and selection

- Select text with drag, press the copy binding, and confirm the clipboard
  receives it in your terminal. OSC 52 delivery is a terminal property: xterm,
  kitty, wezterm and iTerm2 honour it; some configurations disable it.
- Repeat over a real ssh hop if you use one; tmux users get the DCS-wrapped form
  automatically.
- Press `f2`, drag-select with the terminal's own renderer, then press `f2`
  again and confirm wheel clicks and app clicks behave as before.

## Appearance on your terminal

- Confirm the composer rule, the mode accent colour, and the incognito border
  title look as intended against your terminal's own background.
- Confirm reduced motion (`NIKI_REDUCED_MOTION=1`) stops the activity spinner.

## Evaluation credentials

- The matched Claude Code comparison needs `ANTHROPIC_API_KEY`,
  `LANGSMITH_API_KEY`, and a working container runtime. Recheck before running;
  the last check found all of them unavailable.
