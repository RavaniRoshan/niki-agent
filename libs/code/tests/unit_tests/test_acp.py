"""Tests for dcode-specific ACP approval context."""

from collections.abc import AsyncIterator
from types import SimpleNamespace
from typing import TYPE_CHECKING, Any, cast

from acp.schema import TextContentBlock
from langchain_core.messages import AIMessageChunk, HumanMessage
from langgraph.checkpoint.memory import MemorySaver
from langgraph.store.memory import InMemoryStore
from langgraph.types import Command

if TYPE_CHECKING:
    from langgraph.pregel import Pregel

    from niki_agent._cli_context import CLIContextSchema


async def test_auto_graph_adds_trusted_prompt_context() -> None:
    class Graph:
        checkpointer = object()

        async def astream(
            self, value: dict[str, object], **kwargs: object
        ) -> AsyncIterator[object]:
            self.call = {"value": value, **kwargs}
            return
            yield

    graph = Graph()
    store = InMemoryStore()
    from niki_agent.acp import _AutoGraph, _prompt

    token = _prompt.set("Update parser.py")
    try:
        chunks = [
            chunk
            async for chunk in _AutoGraph(
                cast("Pregel[Any, Any, Any, Any]", graph), store
            ).astream(
                {"messages": [{"role": "user", "content": "expanded"}]},
                config={"configurable": {"thread_id": "session-1"}},
            )
        ]
    finally:
        _prompt.reset(token)

    assert not chunks
    context = cast("CLIContextSchema", graph.call["context"])
    assert context.approval_mode == "auto"
    assert context.thread_id == "session-1"
    assert context.approval_mode_key
    item = store.get(("niki_agent", "approval_mode"), context.approval_mode_key)
    assert item
    assert item.value == {"mode": "auto"}
    message = cast("dict[str, Any]", graph.call["value"])["messages"][0]
    assert isinstance(message, HumanMessage)
    metadata = message.additional_kwargs["niki_agent_user_prompt"]
    assert metadata["literal_user_text"] == "Update parser.py"
    assert metadata["turn_id"] == context.turn_id


async def test_auto_graph_bypasses_coordinator_outside_niki_scope(monkeypatch) -> None:
    class Graph:
        checkpointer = object()

        async def astream(
            self, value: dict[str, object], **kwargs: object
        ) -> AsyncIterator[object]:
            self.call: dict[str, object] = {"value": value, **kwargs}
            yield "native-chunk"

    from niki_agent import acp as acp_module

    def fail_if_coordinated(*_args: object, **_kwargs: object) -> AsyncIterator[object]:
        msg = "legacy ACP route must not use the runtime coordinator"
        raise AssertionError(msg)

    monkeypatch.setattr(acp_module, "_coordinated_acp_stream", fail_if_coordinated)
    token = acp_module._prompt.set("legacy prompt")
    try:
        graph = Graph()
        chunks = [
            chunk
            async for chunk in acp_module._AutoGraph(
                cast("Pregel[Any, Any, Any, Any]", graph), InMemoryStore()
            ).astream(
                {"messages": [{"role": "user", "content": "legacy prompt"}]},
                config={"configurable": {"thread_id": "session-legacy"}},
                stream_mode=["messages", "updates"],
                subgraphs=True,
            )
        ]
    finally:
        acp_module._prompt.reset(token)

    assert chunks == ["native-chunk"]
    graph_call = graph.call
    graph_input = cast("dict[str, Any]", graph_call["value"])
    human_message = cast("HumanMessage", graph_input["messages"][0])
    assert human_message.content == "legacy prompt"


async def test_niki_acp_prompt_preserves_native_protocol_updates(monkeypatch) -> None:
    from niki_agent import acp as acp_module
    from niki_agent.acp import AgentServerACP, _RuntimeProgress
    from niki_agent.domain.events import RunEvent, RunEventKind, TurnStatus
    from niki_agent.niki.runtime_mode import niki_runtime_scope

    class Graph:
        checkpointer = MemorySaver()

        def __init__(self) -> None:
            self.calls: list[tuple[object, dict[str, object], object]] = []

        async def astream(
            self, value: object, **kwargs: object
        ) -> AsyncIterator[object]:
            config = cast("dict[str, object]", kwargs["config"])
            self.calls.append((value, config, kwargs["context"]))
            yield ((), "messages", (AIMessageChunk(content="ready"), {}))
            yield (
                (),
                "updates",
                {
                    "tools": {
                        "todos": [{"content": "inspect file", "status": "pending"}]
                    }
                },
            )
            tool_call = AIMessageChunk(
                content="",
                tool_call_chunks=[
                    {
                        "id": "call-1",
                        "name": "read_file",
                        "args": '{"file_path":"/workspace/a.py"}',
                        "index": 0,
                    }
                ],
            )
            yield ((), "messages", (tool_call, {}))

        async def aget_state(self, _config: object) -> object:
            return SimpleNamespace(next=(), interrupts=())

    class Client:
        def __init__(self) -> None:
            self.events: list[object] = []

        async def session_update(self, **kwargs: object) -> None:
            self.events.append(kwargs["update"])

    graph = Graph()
    server = AgentServerACP(
        lambda _context: cast("Pregel[Any, Any, Any, Any]", graph),
        store=InMemoryStore(),
    )
    client = Client()
    server.on_connect(cast("Any", client))
    session = await server.new_session(cwd="/workspace", mcp_servers=[])
    progress_events: list[tuple[RunEventKind, int, TurnStatus]] = []
    record_event = acp_module._record_runtime_event

    def record(progress: _RuntimeProgress, event: RunEvent) -> None:
        record_event(progress, event)
        progress_events.append((event.kind, event.sequence, progress.status))

    monkeypatch.setattr(acp_module, "_record_runtime_event", record)
    with niki_runtime_scope():
        response = await server.prompt(
            [TextContentBlock(type="text", text="inspect file")], session.session_id
        )

    assert response.stop_reason == "end_turn"
    assert [kind for kind, _sequence, _status in progress_events] == [
        RunEventKind.turn_started,
        RunEventKind.text_delta,
        RunEventKind.turn_finished,
    ]
    assert [sequence for _kind, sequence, _status in progress_events] == [1, 2, 3]
    assert progress_events[-1][2] == TurnStatus.completed
    configurable = cast("dict[str, object]", graph.calls[0][1]["configurable"])
    assert configurable["thread_id"] == session.session_id
    update_kinds = [getattr(event, "session_update", None) for event in client.events]
    assert "agent_message_chunk" in update_kinds
    assert "tool_call" in update_kinds
    assert "plan" in update_kinds


async def test_niki_acp_approval_resume_keeps_runtime_turn_identity(
    monkeypatch,
) -> None:
    from niki_agent import acp as acp_module
    from niki_agent.acp import AgentServerACP, _RuntimeProgress
    from niki_agent.domain.events import RunEvent, RunEventKind
    from niki_agent.niki.runtime_mode import niki_runtime_scope

    class Graph:
        checkpointer = MemorySaver()

        def __init__(self) -> None:
            self.calls: list[tuple[object, dict[str, object], object]] = []
            self.interrupt = SimpleNamespace(
                id="approval-1",
                value={
                    "action_requests": [
                        {"name": "read_file", "args": {"file_path": "/workspace/a.py"}}
                    ]
                },
            )

        async def astream(
            self, value: object, **kwargs: object
        ) -> AsyncIterator[object]:
            config = cast("dict[str, object]", kwargs["config"])
            self.calls.append((value, config, kwargs["context"]))
            if isinstance(value, Command):
                yield ((), "messages", (AIMessageChunk(content="approved"), {}))
            else:
                yield ((), "updates", {"__interrupt__": [self.interrupt]})

        async def aget_state(self, _config: object) -> object:
            return SimpleNamespace(next=(), interrupts=())

    class Client:
        def __init__(self) -> None:
            self.permission_requests = 0

        async def session_update(self, **_kwargs: object) -> None:
            return None

        async def request_permission(self, **_kwargs: object) -> object:
            self.permission_requests += 1
            return SimpleNamespace(
                outcome=SimpleNamespace(outcome="selected", option_id="approve")
            )

    graph = Graph()
    server = AgentServerACP(
        lambda _context: cast("Pregel[Any, Any, Any, Any]", graph),
        store=InMemoryStore(),
    )
    client = Client()
    server.on_connect(cast("Any", client))
    session = await server.new_session(cwd="/workspace", mcp_servers=[])
    progress_events: list[tuple[RunEventKind, int]] = []
    progress_objects: set[int] = set()
    record_event = acp_module._record_runtime_event

    def record(progress: _RuntimeProgress, event: RunEvent) -> None:
        record_event(progress, event)
        progress_events.append((event.kind, event.sequence))
        progress_objects.add(id(progress))

    monkeypatch.setattr(acp_module, "_record_runtime_event", record)
    with niki_runtime_scope():
        response = await server.prompt(
            [TextContentBlock(type="text", text="read file")], session.session_id
        )

    assert response.stop_reason == "end_turn"
    assert [kind for kind, _sequence in progress_events] == [
        RunEventKind.turn_started,
        RunEventKind.approval_required,
        RunEventKind.session_resumed,
        RunEventKind.turn_started,
        RunEventKind.text_delta,
        RunEventKind.turn_finished,
    ]
    assert [sequence for _kind, sequence in progress_events] == [1, 2, 3, 4, 5, 6]
    assert isinstance(graph.calls[1][0], Command)
    assert graph.calls[1][0].resume == {"decisions": [{"type": "approve"}]}
    assert client.permission_requests == 1
    assert len(progress_objects) == 1
    first_context = cast("CLIContextSchema", graph.calls[0][2])
    resumed_context = cast("CLIContextSchema", graph.calls[1][2])
    assert resumed_context.turn_id == first_context.turn_id
