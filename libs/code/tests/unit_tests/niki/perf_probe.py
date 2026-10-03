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
import contextlib
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


@dataclass
class EchoSample:
    """Composer key-echo latency, measured while a stream is running."""

    presses: int
    samples_ms: list[float]

    @property
    def p95_ms(self) -> float:
        if not self.samples_ms:
            return 0.0
        ordered = sorted(self.samples_ms)
        # Nearest-rank p95; at these sample counts an interpolation would imply
        # precision the probe does not have.
        index = min(len(ordered) - 1, round(0.95 * (len(ordered) - 1)))
        return ordered[index]

    @property
    def median_ms(self) -> float:
        ordered = sorted(self.samples_ms)
        return ordered[len(ordered) // 2] if ordered else 0.0


async def measure_input_echo(
    app: DeepAgentsApp,
    size: tuple[int, int],
    *,
    presses: int = 20,
    while_streaming: bool = True,
) -> EchoSample:
    """Time each keypress from `pilot.press` to the character reaching the composer.

    With `while_streaming`, a background stream keeps appending to a mounted
    assistant message for the duration, which is the condition S2 asks for: a
    tool flood plus a stream, not a quiet screen.
    """
    from deepagents_code.tui.widgets.messages import AssistantMessage

    samples: list[float] = []
    async with app.run_test(size=size) as pilot:
        await pilot.pause()
        chat_input = app.query_one("#chat-input")
        target = AssistantMessage(content="")
        await app.query_one("#messages").mount(target)
        await pilot.pause()

        async def _stream() -> None:
            for _ in range(200):
                await target.append_content("token ")
                await asyncio.sleep(0.01)

        streamer = asyncio.create_task(_stream()) if while_streaming else None
        try:
            for _ in range(presses):
                before = str(getattr(chat_input, "text", ""))
                started = time.perf_counter()
                await pilot.press("x")
                await pilot.pause()
                elapsed = (time.perf_counter() - started) * 1000
                if str(getattr(chat_input, "text", "")) != before:
                    samples.append(elapsed)
        finally:
            if streamer is not None:
                streamer.cancel()
                with contextlib.suppress(asyncio.CancelledError):
                    await streamer
    return EchoSample(presses=presses, samples_ms=samples)


async def measure_resize_storm(
    app: DeepAgentsApp, size: tuple[int, int], *, resizes: int = 100
) -> float:
    """Drive `resizes` resizes as fast as possible and return the elapsed seconds.

    Returns:
        Wall-clock seconds for the storm, including the settle pass afterwards.
    """
    import random

    started = time.perf_counter()
    async with app.run_test(size=size) as pilot:
        await pilot.pause()
        rng = random.Random(0)
        for index in range(resizes):
            width = 50 + rng.randrange(0, 120)
            height = 16 + rng.randrange(0, 30)
            await pilot.resize_terminal(width, height)
            await pilot.pause()
            if index % 10 == 0:
                await asyncio.sleep(0.002)
        # Settle on a known size and let the layout finish.
        await pilot.resize_terminal(*size)
        await pilot.pause()
        await pilot.pause()
    return time.perf_counter() - started


async def measure_large_tool_output(
    app: DeepAgentsApp, size: tuple[int, int], *, megabytes: int = 10
) -> tuple[int, int, int]:
    """Push a very large tool result through a tool card and report the size.

    Returns:
        `(input_chars, rendered_lines, mounted_widgets)` after the card renders.
        A bounded UI keeps `rendered_lines` small even though `input_chars` is
        in the millions.
    """
    from deepagents_code.tui.widgets.messages import ToolCallMessage

    payload = "x" * (megabytes * 1024 * 1024)
    async with app.run_test(size=size) as pilot:
        await pilot.pause()
        messages = app.query_one("#messages")
        before = len(messages.children)
        card = ToolCallMessage(tool_name="bash", args={"cmd": "cat huge.log"})
        await messages.mount(card)
        await pilot.pause()

        # The card stores output on `_output` and re-renders through
        # `_update_output_display`; both are private, so a rename upstream breaks
        # this probe loudly rather than silently measuring nothing.
        card._output = payload
        card._update_output_display()
        await pilot.pause()
        await asyncio.sleep(0.2)
        await pilot.pause()

        rendered_lines = len(app.screen._compositor.visible_widgets)
        mounted = len(messages.children) - before
    return len(payload), rendered_lines, mounted


def rss_growth_megabytes(before: float) -> float:
    """RSS growth since `before`.

    Returns:
        The increase in resident set size, in MB.
    """
    return rss_megabytes() - before
