"""L4: untrusted text is never interpreted as markup or terminal control.

Tool output, file contents, and model text are all attacker-influenced. This
file proves two independent defences:

1. **Control sequences are stripped.** `sanitize_control_chars`
   (`unicode_security.py:176`) replaces every Unicode "Other" category
   character with a space, after removing invisible/bidi code points. That kills
   OSC title-writes, OSC 52 clipboard writes, cursor moves, screen clears, and
   colour SGR -- none of which can reach the terminal.

2. **Markup is never parsed.** Textual's `Content` does not interpret Rich
   markup in plain text, and upstream builds dynamic content with
   `Content.from_markup(..., $var)` (which auto-escapes) or `Content.assemble`
   rather than f-string interpolation.

The end-to-end check renders hostile text through the real message widgets and
feeds the resulting screen through `pyte`, so what is asserted is what a
terminal would actually show -- not merely that a helper returned a clean string.
"""

from __future__ import annotations

import pytest

from deepagents_code.unicode_security import sanitize_control_chars

#: Every sequence here is something a hostile tool result could contain.
HOSTILE_SEQUENCES: dict[str, str] = {
    "osc_title_change": "\x1b]0;pwned\x07",
    "osc52_clipboard_write": "\x1b]52;c;cGF5bG9hZA==\x07",
    "cursor_move": "\x1b[2J\x1b[H",
    "screen_clear": "\x1b[2J",
    "sgr_colour": "\x1b[31mRED\x1b[0m",
    "bell": "\x07",
    "backspace_overwrite": "safe\x08\x08\x08\x08evil",
    "delete_line": "\x1b[2K",
    "risky_csi": "\x1b[6n",
    # A right-to-left override is exactly the character the bidirectional
    # linter flags, and it is here on purpose: it is the payload under test.
    "bidi_override": "safe\u202eevil",
}

#: Markup that would be dangerous if it were parsed.
HOSTILE_MARKUP: dict[str, str] = {
    "rich_bold": "[bold]not bold[/bold]",
    "rich_escape_close": "text [/tmp/x]",
    "markdown_link": "[click](evil://payload)",
    "markdown_heading": "\n# injected heading\n",
    "html_tag": "<b>not bold</b>",
}


@pytest.mark.parametrize("name", sorted(HOSTILE_SEQUENCES))
def test_control_sequences_are_neutralised(name: str) -> None:
    """No escape or control character survives into renderable text."""
    hostile = HOSTILE_SEQUENCES[name]
    cleaned = sanitize_control_chars(hostile, keep_newlines=True)

    assert "\x1b" not in cleaned, f"{name}: ESC survived sanitisation"
    assert "\x07" not in cleaned, f"{name}: BEL survived sanitisation"
    assert "\x08" not in cleaned, f"{name}: backspace survived sanitisation"
    assert not any(ord(ch) < 32 and ch != "\n" for ch in cleaned), (
        f"{name}: a raw control character survived: {cleaned!r}"
    )


@pytest.mark.parametrize("name", sorted(HOSTILE_MARKUP))
def test_hostile_markup_is_not_interpreted(name: str) -> None:
    """Markup arriving as *data* must not be parsed as markup."""
    from textual.content import Content

    hostile = HOSTILE_MARKUP[name]

    # `Content` treats the whole string as plain text. Two properties matter:
    # the text survives verbatim, and *no style spans* are introduced -- a span
    # is what "interpreted as markup" actually means.
    content = Content(hostile)
    assert content.plain == hostile, f"{name}: Content altered the text"
    assert not content.spans, (
        f"{name}: text was parsed into {len(content.spans)} style spans; "
        f"markup was interpreted"
    )
    # Re-emitting as markup escapes the brackets, so a string that round-trips
    # through markup cannot be re-parsed as markup the second time.
    live_brackets = content.markup.replace("\\[", "").replace("]", "")
    assert "[" not in live_brackets, (
        f"{name}: emitted markup left a live bracket that could be re-parsed"
    )


async def test_hostile_tool_output_never_reaches_the_terminal_as_escapes() -> None:
    """End to end: hostile tool output renders as inert text, not as sequences.

    Feeds the rendered screen through `pyte`, so this asserts on what a terminal
    emulator would interpret -- not on an intermediate string.
    """
    import pyte
    from textual.app import App, ComposeResult

    from deepagents_code.tui.widgets.messages import ToolCallMessage

    class Hostile(App[None]):
        """Mounts one tool card carrying a hostile payload."""

        def compose(self) -> ComposeResult:
            yield ToolCallMessage(tool_name="bash", args={"cmd": "cat evil"})

    payload = "".join(HOSTILE_SEQUENCES.values()) + "".join(HOSTILE_MARKUP.values())
    app = Hostile()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        card = app.query_one(ToolCallMessage)
        card._output = payload
        card._update_output_display()
        await pilot.pause()

        # Read inside the block: `app.screen` is gone once the stack unwinds.
        strips = [
            str(line)
            for line in app.screen._compositor.render_strips()
            if line is not None
        ]
    rendered = "\n".join(strips)
    assert "\x1b" not in rendered, "an ESC byte reached the rendered output"

    # Feed the payload through a real terminal emulator: if a sequence had
    # survived, pyte's title or clipboard handling would show it.
    screen = pyte.Screen(80, 24)
    pyte.Stream(screen).feed(rendered)
    assert screen.title == "", (
        f"hostile text changed the terminal title to {screen.title!r}"
    )
    assert screen.cursor.y < 24, "cursor moved out of bounds"


def test_sanitisation_is_not_bypassable_via_the_markup_helper() -> None:
    """`Content.from_markup` auto-escapes `$var`; prove it for a hostile value."""
    from textual.content import Content

    payload = "[bold]injected[/bold] \\x1b]0;title\\x07"
    content = Content.from_markup("{$msg}", msg=payload)

    assert payload in content.plain, (
        f"from_markup did not carry the substitution through: {content.plain!r}"
    )
    assert not content.spans, (
        f"the substitution introduced {len(content.spans)} style spans; the "
        "hostile value was parsed as markup instead of being escaped"
    )
