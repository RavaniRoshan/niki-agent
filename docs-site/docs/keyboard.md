<!--
  Mirrored from docs/niki/KEYMAP.md by docs-site/scripts/sync-docs.mjs.
  Do not edit this copy: edit the source file and run `npm run sync`.
  source-sha256: d99a720852731b3e1bcd578aad780d829d063b177bad968e6f45366088149a64
-->
# Niki Agent -- Keymap

These are the keys the shipped full-screen app acts on. The table is maintained
by hand from the key handler in `rust/crates/niki-tui-live/src/app.rs`; a key
that appears in the help overlay but does nothing in the app is listed at the
end rather than described as working.

## App keys

These apply when no dialog is open.

| Key | Action |
| --- | --- |
| `Enter` | Send the prompt. While a turn runs, queue it for when that turn ends. |
| `Alt+Enter` | Insert a newline. While a turn runs, steer that turn with the draft instead. |
| `Ctrl+Enter` | Steer the running turn, when the terminal reports the chord. |
| `Shift+Enter` | Insert a newline instead of sending. |
| `Tab` | Put the highlighted slash command in the draft, when the command list is open. |
| `Esc` | Close the command list first; then clear a non-empty draft; with an empty draft, stop the running turn. A second `Esc` within two seconds of the first restores the last prompt. |
| `Ctrl+C` | Close an open dialog and stop the running turn. It does not leave the app. |
| `Ctrl+D` | Leave the app. |
| `Ctrl+Y` | Copy the newest answer to the terminal clipboard (OSC 52). `NIKI_NO_CLIPBOARD=1` turns it off. |
| `Shift+Tab` | Cycle Manual, Auto and YOLO. |
| `?` | Open the key list, when the composer is empty. |
| `PageUp` / `PageDown` | Scroll the transcript one page. |
| `Up` / `Down` | Move the command-list highlight while it is open, recall older prompts in an empty composer, and otherwise move the cursor between lines. |
| `Home` / `End` | Move the cursor to the start or end of its line. |
| `Backspace` / `Delete` | Delete the character before or after the cursor. |
| Mouse wheel | Scroll the transcript. |

A modal is answered before the app keys: while a dialog is open, every key goes
to it except `Ctrl+C` (close it and cancel the turn) and `Ctrl+D` (leave the
app).

## Composer

| Key | What it does |
| --- | --- |
| `Enter` | Send the prompt. While a turn runs, queue it for when that turn ends. |
| `Alt+Enter` | Insert a newline. While a turn runs, steer that turn with the draft instead. |
| `Ctrl+Enter` | Steer the running turn, when the terminal reports the chord. |
| `Shift+Enter` | Insert a newline. |
| `Left` / `Right` | Move the cursor one character. |
| `Up` / `Down` | Move the cursor between lines, or recall an earlier prompt when the composer is empty. |
| `Home` / `End` | Move the cursor to the start or end of its line. |
| `Backspace` / `Delete` | Delete before or after the cursor. |
| `Esc` | Close the command list if it is open; otherwise clear the draft first; a further press stops the running turn. A second `Esc` within two seconds of the first, with an empty composer, restores the last prompt. |
| `Ctrl+Y` | Copy the newest answer. |

Bracketed paste is enabled, so a multi-line paste arrives as one paste and is
inserted literally. A paste never sends the prompt. A line that starts with `/`
is a slash command; it is submitted with `Enter` like any other line.

While a turn runs, `Enter` does not send: it queues the prompt. Each queued
prompt is drawn under the transcript in a dim `queued:` row, the status bar
counts them (`queued 2`), and the first one is sent when the turn ends. A
cancelled turn (`Esc`, `Ctrl+C`) and `/clear` or `/quit` discard the queue.

`Alt+Enter` while a turn runs hands the draft to that turn instead of queueing
it: the model reads it on its next request, inside the same turn, so no second
turn starts. The line is shown in the transcript as a prompt.

`Esc` `Esc` on an empty composer restores the last prompt to the composer and
drops it, with the answer it got, from the view and from the model's history.
The saved session is left alone, so `/resume` still shows that turn.

A line that starts with `!` runs locally and never reaches the model. The
command goes through the same policy a `shell` tool call does: YOLO runs it,
Auto runs it when the shell allow-list permits, and Manual asks first. A line
with nothing after the `!` does nothing.

Accepted prompts are appended to `$NIKI_HOME/history.jsonl` (falling back to
`$HOME/.niki`), one JSON string per line; slash commands and blank prompts are
not recorded, and a line that no longer parses is skipped. `Up` in an empty
composer walks the file backwards, `Down` returns toward the draft.

## Approval prompt

The prompt appears when the policy asks about a shell command or a gated MCP
tool.

| Key | Answer |
| --- | --- |
| `y` | Allow this once. |
| `a` | Always allow: the exact command or the tool name is recorded for the rest of the session. |
| `n` | Refuse. |
| `Enter` | Choose the highlighted option. The first is "Allow this once". |
| `Esc` | Refuse. There is no way to leave the prompt unanswered. |
| `Up` / `Left` / `Shift+Tab` | Move the highlight up. |
| `Down` / `Right` / `Tab` | Move the highlight down. |

Closing the prompt without an answer counts as a refusal, so the turn never
waits on a question nobody answered.

## Help overlay

Opened with `?` on an empty composer, or with `/help`.

| Key | What it does |
| --- | --- |
| `Esc`, `?`, `q` | Close the overlay. |
| `Up` / `Down` | Scroll one line. |
| `PageUp` / `PageDown` | Scroll ten lines. |

## Model picker

Opened with `/model`.

| Key | What it does |
| --- | --- |
| `Up`, `k` | Move the highlight up. |
| `Down`, `j` | Move the highlight down. |
| `Home` / `End` | Jump to the first or last model. |
| `Enter` | Switch to the highlighted model. |
| `Esc` | Close without changing the model. |

## Session browser

Opened with `/threads`.

| Key | What it does |
| --- | --- |
| `Up`, `k` | Move the highlight up. |
| `Down`, `j` | Move the highlight down. |
| `Home` / `End` | Jump to the first or last session. |
| `Backspace` | Remove the last filter character. |
| `Enter` | Resume the highlighted session, as `/resume <id>` does. |
| `Esc` | Close without changing anything. |

Typing filters the list case-insensitively by preview, cwd or id. Because `j`
and `k` move the highlight, type `J` or `K` to filter for those letters. A
session with no prompt previews as `(no prompt)`.

## Keys the help overlay lists that do nothing

The overlay is drawn from a registry that still carries five bindings the app
does not act on. They are not shortcuts in this build:

- `Ctrl+Shift+P` (it would start a slash command)
- `Ctrl+O` (tool output)
- `Ctrl+T` (subagent activity)
- `Ctrl+N` (notifications)
- `Ctrl+G` (open an editor)

Those actions are unbuilt; the overlay marks them as such.

## Slash commands

Type these in the composer. `/help` lists the same set.

| Command | What it does |
| --- | --- |
| `/auto` | Switch to Auto approval mode without restarting. |
| `/clear` | Start a fresh session and empty the view. |
| `/compact` | Fold older rows out of the view; the session store keeps them. |
| `/context` | Show the conversation's size and the session's token total. |
| `/diff` | Show the working tree's staged, unstaged and untracked changes; `/diff staged`, `/diff unstaged`, `/diff untracked` narrow it. |
| `/export` | Write the session to `niki-<id>.md` in the workspace, or to a path you give. |
| `/help` | This list. `/keys` is an alias. |
| `/init` | Write a starter `AGENTS.md` when the workspace has none. |
| `/manual` | Switch to Manual approval mode without restarting. |
| `/model` | Choose a model, or name one with `/model provider:model`. |
| `/quit` | Leave the app. `/exit` is an alias. |
| `/rename` | Name the current session; with no argument, report the current name. |
| `/resume` | List saved sessions, or resume one by id or one-based index. |
| `/skills` | List the skills discovery found. |
| `/theme` | Choose a palette, or cycle to the next one. |
| `/threads` | Browse saved sessions and resume the one you choose. |
| `/tools` | List the registered tools. |
| `/version` | Show the version. |
| `/yolo` | Switch to YOLO approval mode without restarting. |

`/compact` asks the model to fold older turns into a summary, then keeps the
newest rows in the view. The session store keeps the original rows, so a
`/resume` shows the uncompressed record.

`/resume` redraws the selected session on screen. This build does not replay
the resumed session into the model's context.

## Themes

`/theme` cycles the four palettes; `/theme <name>` selects one. The names are
`niki`, `high-contrast`, `colorblind`, and `mono`; the aliases `dark`,
`default`, `light`, `colour-blind`, `monochrome`, `grey` and `gray` resolve to
the same palettes. `[ui].theme` sets the startup palette.

`NIKI_ASCII=1` draws the app with ASCII characters only.
`NIKI_REDUCED_MOTION=1` stops the spinner without changing the layout.

---

Source: [`docs/niki/KEYMAP.md`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/docs/niki/KEYMAP.md) in the Niki Agent repository.
