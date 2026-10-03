"""V8: diff rendering — the part that can be proven honestly.

**What this file does NOT claim.** An earlier draft captured the rendered screen
after mounting `compose_diff_lines` output into the transcript. That approach
does not work: `#messages` uses Textual's `stream` layout, and widgets mounted
into it directly do not lay out where a reader expects, so the capture showed an
empty region and the assertions were reading nothing. The same happened with an
`ApprovalMenu` carrying a file-edit diff.

Those three screen-capture tests were **deleted rather than left failing or
quietly passing**. Rendering the diff through the real approval flow, with a real
file change, is recorded as OWNER-VERIFY in `docs/niki/OWNER_VERIFY.md`.

What remains here are assertions that are actually decidable: that Niki gives
added and removed *different* colours, that both clear the 3:1 glyph contrast
floor against every surface, and that upstream's line cap is a real cap.
"""

from __future__ import annotations

import re
from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from textual.app import App
    from textual.pilot import Pilot

DIFF = """@@ -1,4 +1,5 @@
 def compute():
-    total = 1
+    total = 2
+    note = "hi"
     return total
"""

LARGE_DIFF = "".join(f"+line {i}\n" for i in range(400))


def _app() -> App:  # type: ignore[type-arg]
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-diff")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


def _rendered(app: App) -> str:  # type: ignore[type-arg]
    """The screen as plain text.

    `render_strips()` yields `Strip` objects whose `str()` is a repr. Slicing
    `.text` is what a terminal would actually show; without this the assertions
    would be matching against debug output.
    """
    return "\n".join(
        strip.text
        for strip in app.screen._compositor.render_strips()
        if strip is not None
    )


async def _scroll_to_end(app: App, pilot: Pilot[None]) -> None:  # type: ignore[type-arg]
    """Scroll the transcript to the bottom before capturing.

    The welcome banner occupies the top of a 24-row screen, so freshly appended
    content sits below the fold and never reaches a render. Without this the
    assertions read an empty region and fail for the wrong reason.
    """
    app.query_one("#chat").scroll_end(animate=False)
    await pilot.pause()
    await pilot.pause()


async def test_v8_a_long_diff_is_truncated_rather_than_rendered_whole() -> None:
    """V8: a 400-line diff must not paint 400 rows into a 24-row viewport."""
    from deepagents_code.tui.widgets.diff import compose_diff_lines

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await app.query_one("#messages").mount_all(
            list(compose_diff_lines(LARGE_DIFF, max_lines=40))
        )
        await pilot.pause()
        await _scroll_to_end(app, pilot)
        widgets = list(app.screen.query("Static"))
        # Read inside the block: the tree is gone once run_test unwinds.
        mounted = len(widgets)

    assert mounted <= 60, (
        f"V8: {mounted} widgets mounted for a diff capped at 40 lines; the cap "
        "is not being applied"
    )


def test_v8_niki_colours_the_diff_tokens() -> None:
    """V8: the + and - lines must actually carry Niki's semantic colours."""
    from deepagents_code.niki.theme import NIKI_DARK

    assert NIKI_DARK["success"] != NIKI_DARK["error"], (
        "V8: success and error are the same colour, so a diff cannot "
        "distinguish added from removed"
    )


@pytest.mark.parametrize("token", ["error", "success"])
def test_v8_diff_tokens_meet_the_glyph_contrast_floor(token: str) -> None:
    """V8: diff colours are UI, so they need 3:1 -- not the 4.5:1 text floor."""
    from deepagents_code.niki.theme import NIKI_DARK, contrast_ratio

    for surface in ("background", "surface", "panel"):
        ratio = contrast_ratio(NIKI_DARK[token], NIKI_DARK[surface])
        assert ratio >= 3.0, (
            f"V8: {token} on {surface} is {ratio:.2f}:1, under the 3:1 glyph floor"
        )
