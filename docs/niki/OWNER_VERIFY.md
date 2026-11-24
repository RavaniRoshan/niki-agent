# Owner Verify

Real-terminal checks requiring owner verification.

## ANSI color fallback

- Launch `NIKI_COLOR_DEPTH=16 niki` in a terminal configured for 16 ANSI colors.
- Confirm the default dark theme keeps body text, muted labels, borders, and tool/status colors legible against that terminal's ANSI palette.
- Run `/theme niki-light` and confirm the light ANSI palette remains readable.
- In a terminal with truecolor support, launch without `NIKI_COLOR_DEPTH=16` and confirm the measured RGB palette is active.

## Command palette and turn summary

- Press `Ctrl+Shift+P`, filter a command, move the selection with Up/Down, press Enter, and confirm the selected slash command follows its normal dispatch behavior.
- Open the palette again and press Escape; confirm it closes and focus returns to the composer without changing the prompt.
- Complete, fail, and cancel one Niki runtime turn; confirm each terminal outcome produces one summary with elapsed time and event-backed tool/file counts, including a turn that pauses for approval and resumes.
