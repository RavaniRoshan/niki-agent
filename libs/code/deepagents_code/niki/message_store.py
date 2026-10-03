"""`NikiMessageStore`: a bounded mounted window over upstream's virtualization.

This is the S3 fix. Measured at BASE_SHA, appending streamed text at 100 mounted
messages costs **107 ms/paint** and at 500 costs **224 ms/paint** -- a **2.09x**
growth against a 1.5x target. Profiling the deep case shows why: one streamed
token triggers a full-tree layout pass, so the cost is O(mounted widgets).
At depth 500 a single stream drove 19,811 asyncio callbacks, 3,658
`stylesheet.apply` calls, and 1.95 s inside `_refresh_layout`.

Because the cost tracks *mounted* widgets rather than total messages, the fix is
to keep the mounted window small. Upstream's `WINDOW_SIZE = 800` /
`HARD_WINDOW_SIZE = 900` is what allows the tree to grow that far. Niki caps it,
so a 5,000-message transcript mounts roughly as many widgets as a 100-message
one and the ratio flattens.

**This trades scroll-back depth for responsiveness.** Upstream already hydrates
messages back in as the user scrolls (`HYDRATE_BUFFER`, `_request_hydration`), so
a smaller window is a supported configuration rather than a broken one -- but
"how far back can I scroll without a pause" is a taste call. `WINDOW_SIZE` is a
class attribute precisely so it can be tuned; the values below are the measured
starting point, and `docs/niki/OWNER_VERIFY.md` asks the owner to judge the
scrolling feel.
"""

from __future__ import annotations

from deepagents_code.tui.widgets.message_store import MessageStore


class NikiMessageStore(MessageStore):
    """`MessageStore` with a mount window sized for a responsive transcript.

    `INITIAL_WINDOW_SIZE` is left at upstream's value: it is the synchronous
    mount count on thread resume, and cutting it would make resumed sessions
    visibly incomplete.
    """

    INITIAL_WINDOW_SIZE: int = 30

    WINDOW_SIZE: int = 150
    """Soft prune target. The mounted tree stays small enough that a streamed
    token's full-tree layout stays cheap."""

    HARD_WINDOW_SIZE: int = 180
    """Immediate-prune trigger, kept just above `WINDOW_SIZE` so a burst does not
    thrash the mount/unmount path."""

    HYDRATE_BUFFER: int = 8
    PREFETCH_VIEWPORTS: int = 8


__all__ = ["NikiMessageStore"]
