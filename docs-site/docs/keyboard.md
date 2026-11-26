<!--
  Mirrored from docs/niki/KEYMAP.md by docs-site/scripts/sync-docs.mjs.
  Do not edit this copy: edit the source file and run `npm run sync`.
  source-sha256: c2b6b28cf458f8c09de69f80108f7fbba51869dd2590e51d55e33f5c395766e1
-->
# Niki Agent -- Keymap

The keymap is the `BINDINGS` table in
`rust/crates/niki-tui/src/lib.rs`; the help overlay draws the same table from
`rust/crates/niki-tui-modals/src/help.rs`. A binding that is listed here and not
acted on by the full-screen app is marked **not in this build** rather than
described as working.

## Ownership

The composer and the app claim different keys:

- The composer claims characters, arrows, Home and End, Enter (send) and
  Alt+Enter (newline).
- The app claims Ctrl-C, Ctrl-D, PageUp and PageDown, Shift+Tab, and the wheel.
- A modal claims every key while it is open, except Ctrl-C and Ctrl-D.

## App bindings

| Key | Action | What it does |
| --- | --- | --- |
| `ctrl+c` | `QuitOrInterrupt` | Stop the running turn; leaves when nothing is running |
| `ctrl+d` | `Quit` | Leave the app |
| `escape` | `Interrupt` | Clear the draft, then stop the running turn |
| `ctrl+shift+p` | `CommandPalette` | Start a slash command in the composer |
| `?` | `Help` | This list |
| `shift+tab` | `ToggleAutoApprove` | Cycle manual, auto and yolo |
| `ctrl+o` | `ToggleToolOutput` | Show or hide tool output (**not in this build**) |
| `ctrl+t` | `ToggleSubagents` | Show or hide subagent activity (**not in this build**) |
| `ctrl+n` | `OpenNotifications` | Show notifications (**not in this build**) |
| `ctrl+g` | `OpenEditor` | Edit the prompt in your editor (**not in this build**) |
| `up` | `ScrollUp` | Move the cursor up a line in the composer |
| `down` | `ScrollDown` | Move the cursor down a line in the composer |
| `pageup` | `PageUp` | Move the transcript back one page |
| `pagedown` | `PageDown` | Move the transcript forward one page |
| `home` | `ScrollHome` | Move the cursor to the start of its line |
| `end` | `ScrollEnd` | Move the cursor to the end of its line |
| `y` | `ApprovalYes` | Allow the command that is waiting |
| `n` | `ApprovalNo` | Refuse the command that is waiting |

The four keys marked **not in this build** are in the registry and the help
overlay, but the live app does not act on them yet. Treat them as unbound until
the overlay stops saying so.

## Composer

| Key | What it does |
| --- | --- |
| `Enter` | Send the prompt. |
| `Alt+Enter` | Insert a newline instead of sending. |
| `Up` / `Down` | Move the cursor between lines. |
| `Home` / `End` | Move the cursor to the start or end of its line. |
| `Esc` | Clear the draft, then stop the running turn. |

Bracketed paste is enabled, so a multi-line paste arrives as one paste and is
inserted literally. A paste never sends the prompt.

## Slash commands

Type these in the composer. `/help` lists the same set.

| Command | What it does |
| --- | --- |
| `/clear` | Start a fresh session and empty the view |
| `/compact` | Fold older rows out of the view; the session store keeps them |
| `/help` | This list |
| `/model` | Choose a model, or name one with `/model provider:model` |
| `/quit` | Leave the app |
| `/resume` | List saved sessions, or resume one by id or index |
| `/skills` | List project and user skills |
| `/theme` | Choose a palette, or cycle to the next one |
| `/tools` | List the registered tools |
| `/version` | Show the version |

`/compact` does not summarise: this build has no summarising model, so it only
hides older rows. The session store keeps the full transcript.

## Themes

`/theme` cycles the four palettes; `/theme <name>` selects one. The names are
`niki`, `high-contrast`, `colorblind`, and `mono`; the aliases `dark`,
`default`, `light`, `colour-blind`, `monochrome`, `grey` and `gray` resolve to
the same palettes. `[ui].theme` sets the startup palette.

`NIKI_ASCII=1` draws the app with ASCII characters only.
`NIKI_REDUCED_MOTION=1` stops the spinner without changing the layout.

---

Source: [`docs/niki/KEYMAP.md`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/docs/niki/KEYMAP.md) in the Niki Agent repository.
