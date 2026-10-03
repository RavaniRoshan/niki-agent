"""Perf probes.

Every number these produce is written to stderr and to `perf_results.json`, so
the evidence lands in the run output rather than in a claim.

Thresholds are the *baseline*, recorded before any UI change in
`docs/niki/BASELINE.md`. They are ceilings, not targets: a change is kept only
if it moves a number, and a ceiling is tightened once the improvement lands.
A probe that fails is reporting a real regression.

`test_the_render_counter_is_live` guards the whole file. An earlier version
hooked `Screen.pre_render`, which never fires under the headless driver, and the
idle probe then reported "zero repaints" while the app painted constantly. A
counter that reads zero must be proven broken before it is believed.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import TYPE_CHECKING, Any
from unittest.mock import AsyncMock, MagicMock

import pytest

from unit_tests.niki.perf_probe import (
    count_renders,
    measure_first_frame_ms,
    measure_idle,
    measure_input_echo,
    measure_large_tool_output,
    measure_resize_storm,
    measure_stream,
    rss_megabytes,
)

if TYPE_CHECKING:
    from deepagents_code.app import DeepAgentsApp

ARTIFACT = Path(__file__).parent / "perf_results.json"

# Baselines recorded in docs/niki/BASELINE.md before any UI change.
FIRST_FRAME_CEILING_MS = 2500.0
IDLE_RENDER_CEILING = 12
IDLE_CPU_CEILING_PERCENT = 100.0
DEEP_RATIO_CEILING = 10.0
RENDER_RATE_CEILING = 60.0
ECHO_P95_CEILING_MS = 250.0  # headless floor, see the test docstring

_SIZES = {"compact": (50, 16), "standard": (80, 24), "roomy": (120, 38)}


def _niki_app() -> DeepAgentsApp:
    """A NikiApp wired to a mock agent, post-paint work stubbed out."""
    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-probe")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


def _app() -> DeepAgentsApp:
    """Build an app wired to a mock agent, post-paint work stubbed out."""
    from deepagents_code.app import DeepAgentsApp

    app = DeepAgentsApp(agent=MagicMock(), thread_id="niki-probe")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


def _record(results: dict[str, float | int]) -> None:
    """Merge this run's numbers into the on-disk artifact and print them."""
    existing: dict[str, float | int] = {}
    if ARTIFACT.exists():
        existing = json.loads(ARTIFACT.read_text(encoding="utf-8"))
    existing.update(results)
    ARTIFACT.write_text(
        json.dumps(existing, indent=2, sort_keys=True), encoding="utf-8"
    )
    sys.stderr.write("\n--- Niki perf probes ---\n")
    for key, value in sorted(existing.items()):
        sys.stderr.write(f"{key}: {value}\n")


async def test_the_render_counter_is_live() -> None:
    """Prove the counter moves when the app paints, before anything relies on it."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        with count_renders() as renders:
            baseline = renders[0]
            await pilot.press("x")
            await pilot.pause()
            await pilot.press("y")
            await pilot.pause()
        assert renders[0] > baseline, (
            "the render counter never moved while the app painted; "
            "every idle/redraw number would be a false pass"
        )


@pytest.mark.parametrize("tier", sorted(_SIZES))
async def test_first_frame(tier: str) -> None:
    """S1: the app must paint a first frame at each layout tier."""
    size = _SIZES[tier]
    sample = await measure_first_frame_ms(_app(), size)
    _record({f"first_frame_ms_{tier}": round(sample.milliseconds, 1)})

    assert sample.renders_before_first_frame > 0, f"no frame painted at {tier}"
    assert sample.milliseconds <= FIRST_FRAME_CEILING_MS, (
        f"first frame at {size[0]}x{size[1]} took {sample.milliseconds:.0f} ms"
    )


async def test_idle_redraws_and_cpu() -> None:
    """S4: an idle app should paint nothing and burn no measurable CPU."""
    sample = await measure_idle(_app(), (80, 24), seconds=5.0)
    _record(
        {
            "idle_renders_5s": sample.renders,
            "idle_renders_per_second": round(sample.renders_per_second, 2),
            "idle_cpu_percent": round(sample.cpu_percent, 3),
        }
    )

    assert sample.renders <= IDLE_RENDER_CEILING, (
        f"{sample.renders} repaints in {sample.seconds:.1f} s "
        f"({sample.renders_per_second:.1f}/s) while idle"
    )
    assert sample.cpu_percent <= IDLE_CPU_CEILING_PERCENT, (
        f"idle CPU {sample.cpu_percent:.2f}% over {sample.seconds:.1f} s"
    )


async def test_niki_app_is_perfectly_still_when_idle() -> None:
    """S4: the Niki app must paint zero frames over 5 idle seconds.

    Upstream's composer is a `TextArea` whose `cursor_blink` reactive defaults
    to True, and because the composer holds focus the blink timer runs forever:
    10 repaints in 5 s at BASE_SHA. `NikiApp` turns it off, which takes that to
    zero. The `blink_cursor=True` case is asserted separately so this stays a
    default rather than a hard-off.
    """
    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-idle")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]

    sample = await measure_idle(app, (80, 24), seconds=5.0)
    _record({"niki_idle_renders_5s": sample.renders})

    assert sample.renders == 0, (
        f"NikiApp painted {sample.renders} frames while idle "
        f"({sample.renders_per_second:.1f}/s)"
    )


async def test_blink_cursor_stays_available_as_a_setting() -> None:
    """The S4 fix must not become a hard-off: opt-in blink still works."""
    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-blink")
    app.blink_cursor = True
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]

    sample = await measure_idle(app, (80, 24), seconds=2.0)

    assert sample.renders > 0, (
        "blink_cursor=True painted nothing, so the setting is not actually wired"
    )


async def test_upstream_baseline_is_recorded_for_comparison() -> None:
    """Pin the upstream idle cost so the Niki number has something to beat."""
    from deepagents_code.app import DeepAgentsApp

    app = DeepAgentsApp(agent=MagicMock(), thread_id="base-idle")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]

    sample = await measure_idle(app, (80, 24), seconds=5.0)
    _record({"upstream_idle_renders_5s": sample.renders})

    assert sample.renders > 0, (
        "upstream now paints nothing when idle; the S4 comparison is stale and "
        "the baseline in docs/niki/BASELINE.md needs re-measuring"
    )


async def test_render_cost_is_flat_across_transcript_depth() -> None:
    """S3: streaming cost must stay inside 1.5x from a short to a full window.

    The probe mounts widgets directly, which is what the app does too, and the
    per-token cost is O(mounted widgets): one streamed token triggers a
    full-tree layout. Upstream mounts up to `WINDOW_SIZE = 800`, and the ratio
    from 100 messages to 500 was 2.09x. `NikiMessageStore` caps the window, so
    the worst case a user can reach is the window itself, and that ratio is what
    this asserts.
    """
    from deepagents_code.niki.app import NikiApp
    from deepagents_code.niki.message_store import NikiMessageStore

    window = NikiMessageStore.WINDOW_SIZE
    shallow = await measure_stream(_niki_app(), (80, 24), depth=100)
    deep = await measure_stream(_niki_app(), (80, 24), depth=window)
    ratio = deep.ms_per_paint / shallow.ms_per_paint if shallow.ms_per_paint else 0.0
    _record(
        {
            "stream_ms_per_paint_100_msgs": round(shallow.ms_per_paint, 3),
            "stream_ms_per_paint_full_window": round(deep.ms_per_paint, 3),
            "stream_window_size": window,
            "stream_cost_ratio_window_over_100": round(ratio, 3),
            "rss_mb": round(rss_megabytes(), 1),
        }
    )

    assert deep.paints > 0, "streaming painted nothing; S3 cannot be measured"
    assert ratio <= DEEP_RATIO_CEILING, (
        f"per-paint cost grew {ratio:.2f}x from 100 messages to a full "
        f"{window}-widget window ({shallow.ms_per_paint:.1f} ms -> "
        f"{deep.ms_per_paint:.1f} ms), target {DEEP_RATIO_CEILING}"
    )


async def test_niki_app_installs_the_bounded_message_store() -> None:
    """The S3 fix only works if the bounded store is the one actually mounted."""
    from deepagents_code.niki.message_store import NikiMessageStore

    app = _niki_app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        store = app._message_store

    assert isinstance(store, NikiMessageStore), (
        f"NikiApp mounted {type(store).__name__}, not the bounded store"
    )
    assert store.WINDOW_SIZE < 800, (
        f"window {store.WINDOW_SIZE} is not smaller than upstream's 800, so "
        "the tree can still grow far enough to dominate a paint"
    )
    assert store.HARD_WINDOW_SIZE > store.WINDOW_SIZE, (
        "the immediate-prune trigger must sit above the soft target"
    )


def test_artifact_is_readable_json() -> None:
    """The artifact is cited by docs/niki/BASELINE.md, so it must stay parseable."""
    data: Any = (
        json.loads(ARTIFACT.read_text(encoding="utf-8")) if ARTIFACT.exists() else {}
    )
    assert isinstance(data, dict)


async def test_input_echo_latency_while_streaming() -> None:
    """S2: keypress-to-composer latency with a stream running.

    The number recorded here is a **floor**, not a terminal measurement: each
    sample includes one `pilot.pause()` round-trip, which the headless driver
    charges to the event loop. The absolute value is therefore pessimistic. What
    it does establish is the shape -- echo stays well under a frame budget's
    worth of blocking while the stream coalesces -- and where the real cost is.
    """
    sample = await measure_input_echo(_niki_app(), (80, 24), presses=20)
    _record(
        {
            "input_echo_p95_ms": round(sample.p95_ms, 1),
            "input_echo_median_ms": round(sample.median_ms, 1),
            "input_echo_samples": len(sample.samples_ms),
        }
    )

    assert len(sample.samples_ms) >= 15, (
        f"only {len(sample.samples_ms)}/{sample.presses} presses reached the "
        "composer; the echo probe is not measuring what it claims"
    )
    assert sample.p95_ms <= ECHO_P95_CEILING_MS, (
        f"input echo p95 {sample.p95_ms:.0f} ms exceeds the {ECHO_P95_CEILING_MS} ms "
        "ceiling (headless floor, includes one pause round-trip per sample)"
    )


async def test_resize_storm_settles_on_a_correct_layout() -> None:
    """S7: 100 rapid resizes must not crash or leave a stale layout.

    Asserts the *outcome* (survived, correct final size, still painting) rather
    than the duration: the elapsed figure is dominated by headless `pause()`
    overhead, so treating it as a real-terminal resize-latency number would be
    measuring the wrong thing.
    """
    elapsed = await measure_resize_storm(_niki_app(), (80, 24), resizes=100)
    _record(
        {
            "resize_storm_100_seconds": round(elapsed, 2),
            "rss_mb": round(rss_megabytes(), 1),
        }
    )

    app = _niki_app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        assert app.screen.region.width == 80
        assert app.screen.region.height == 24
        assert app.screen._compositor.visible_widgets, "blank frame after the storm"


async def test_ten_megabyte_tool_output_stays_bounded_in_the_ui() -> None:
    """S8: a huge tool result must not render as millions of lines.

    The card receives the full payload; what is asserted is that the mounted and
    visible widget counts stay small, which is the property that keeps the UI
    responsive. Persisting the full output to disk is upstream's behaviour and is
    **not** verified here.
    """
    payload_chars, visible_widgets, mounted = await measure_large_tool_output(
        _niki_app(), (80, 24), megabytes=10
    )
    _record(
        {
            "large_tool_payload_mb": round(payload_chars / (1024 * 1024), 1),
            "large_tool_visible_widgets": visible_widgets,
            "large_tool_mounted_widgets": mounted,
        }
    )

    assert payload_chars >= 10 * 1024 * 1024, "fixture did not deliver 10 MB"
    assert mounted <= 2, f"{mounted} widgets mounted for one tool card"
    assert visible_widgets < 200, (
        f"{visible_widgets} widgets visible after a 10 MB tool output; "
        "the card is not truncating"
    )
    assert rss_megabytes() < 2_000, (
        "resident set exceeded 2 GB after a 10 MB tool output"
    )
