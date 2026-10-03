"""Regenerate `docs/niki/KEYMAP.md` from the live binding registry.

The document is derived, never hand-maintained: every row corresponds to a
binding a mounted widget actually declares. That is what stops Help, the footer,
and the docs from advertising a key that does nothing.

Run from `libs/code`:

    uv run python scripts/gen_keymap.py
"""

from __future__ import annotations

import asyncio
import pathlib
import sys
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from deepagents_code.niki.keymap import KeymapEntry

HEADER = """# Niki Agent -- Keymap

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
"""


async def render_document() -> str:
    """Mount the app and render the complete document.

    Returns:
        The Markdown document body.
    """
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp
    from deepagents_code.niki.keymap import build_keymap, render_markdown

    app = NikiApp(agent=MagicMock(), thread_id="keymap-doc")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    async with app.run_test(size=(120, 38)) as pilot:
        await pilot.pause()
        keymap: list[KeymapEntry] = build_keymap(app)
    return f"{HEADER}\n{render_markdown(keymap)}\n"


def output_path() -> pathlib.Path:
    """Return the repository-relative destination for the document."""
    return pathlib.Path(__file__).resolve().parents[3] / "docs" / "niki" / "KEYMAP.md"


def main() -> int:
    """Write the keymap document and report where it landed.

    Returns:
        Process exit code.
    """
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
    destination = output_path()
    destination.write_text(asyncio.run(render_document()), encoding="utf-8")
    print(f"wrote {destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
