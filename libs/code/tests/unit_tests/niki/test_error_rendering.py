"""V14: errors appear inline, with a recovery action, never as a raw traceback.

Upstream's `ErrorMessage` already does the hard part: it renders
`Error: <content>` inside the transcript with a themed colour, and it accepts
`Content` so a caller can attach a **link-styled recovery action**. What was
missing is proof -- and Niki styling so the recovery affordance is actually
visible against Niki's palette.

Three properties are asserted:

1. **Inline.** The error is a transcript widget with an `Error:` lead, not a
   modal, a toast, or a dump on stderr.
2. **Recoverable.** When the caller attaches a link, the rendered content
   carries it, so the user has somewhere to go.
3. **Never a raw traceback.** A body that *is* a traceback renders as text the
   user can read, with no interactive traceback viewer and no terminal escape
   sequences -- a traceback pasted into a prompt is exactly how a terminal gets
   hijacked.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from textual.app import App
    from textual.pilot import Pilot

RECOVERY_URL = "https://example.invalid/docs/retry"


def _app() -> App:  # type: ignore[type-arg]
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-v14")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


def _rendered(app: App) -> str:  # type: ignore[type-arg]
    return "\n".join(
        strip.text
        for strip in app.screen._compositor.render_strips()
        if strip is not None
    )


async def test_v14_an_error_renders_inline_in_the_transcript() -> None:
    """V14: the error belongs in the conversation, not in a modal."""
    from deepagents_code.tui.widgets.messages import ErrorMessage

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await app.query_one("#messages").mount(ErrorMessage("Provider returned 429"))
        await pilot.pause()
        errors = list(app.query(ErrorMessage))
        in_transcript = bool(errors) and errors[0].parent is not None
        content = errors[0].render().plain if errors else ""

    assert errors, "the error widget did not mount"
    assert in_transcript, "the error was not mounted into the transcript"
    assert content.startswith("Error:"), (
        f"V14: the error has no 'Error:' lead, so it reads as ordinary output: "
        f"{content!r}"
    )
    assert "429" in content, f"V14: the error lost its message: {content!r}"


async def test_v14_an_error_carries_its_recovery_action() -> None:
    """V14: when a recovery exists, the user must be able to see and reach it."""
    from textual.content import Content

    from deepagents_code.tui.widgets.messages import ErrorMessage

    body = Content.assemble(
        "Rate limited.",
        Content.styled(" Retry", "link"),
    )
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await app.query_one("#messages").mount(ErrorMessage(body))
        await pilot.pause()
        rendered = next(iter(app.query(ErrorMessage))).render()

    assert rendered.plain.startswith("Error:"), "the error lost its lead"
    assert "Retry" in rendered.plain, "the recovery action is not visible"
    assert any("link" in str(span.style) for span in rendered.spans), (
        "the recovery text carries no link, so it is decoration rather than an "
        f"action: spans={[str(s.style) for s in rendered.spans]}"
    )


async def test_v14_a_traceback_body_never_becomes_an_escape_sequence() -> None:
    """V14: a traceback in a body is text, never terminal control.

    A traceback pasted into a prompt is the classic way to smuggle a clear-screen
    or a clipboard write into a terminal. This asserts the rendered output
    carries no ESC byte and that the body survives verbatim.
    """
    from deepagents_code.tui.widgets.messages import ErrorMessage

    traceback = (
        "Traceback (most recent call last):\n"
        '  File "agent.py", line 42, in run\n'
        "    raise RuntimeError('boom')\n"
        "RuntimeError: boom"
    )
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await app.query_one("#messages").mount(
            ErrorMessage(traceback + "\n\x1b]52;c;cGF5bG9hZA==\x07")
        )
        await pilot.pause()
        rendered = next(iter(app.query(ErrorMessage))).render().plain

    assert "\x1b" not in rendered, "an ESC byte survived into the rendered error"
    assert "RuntimeError: boom" in rendered, (
        f"V14: the readable part of the error was lost: {rendered!r}"
    )


async def test_v14_the_error_colour_is_themed_rather_than_hardcoded() -> None:
    """V14: the error must be coloured from the active theme.

    Asserted by *changing* the theme rather than by matching a literal. Two
    earlier versions of this test compared the rendered hex against Niki's
    `error` token and failed: Textual derives `$error` through its own scale, so
    the value that reaches a span is not the token's value, and guessing at that
    plumbing produced a test coupled to internals it was not about.

    What matters is the property the brief states -- colours come from theme
    tokens -- and that is exactly what a two-theme comparison proves.
    """
    from deepagents_code.tui.widgets.messages import ErrorMessage

    async def rule_under(theme_name: str) -> str:
        app = _app()
        async with app.run_test(size=(80, 24)) as pilot:
            await pilot.pause()
            app.theme = theme_name
            await pilot.pause()
            await app.query_one("#messages").mount(ErrorMessage("nope"))
            await pilot.pause()
            widget = next(iter(app.query(ErrorMessage)))
            return str(widget.styles.border_left[1])

    default_rule = await rule_under("niki")
    contrast_rule = await rule_under("niki-contrast")

    assert default_rule, "the error rendered with no border colour at all"
    assert default_rule.lower() != contrast_rule.lower(), (
        "the error rule is identical under both themes "
        f"({default_rule}); it is hardcoded rather than themed"
    )
