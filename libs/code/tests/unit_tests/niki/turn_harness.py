"""Drive one real NikiApp turn offline with a scripted agent double.

The app's turn pipeline is the part every "agent-turn" checklist row depends
on, and it is normally reached only through the HTTP client to the LangGraph
subprocess. That is why several rows were parked on the owner: there was no way
to produce a real streamed turn without a provider.

This module closes that gap the same way the ACP tests do — with a transport
double rather than a mocked-out app. `ScriptedAgent.astream` yields the same
three-tuples LangGraph's `subagent_stream` yields, and the app consumes them
through its real adapter, widget mounting, status bar, and queue drain. Nothing
in the app is stubbed except the server start-up itself (`_post_paint_init` is
replaced, exactly as the other Niki tests do): a turn started here runs the
same code a provider-backed turn runs.

Script steps:

- `Text` — assistant text, optionally carrying usage metadata.
- `ToolUse` — a tool call as `AIMessageChunk.tool_call_chunks`.
- `ToolResult` — the matching `ToolMessage` result.
- `Usage` — a usage-only chunk.
- `Gate` — block the stream until the test sets an `asyncio.Event`, which is
  how a turn is held open long enough to assert on its in-flight UI.
- `Sleep` — block for wall-clock time.
- `Mutate` — run a synchronous side effect between chunks (e.g. actually write
  the file a `write_file` call claims to have written, so the diff read-back
  sees the new content).
"""

from __future__ import annotations

import asyncio
import json
from dataclasses import dataclass
from types import SimpleNamespace
from typing import TYPE_CHECKING, Any

from langchain_core.messages import AIMessageChunk, ToolMessage

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Callable, Sequence

    from textual.pilot import Pilot

    from deepagents_code.niki.app import NikiApp
    from deepagents_code.tui.textual_adapter import TextualUIAdapter


@dataclass(frozen=True)
class Text:
    """One assistant text chunk."""

    content: str
    usage: dict[str, int] | None = None


@dataclass(frozen=True)
class ToolUse:
    """One tool call, emitted as a streaming tool-call chunk."""

    call_id: str
    name: str
    args: dict[str, Any]


@dataclass(frozen=True)
class ToolResult:
    """The result message matching a `ToolUse`."""

    call_id: str
    name: str
    content: str
    status: str = "success"


@dataclass(frozen=True)
class Usage:
    """A usage-only chunk, for accounting assertions."""

    input_tokens: int
    output_tokens: int


@dataclass(frozen=True)
class Gate:
    """Hold the stream open until `event` is set."""

    event: asyncio.Event


@dataclass(frozen=True)
class Sleep:
    """Block the stream for wall-clock time."""

    seconds: float


@dataclass(frozen=True)
class Mutate:
    """Run a side effect between chunks."""

    action: Callable[[], None]


@dataclass(frozen=True)
class Raise:
    """Raise an exception from inside the stream, as a failed provider would."""

    exception: Exception


Step = Text | ToolUse | ToolResult | Usage | Gate | Sleep | Mutate | Raise


class ScriptedAgent:
    """A transport double that replays scripts in turn order.

    Each call to `astream` consumes the next script; the last script repeats so
    a test that queues a follow-up message does not run out of answers.
    """

    def __init__(self, *scripts: Sequence[Step]) -> None:
        """Initialize the agent.

        Args:
            *scripts: One script per turn. At least one is required.
        """
        self._scripts = [list(script) for script in scripts]
        self.calls: list[Any] = []

    async def aput_store_item(self, *_args: Any, **_kwargs: Any) -> None:
        """Stand in for the remote store writer (no-op)."""

    async def aget_state(self, _config: Any) -> Any:  # noqa: ANN401  # graph state is dynamic
        """Return an empty graph state with no interrupts."""
        return SimpleNamespace(next=(), interrupts=(), values={})

    async def aupdate_state(self, _config: object, _update: object) -> None:
        """Accept a state write and discard it (no-op)."""

    async def astream(
        self,
        value: Any,  # noqa: ANN401  # graph input is dynamic
        **_kwargs: Any,
    ) -> AsyncIterator[object]:
        """Yield LangGraph-shaped `(namespace, mode, payload)` chunks."""
        self.calls.append(value)
        if not self._scripts:
            script: list[Step] = []
        elif len(self._scripts) == 1:
            script = self._scripts[0]
        else:
            script = self._scripts.pop(0)

        for step in script:
            if isinstance(step, Gate):
                await step.event.wait()
            elif isinstance(step, Sleep):
                await asyncio.sleep(step.seconds)
            elif isinstance(step, Mutate):
                step.action()
            elif isinstance(step, Raise):
                raise step.exception
            elif isinstance(step, ToolUse):
                yield (
                    (),
                    "messages",
                    (
                        AIMessageChunk(
                            content="",
                            tool_call_chunks=[
                                {
                                    "id": step.call_id,
                                    "name": step.name,
                                    "args": json.dumps(step.args),
                                    "index": 0,
                                }
                            ],
                        ),
                        {},
                    ),
                )
            elif isinstance(step, ToolResult):
                yield (
                    (),
                    "messages",
                    (
                        ToolMessage(
                            content=step.content,
                            tool_call_id=step.call_id,
                            name=step.name,
                            status=step.status,
                        ),
                        {},
                    ),
                )
            elif isinstance(step, Usage):
                yield (
                    (),
                    "messages",
                    (
                        AIMessageChunk(
                            content="",
                            usage_metadata={
                                "input_tokens": step.input_tokens,
                                "output_tokens": step.output_tokens,
                                "total_tokens": step.input_tokens + step.output_tokens,
                            },
                        ),
                        {},
                    ),
                )
            else:
                yield (
                    (),
                    "messages",
                    (
                        AIMessageChunk(
                            content=step.content,
                            usage_metadata=step.usage,
                        ),
                        {},
                    ),
                )
        yield ((), "updates", {"agent": {"messages": []}})


def make_app(*, thread_id: str = "niki-turn") -> NikiApp:
    """Build a keyless NikiApp with its server start-up stubbed out."""
    from unittest.mock import AsyncMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=None, thread_id=thread_id)
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


def build_adapter(app: NikiApp) -> TextualUIAdapter:
    """Build the app's real UI adapter against its own callbacks.

    Mirrors the wiring in `_post_paint_init`, including the token, cost, and
    stream-complete callbacks, which are assigned after construction there.
    Without them a recorded usage chunk updates the session stats but never
    reaches the footer.
    """
    from deepagents_code.tui.textual_adapter import TextualUIAdapter

    adapter = TextualUIAdapter(
        mount_message=app._mount_message,
        update_status=app._update_status,
        request_approval=app._request_approval,
        on_auto_approve_enabled=app._on_auto_approve_enabled,
        on_switch_to_manual=app._switch_to_manual_from_fallback,
        set_spinner=app._set_spinner,
        set_active_message=app._set_active_message,
        on_user_visible_output_started=app._on_user_visible_output_started,
        sync_message_content=app._sync_message_content,
        sync_tool_message=app._sync_tool_message_state,
        request_ask_user=app._request_ask_user,
        on_tool_complete=app._schedule_git_branch_refresh,
        on_subagent_event=app._on_subagent_event,
        on_team_event=app._on_team_event,
        on_runtime_event=app._on_runtime_event,
        on_auto_mode_event=app._on_auto_mode_event,
        on_approval_mode_fallback=app._on_approval_mode_fallback,
        show_diff_line_numbers=app._show_diff_line_numbers,
    )
    adapter._on_tokens_update = app._on_tokens_update
    adapter._on_tokens_pending = app._show_pending_tokens
    adapter._on_tokens_show = app._show_tokens
    adapter._on_session_cost = app._set_session_cost
    adapter._on_provisional_cost = app._add_provisional_cost
    adapter._on_usage_update = app._refresh_cache_display
    adapter._on_stream_complete = app._mark_thread_turn_completed
    return adapter


async def arm_turn(
    app: NikiApp,
    pilot: Pilot[None],
    agent: ScriptedAgent,
    *,
    thread_id: str = "niki-turn",
) -> None:
    """Wire the agent and session the way a connected app would have them."""
    await app._init_session_state()
    app._ui_adapter = build_adapter(app)
    app._agent = agent  # type: ignore[assignment]
    app._lc_thread_id = thread_id
    await pilot.pause()


async def wait_running(app: NikiApp, pilot: Pilot[None], *, limit: float = 5.0) -> None:
    """Wait until the app reports a turn in flight."""
    await _wait_for(pilot, lambda: bool(app._agent_running), limit=limit)


async def wait_idle(app: NikiApp, pilot: Pilot[None], *, limit: float = 20.0) -> None:
    """Wait until no turn is running and no queued message is waiting."""
    await _wait_for(
        pilot,
        lambda: not app._agent_running and not app._pending_messages,
        limit=limit,
    )


async def _wait_for(
    pilot: Pilot[None],
    predicate: Callable[[], bool],
    *,
    limit: float,
) -> None:
    deadline = asyncio.get_running_loop().time() + limit
    while asyncio.get_running_loop().time() < deadline:
        await pilot.pause()
        if predicate():
            # One more paint so the UI reflects the state that was waited for.
            await pilot.pause()
            return
        await asyncio.sleep(0.02)
    msg = "timed out waiting for the app to reach the expected state"
    raise AssertionError(msg)


def screen_text(app: NikiApp) -> str:
    """Return what a terminal would show, as plain text."""
    return "\n".join(
        strip.text
        for strip in app.screen._compositor.render_strips()
        if strip is not None
    )


async def scroll_end(app: NikiApp, pilot: Pilot[None]) -> None:
    """Scroll the transcript to its newest content before capturing."""
    app.query_one("#chat").scroll_end(animate=False)
    await pilot.pause()
    await pilot.pause()
