# Niki Agent -- Keymap

Generated from the live binding registry by
`libs/code/deepagents_code/niki/keymap.py`. **Do not edit by hand.**

Regenerate with:

```bash
cd libs/code && uv run python scripts/gen_keymap.py
```

Every row below exists because a mounted widget class actually binds it.
`test_every_advertised_action_has_a_handler` fails the build if a row has no
`action_*` handler to dispatch to, and Niki's footer hints and Help render from
this same registry -- so no surface can advertise a dead key.

| Key | Action | Description | Source |
| --- | --- | --- | --- |
| `1` | `approval_position(0)` | Select first | `NikiApp` |
| `2` | `approval_position(1)` | Select second | `NikiApp` |
| `3` | `approval_position(2)` | Select third | `NikiApp` |
| `a` | `approval_auto` | Auto | `NikiApp` |
| `ctrl+backslash` | `toggle_debug_console` | Debug Console | `NikiApp` |
| `ctrl+backspace,alt+backspace` | `delete_word_left` | Delete left to start of word | `ChatTextArea` |
| `ctrl+c` | `quit_or_interrupt` | Quit/Interrupt | `NikiApp` |
| `ctrl+c,super+c` | `screen.copy_text` | Copy selected text | `_MainScreen` |
| `ctrl+d` | `quit_app` | Quit | `NikiApp` |
| `ctrl+g` | `open_editor` | Open Editor | `NikiApp` |
| `ctrl+n` | `open_notifications` | Notifications | `NikiApp` |
| `ctrl+o` | `toggle_tool_output` | Toggle Tool Output | `NikiApp` |
| `ctrl+pagedown` | `page_right` | Page Right | `VerticalScroll` |
| `ctrl+pageup` | `page_left` | Page Left | `VerticalScroll` |
| `ctrl+r` | `open_prompt_clipboard` | Prompt Clipboard | `NikiApp` |
| `ctrl+t` | `toggle_subagent_panel` | Toggle Subagents | `NikiApp` |
| `down` | `approval_down` | Down | `NikiApp` |
| `down` | `scroll_down` | Scroll Down | `VerticalScroll` |
| `end` | `scroll_end` | Scroll End | `VerticalScroll` |
| `enter` | `approval_select` | Select | `NikiApp` |
| `escape` | `abandon_search` | Cancel | `PromptSearchInput` |
| `escape` | `interrupt` | Interrupt | `NikiApp` |
| `home` | `scroll_home` | Scroll Home | `VerticalScroll` |
| `j` | `approval_down` | Down | `NikiApp` |
| `k` | `approval_up` | Up | `NikiApp` |
| `left` | `scroll_left` | Scroll Left | `VerticalScroll` |
| `n` | `approval_no` | No | `NikiApp` |
| `pagedown` | `page_down` | Page Down | `VerticalScroll` |
| `pageup` | `page_up` | Page Up | `VerticalScroll` |
| `right` | `scroll_right` | Scroll Right | `VerticalScroll` |
| `shift+enter,alt+enter,ctrl+enter,ctrl+j` | `insert_newline` | New Line | `ChatTextArea` |
| `shift+tab` | `app.focus_previous` | Focus Previous | `_MainScreen` |
| `shift+tab` | `toggle_auto_approve` | Toggle Approval Mode | `NikiApp` |
| `tab` | `app.focus_next` | Focus Next | `_MainScreen` |
| `tab` | `approval_reject_with_reason` | Reject with feedback | `NikiApp` |
| `up` | `approval_up` | Up | `NikiApp` |
| `up` | `scroll_up` | Scroll Up | `VerticalScroll` |
| `y` | `approval_yes` | Yes | `NikiApp` |
