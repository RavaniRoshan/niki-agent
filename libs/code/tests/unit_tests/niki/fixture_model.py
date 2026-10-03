"""Deterministic, offline, scriptable chat model for the Niki harness.

Every Niki test drives the TUI through this model: no network, no API key, no
real provider. It replays a fixed script so a run is byte-for-byte repeatable,
which is what makes the perf probes comparable across a baseline and a change.

The base class is upstream's `_ToolBindingFakeModel`. It is private, but it is
the same base upstream's own integration fakes and `dcode tools list` build on,
and it supplies the two things `GenericFakeChatModel` lacks: a `bind_tools`
passthrough and a minimal `profile`. Reusing it keeps the harness from inventing
a second fake-model contract that could drift from what the agent runtime
actually negotiates.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any

from langchain_core.messages import AIMessage, AIMessageChunk
from langchain_core.outputs import ChatGeneration, ChatGenerationChunk, ChatResult
from pydantic import Field, PrivateAttr

from deepagents_code._fake_models import _ToolBindingFakeModel

if TYPE_CHECKING:
    from collections.abc import Iterator

    from langchain_core.callbacks import CallbackManagerForLLMRun
    from langchain_core.messages import BaseMessage


@dataclass
class StreamStep:
    """One scripted turn: text to stream, or a tool call to request.

    `delay_ms` is carried by the script but applied by the caller, so a slow
    tool can be built without a real one and the harness stays offline.
    """

    text: str = ""
    tool_call: tuple[str, str, str] | None = None
    delay_ms: int = 0


@dataclass
class Script:
    """A named script of steps, replayed in order and then held on the last.

    Holding the last step rather than raising means a test that streams longer
    than it scripted still gets a live stream instead of an error, which is what
    the per-token render probe needs.
    """

    name: str
    steps: list[StreamStep] = field(default_factory=list)

    def step_for(self, turn: int) -> StreamStep:
        """Return the step for `turn`, holding on the last one once exhausted."""
        if not self.steps:
            return StreamStep(text="")
        return self.steps[min(turn, len(self.steps) - 1)]


def _pieces(text: str, width: int = 12) -> list[str]:
    """Split `text` into fixed-width pieces, preserving every character."""
    if not text:
        return []
    return [text[i : i + width] for i in range(0, len(text), width)]


CHATTY = Script(
    name="chatty",
    steps=[StreamStep(text="Hello from the Niki fixture. " * 20) for _ in range(6)],
)
"""Six turns of ~460 characters each, so every turn streams ~39 chunks and the
coalescing timer has real work to coalesce."""

LONG_SINGLE = Script(name="long-single", steps=[StreamStep(text="x" * 20_000)])
"""One long message, isolating render cost from chunk count."""

TOOL_THEN_TEXT = Script(
    name="tool-then-text",
    steps=[
        StreamStep(tool_call=("read", '{"file_path": "/etc/hostname"}', "read-1")),
        StreamStep(text="Read the file."),
    ],
)
"""A tool call, so tool cards and approval flows have a real event to render."""

ALL_SCRIPTS = {
    "chatty": CHATTY,
    "long-single": LONG_SINGLE,
    "tool-then-text": TOOL_THEN_TEXT,
}


class ScriptedFakeModel(_ToolBindingFakeModel):
    """Replays a `Script` as a streaming chat model, counting the calls it served.

    `stream_calls` and `generate_calls` are asserted by the harness rather than
    mocked, so a test that accidentally takes the non-streaming path fails
    loudly instead of passing quietly.
    """

    messages: object = Field(default_factory=lambda: iter(()))
    stream_calls: int = 0
    generate_calls: int = 0
    _niki_script: Script | None = PrivateAttr(default=None)

    def _stream(
        self,
        messages: list[BaseMessage],  # noqa: ARG002
        stop: list[str] | None = None,  # noqa: ARG002
        run_manager: CallbackManagerForLLMRun | None = None,  # noqa: ARG002
        **kwargs: Any,  # noqa: ARG002
    ) -> Iterator[ChatGenerationChunk]:
        """Yield the scripted chunks for the current turn."""
        self.stream_calls += 1
        step = self._require_script().step_for(self.stream_calls - 1)
        if step.tool_call is not None:
            name, args, call_id = step.tool_call
            chunk = AIMessageChunk(
                content="",
                tool_call_chunks=[{"name": name, "args": args, "id": call_id}],
            )
            yield ChatGenerationChunk(message=chunk)
            return
        for piece in _pieces(step.text):
            yield ChatGenerationChunk(message=AIMessageChunk(content=piece))

    def _generate(
        self,
        messages: list[BaseMessage],  # noqa: ARG002
        stop: list[str] | None = None,  # noqa: ARG002
        run_manager: CallbackManagerForLLMRun | None = None,  # noqa: ARG002
        **kwargs: Any,  # noqa: ARG002
    ) -> ChatResult:
        """Serve the non-streaming path, which the harness must not silently use."""
        self.generate_calls += 1
        step = self._require_script().step_for(0)
        return ChatResult(
            generations=[ChatGeneration(message=AIMessage(content=step.text))]
        )

    def _require_script(self) -> Script:
        script = self._niki_script
        if script is None:
            msg = "ScriptedFakeModel used without a script; call make_model()"
            raise AssertionError(msg)
        return script


def make_model(script: Script) -> ScriptedFakeModel:
    """Build a `ScriptedFakeModel` bound to `script`."""
    model = ScriptedFakeModel(messages=iter(()))
    model._niki_script = script
    return model


__all__ = [
    "ALL_SCRIPTS",
    "CHATTY",
    "LONG_SINGLE",
    "TOOL_THEN_TEXT",
    "Script",
    "ScriptedFakeModel",
    "StreamStep",
    "make_model",
]
