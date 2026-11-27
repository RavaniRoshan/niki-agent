<!--
  Mirrored from docs/niki/KEYMAP.md by docs-site/scripts/sync-docs.mjs.
  Do not edit this copy: edit the source file and run `npm run sync`.
  source-sha256: ce778f448609ad3b9f0356710d48af4cd69e723b4d5472be8bf1342bdb71a090
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
| `Enter` | Send the prompt. |
| `Alt+Enter` or `Shift+Enter` | Insert a newline instead of sending. |
| `Esc` | Clear a non-empty draft. With an empty draft, stop the running turn. |
| `Ctrl+C` | Close an open dialog and stop the running turn. It does not leave the app. |
| `Ctrl+D` | Leave the app. |
| `Shift+Tab` | Cycle Manual, Auto and YOLO. |
| `?` | Open the key list, when the composer is empty. |
| `PageUp` / `PageDown` | Scroll the transcript one page. |
| `Up` / `Down` | Move the cursor between lines in the composer. |
| `Home` / `End` | Move the cursor to the start or end of its line. |
| `Backspace` / `Delete` | Delete the character before or after the cursor. |
| Mouse wheel | Scroll the transcript. |

A modal is answered before the app keys: while a dialog is open, every key goes
to it except `Ctrl+C` (close it and cancel the turn) and `Ctrl+D` (leave the
app).

## Composer

| Key | What it does |
| --- | --- |
| `Enter` | Send the prompt. |
| `Alt+Enter`, `Shift+Enter` | Insert a newline. |
| `Left` / `Right` | Move the cursor one character. |
| `Up` / `Down` | Move the cursor between lines. |
| `Home` / `End` | Move the cursor to the start or end of its line. |
| `Backspace` / `Delete` | Delete before or after the cursor. |
| `Esc` | Clear the draft first; a second press stops the running turn. |

Bracketed paste is enabled, so a multi-line paste arrives as one paste and is
inserted literally. A paste never sends the prompt. A line that starts with `/`
is a slash command; it is submitted with `Enter` like any other line.

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
| `/clear` | Start a fresh session and empty the view. |
| `/compact` | Fold older rows out of the view; the session store keeps them. |
| `/help` | This list. `/keys` is an alias. |
| `/model` | Choose a model, or name one with `/model provider:model`. |
| `/quit` | Leave the app. `/exit` is an alias. |
| `/resume` | List saved sessions, or resume one by id or one-based index. |
| `/skills` | List the skills discovery found. |
| `/theme` | Choose a palette, or cycle to the next one. |
| `/tools` | List the registered tools. |
| `/version` | Show the version. |

`/compact` does not summarise: this build has no summarising model, so it only
hides older rows. The session store keeps the full transcript.

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
