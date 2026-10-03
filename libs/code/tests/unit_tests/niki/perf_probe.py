"""Measured perf probes for the Niki scoreboard.

Each function returns a real number and every number is printed by the tests that
call it, so a run's evidence lands in the transcript rather than in a claim.

Repaints are counted by wrapping `Compositor.render_update`, which Textual calls
once per painted frame. An earlier attempt hooked `Screen.pre_render` and
`HeadlessDriver.write`; both fire zero times under the headless driver, which
made an idle probe report "zero repaints" no matter what the app did. Counting
the compositor is the only hook here that actually moves, so it is the one used,
and `test_perf_probes.py` asserts the counter is live before trusting it.

Measurements backing the checklist:

- `measure_first_frame_ms` (S1) -- entry to the first painted frame.
- `measure_idle` (S4) -- frames painted and CPU used while nothing happens.
- `measure_stream` (S3) -- paints and cost per paint for N streamed tokens at a
  given transcript depth, so the shallow/deep ratio is comparable across runs.
"""

from __future__ import annotations

import asyncio
import os
import pathlib
import resource
import time
from contextlib import contextmanager
from dataclasses import dataclass
from typing import TYPE_CHECKING

from textual._compositor import Compositor

if TYPE_CHECKING:
    from collections.abc import Iterator

    from rich.console import RenderableType
    from textual.screen import Screen

    from deepagents_code.app import DeepAgentsApp


@dataclass
class IdleSample:
    """What the app did while nothing was happening."""

    seconds: float
    renders: int
    cpu_seconds: float

    @property
    def cpu_percent(self) -> float:
        """Process CPU over the window, as a percentage of one core."""
        return (self.cpu_seconds / self.seconds * 100) if self.seconds else 0.0

    @property
    def renders_per_second(self) -> float:
        return (self.renders / self.seconds) if self.seconds else 0.0


@dataclass
class FirstFrameSample:
    """Time from starting the app to the first painted frame."""

    milliseconds: float
    renders_before_first_frame: int


@dataclass
class StreamSample:
    """Cost of streaming `chunks` tokens into a transcript of `depth` messages.

    Upstream coalesces streamed text on a 0.1 s timer, so one token is not one
    paint. Recording paints, elapsed time, and the derived rate is the honest
    measurement: it shows rendering stays bounded and that cost per paint does
    not drift as the transcript grows.
    """

    depth: int
    chunks: int
    paints: int
    elapsed_ms: float

    @property
    def paints_per_second(self) -> float:
        return (self.paints / (self.elapsed_ms / 1000)) if self.elapsed_ms else 0.0

    @property
    def ms_per_paint(self) -> float:
        return (self.elapsed_ms / self.paints) if self.paints else 0.0


@contextmanager
def count_renders() -> Iterator[list[int]]:
    """Count painted frames by wrapping the compositor's real paint entry."""
    counter = [0]
    original = Compositor.render_update

    def counted(
        self: Compositor,
        full: bool = False,
        screen_stack: list[Screen] | None = None,
        simplify: bool = False,
    ) -> RenderableType | None:
        counter[0] += 1
        return original(self, full, screen_stack, simplify)

    Compositor.render_update = counted
    try:
        yield counter
    finally:
        Compositor.render_update = original


def _cpu_seconds() -> float:
    """Total CPU seconds for this process and its children."""
    me = resource.getrusage(resource.RUSAGE_SELF)
    kids = resource.getrusage(resource.RUSAGE_CHILDREN)
    return me.ru_utime + me.ru_stime + kids.ru_utime + kids.ru_stime


async def measure_first_frame_ms(
    app: DeepAgentsApp, size: tuple[int, int]
) -> FirstFrameSample:
    """Time entry to the first painted frame at `size`."""
    started = time.perf_counter()
    with count_renders() as renders:
        async with app.run_test(size=size) as pilot:
            await pilot.pause()
            first_frame = time.perf_counter()
            painted = renders[0]
    return FirstFrameSample(
        milliseconds=(first_frame - started) * 1000,
        renders_before_first_frame=painted,
    )


async def measure_idle(
    app: DeepAgentsApp, size: tuple[int, int], seconds: float = 5.0
) -> IdleSample:
    """Count painted frames and CPU used while the app sits idle."""
    async with app.run_test(size=size) as pilot:
        await pilot.pause()
        # Let startup work settle before the idle window opens, so a slow
        # background init is not miscounted as idle churn.
        await asyncio.sleep(0.5)
        cpu_before = _cpu_seconds()
        wall_before = time.perf_counter()
        with count_renders() as renders:
            await asyncio.sleep(seconds)
            await pilot.pause()
        wall = time.perf_counter() - wall_before
        cpu = _cpu_seconds() - cpu_before
    return IdleSample(seconds=wall, renders=renders[0], cpu_seconds=cpu)


async def measure_stream(
    app: DeepAgentsApp, size: tuple[int, int], *, depth: int, chunks: int = 60
) -> StreamSample:
    """Stream `chunks` tokens into a transcript pre-loaded to `depth` messages.

    Tokens are appended without pausing between them: pausing per token forces
    a full relayout of the mounted tree and measures the layout, not the
    streaming. One trailing pause plus a settle window is enough to let the
    coalescing timer flush, which is the behaviour under test.
    """
    from deepagents_code.tui.widgets.messages import AssistantMessage

    async with app.run_test(size=size) as pilot:
        await pilot.pause()
        transcript = app.query_one("#messages")

        await transcript.mount_all(
            [AssistantMessage(content="seed ") for _ in range(depth)]
        )
        await pilot.pause()

        target = AssistantMessage(content="")
        await transcript.mount(target)
        await pilot.pause()

        with count_renders() as renders:
            baseline = renders[0]
            started = time.perf_counter()
            for _ in range(chunks):
                await target.append_content("token ")
            await pilot.pause()
            await asyncio.sleep(0.4)
            await pilot.pause()
            elapsed_ms = (time.perf_counter() - started) * 1000
            paints = renders[0] - baseline

    return StreamSample(
        depth=depth, chunks=chunks, paints=paints, elapsed_ms=elapsed_ms
    )


def rss_megabytes() -> float:
    """Resident set size of this process, in MB, read from procfs."""
    statm = pathlib.Path(f"/proc/{os.getpid()}/statm")
    try:
        pages = int(statm.read_text(encoding="utf-8").split()[1])
    except (OSError, IndexError, ValueError):
        return 0.0
    return pages * resource.getpagesize() / (1024 * 1024)


__all__ = [
    "FirstFrameSample",
    "IdleSample",
    "StreamSample",
    "count_renders",
    "measure_first_frame_ms",
    "measure_idle",
    "measure_stream",
    "rss_megabytes",
]
