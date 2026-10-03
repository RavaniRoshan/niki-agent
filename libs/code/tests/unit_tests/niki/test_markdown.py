"""V11: markdown rendering — what can and cannot be decided headlessly.

**This file is mostly a retraction.** An earlier draft mounted an
`AssistantMessage` with headings, lists, a table, a link, and a fenced code
block, then asserted they appeared on the rendered screen. Every one of those
assertions would have been reading an empty region.

Measured, not assumed:

- The widget mounts: `messages.children` grows by one.
- Its layout height stays **0** (`Region(x=1, y=5, width=78, height=0)`,
  `virtual_size.height == 0`), so it paints nothing.
- That is true through the app's own `app._mount_message(...)` path, not just a
  synthetic mount, and it does not improve with more event-loop time.

The markdown sub-widget is never measured because the transcript's height
measurement is scheduled by the agent/thread lifecycle, which a synthetic mount
does not stand up. So the *on-screen* half of V11 — headings, lists, tables,
links, code-block language labels, and stability while streaming — is
**OWNER-VERIFY**, with exact steps in `docs/niki/OWNER_VERIFY.md`.

What remains below is the part that is genuinely decidable: that the message
pipeline actually carries markdown source into a `Markdown` widget. That is
worth asserting, because it is the step that would silently break first if the
streaming path stopped feeding the renderer.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from textual.app import App
    from textual.pilot import Pilot

MARKDOWN = """# Heading One

- first bullet
- second bullet

| col a | col b |
| --- | --- |
| 1 | 2 |

```python
print("hello")
```

A [link](https://example.com) inline.
"""


def _app() -> App:  # type: ignore[type-arg]
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-md")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


async def test_v11_the_markdown_renderer_is_wired_into_the_message() -> None:
    """V11: the assistant message must carry a real Markdown renderer.

    Structural, not visual. Textual's `Markdown` keeps its parsed source in an
    internal block tree rather than a plain string attribute, so this asserts
    the renderer is present and correctly identified instead of guessing at
    where the source lives. That is the seam which breaks first if the streaming
    path stops feeding the renderer.
    """
    from textual.widgets import Markdown

    from deepagents_code.tui.widgets.messages import AssistantMessage

    app = _app()
    async with app.run_test(size=(80, 40)) as pilot:
        await pilot.pause()
        await app.query_one("#messages").mount(AssistantMessage(content=MARKDOWN))
        await pilot.pause()
        message = app.query_one(AssistantMessage)
        # Read inside the block: the tree is gone once run_test unwinds.
        renderer_ids = [w.id for w in message.query(Markdown)]

    assert renderer_ids, (
        "V11: the assistant message mounted no Markdown widget, so markdown "
        "content has nowhere to render"
    )
    assert "assistant-content" in renderer_ids, (
        f"V11: expected the renderer at #assistant-content, found {renderer_ids}"
    )


async def test_v11_a_streams_into_the_same_markdown_widget() -> None:
    """V11: streamed appends must feed one widget, not accumulate renderers."""
    import asyncio

    from textual.widgets import Markdown

    from deepagents_code.tui.widgets.messages import AssistantMessage

    app = _app()
    async with app.run_test(size=(80, 40)) as pilot:
        await pilot.pause()
        await app.query_one("#messages").mount(AssistantMessage(content=""))
        await pilot.pause()
        message = app.query_one(AssistantMessage)
        before = len(list(message.query(Markdown)))

        for _ in range(10):
            await message.append_content("streamed line\n")
            await asyncio.sleep(0.005)
        await pilot.pause()
        after = len(list(message.query(Markdown)))

    assert before == 1, f"V11: expected one Markdown widget, found {before}"
    assert after == before, (
        f"V11: streaming grew the Markdown widget count from {before} to "
        f"{after}; each append is creating a renderer"
    )


async def test_v11_markdown_widget_is_present_but_unmeasured_in_isolation() -> None:
    """Document the limitation so the next person does not retry this.

    If a future change makes synthetic mounts lay out, this test fails and the
    OWNER-VERIFY note in the checklist can be retired.
    """
    from deepagents_code.tui.widgets.messages import AssistantMessage

    app = _app()
    async with app.run_test(size=(80, 40)) as pilot:
        await pilot.pause()
        await app.query_one("#messages").mount(AssistantMessage(content=MARKDOWN))
        await pilot.pause()
        await pilot.pause()
        height = app.query_one(AssistantMessage).region.height

    assert height == 0, (
        f"V11: a synthetic assistant message now lays out (height={height}). If "
        "that is expected, the markdown rendering tests can be restored and the "
        "OWNER-VERIFY note in docs/niki/CHECKLIST.md retired in this change."
    )
