"""The harness itself has to be trustworthy before any row can cite it.

A perf number from a harness that leaks state, or a Pilot test that passes
without ever pressing a key, proves nothing. These assert the fixture model is
deterministic and offline, and that the driver really drives the app.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
from langchain_core.messages import HumanMessage

if TYPE_CHECKING:
    from collections.abc import Iterable

    from langchain_core.messages import BaseMessageChunk

from unit_tests.niki.fixture_model import (
    ALL_SCRIPTS,
    CHATTY,
    TOOL_THEN_TEXT,
    make_model,
)


def _text(chunks: Iterable[BaseMessageChunk]) -> str:
    """Concatenate streamed text.

    `AIMessageChunk.content` is typed `str | list[ContentBlock]`; the scripted
    model only ever emits strings, so a non-string block is a bug worth seeing
    rather than something to stringify away.
    """
    parts: list[str] = []
    for chunk in chunks:
        if not isinstance(chunk.content, str):
            msg = f"expected text content, got {type(chunk.content)}"
            raise TypeError(msg)
        parts.append(chunk.content)
    return "".join(parts)


async def test_scripted_model_streams_and_never_calls_generate() -> None:
    model = make_model(CHATTY)
    chunks = [c async for c in model.astream([HumanMessage("hi")])]

    assert model.stream_calls == 1, "the harness must exercise the streaming path"
    assert model.generate_calls == 0, (
        "falling back to _generate would hide a regression"
    )
    # One `astream` call is one turn, so it serves exactly step 0 of the script.
    assert _text(chunks) == CHATTY.steps[0].text


async def test_successive_turns_advance_through_the_script() -> None:
    """A second turn serves the next step, so a multi-turn test can script both."""
    model = make_model(CHATTY)

    first = [c async for c in model.astream([HumanMessage("one")])]
    second = [c async for c in model.astream([HumanMessage("two")])]

    assert _text(first) == CHATTY.steps[0].text
    assert _text(second) == CHATTY.steps[1].text
    assert model.stream_calls == 2


async def test_a_single_turn_emits_many_chunks() -> None:
    """The chatty script must produce many chunks, or it cannot test coalescing."""
    model = make_model(CHATTY)
    chunks = [c async for c in model.astream([HumanMessage("hi")])]

    assert len(chunks) >= 10, (
        "chatty script is not chatty enough to exercise coalescing"
    )


async def test_scripted_model_emits_a_tool_call() -> None:
    model = make_model(TOOL_THEN_TEXT)
    chunks = [c async for c in model.astream([HumanMessage("read it")])]

    assert model.stream_calls == 1
    tool_chunks = [c for c in chunks if c.tool_call_chunks]
    assert tool_chunks, "the tool script produced no tool call"
    assert tool_chunks[0].tool_call_chunks[0]["name"] == "read"


async def test_script_is_deterministic_across_two_runs() -> None:
    """Two fresh models must produce identical text, or the probes are noise."""
    first = [c async for c in make_model(CHATTY).astream([HumanMessage("hi")])]
    second = [c async for c in make_model(CHATTY).astream([HumanMessage("hi")])]

    assert [c.content for c in first] == [c.content for c in second]


def test_script_holds_the_last_step_instead_of_raising() -> None:
    """A test that streams longer than it scripted gets a live stream, not an error."""
    step = CHATTY.step_for(10_000)
    assert step.text == CHATTY.steps[-1].text


@pytest.mark.parametrize("script_name", ["chatty", "long-single", "tool-then-text"])
def test_every_registered_script_has_steps(script_name: str) -> None:
    assert ALL_SCRIPTS[script_name].steps, f"{script_name} is registered but empty"
