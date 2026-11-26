"""Dcode-specific ACP approval-mode adapter."""

from __future__ import annotations

import asyncio
from contextvars import ContextVar
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, cast
from uuid import uuid4

from acp.schema import PromptResponse, TextContentBlock
from langchain_core.messages import HumanMessage

from niki_agent._cli_context import CLIContextSchema
from niki_agent.acp_sdk.server import AgentServerACP as BaseAgentServerACP
from niki_agent.approval_mode import (
    APPROVAL_MODE_NAMESPACE,
    ApprovalMode,
    approval_mode_key,
    approval_mode_payload,
)
from niki_agent.auto_mode import USER_PROMPT_METADATA_KEY, user_prompt_metadata
from niki_agent.domain.events import (
    ResumeRequest,
    RunEvent,
    RunEventKind,
    TurnRequest,
    TurnStatus,
)

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Callable, Sequence

    from langchain_core.runnables import RunnableConfig
    from langgraph.pregel import Pregel
    from langgraph.store.base import BaseStore
    from langgraph.types import Command

    from niki_agent.acp_sdk.server import AgentSessionContext
    from niki_agent.adapters.runtime.acp import _PregelGraph

_prompt: ContextVar[str | None] = ContextVar("acp_auto_prompt", default=None)


@dataclass(slots=True)
class _RuntimeProgress:
    """Lifecycle identity carried across ACP approval resumes for one prompt."""

    turn_id: str
    sequence: int = 0
    status: TurnStatus = TurnStatus.created


_runtime_progress: ContextVar[_RuntimeProgress | None] = ContextVar(
    "acp_runtime_progress", default=None
)


def _record_runtime_event(progress: _RuntimeProgress, event: RunEvent) -> None:
    progress.sequence = event.sequence
    if event.kind == RunEventKind.turn_started:
        progress.status = TurnStatus.running
    elif event.kind == RunEventKind.approval_required:
        progress.status = TurnStatus.waiting_for_approval
    elif event.kind == RunEventKind.turn_finished:
        progress.status = TurnStatus.completed
    elif event.kind == RunEventKind.turn_failed:
        progress.status = TurnStatus.failed
    elif event.kind == RunEventKind.turn_cancelled:
        progress.status = TurnStatus.cancelled


async def _coordinated_acp_stream(
    graph: _PregelGraph,
    value: object,
    *,
    config: RunnableConfig,
    context: CLIContextSchema,
    progress: _RuntimeProgress,
    stream_mode: Sequence[str],
    subgraphs: bool,
) -> AsyncIterator[object]:
    """Validate an ACP graph run while yielding its native chunks to the server.

    Yields:
        The unchanged LangGraph stream chunks consumed by the ACP prompt handler.

    Raises:
        TypeError: A resumed ACP graph run has no decision object.
    """
    from langgraph.types import Command as GraphCommand

    from niki_agent.adapters.runtime.acp import ACPAgentRuntime
    from niki_agent.application.native_stream import stream_native_chunks
    from niki_agent.application.run_turn import RunTurnCoordinator

    session_id = config["configurable"]["thread_id"]
    metadata = config.get("metadata", {})
    workspace_root = metadata.get("cwd", "") if isinstance(metadata, dict) else ""
    chunks: asyncio.Queue[object] = asyncio.Queue(maxsize=1)
    runtime = ACPAgentRuntime(
        graph,
        on_chunk=chunks.put,
        stream_input=None if isinstance(value, GraphCommand) else value,
        config=config,
        stream_mode=stream_mode,
        subgraphs=subgraphs,
        stream_kwargs={"context": context},
    )
    coordinator = RunTurnCoordinator(runtime)
    if isinstance(value, GraphCommand):
        decision = value.resume
        if not isinstance(decision, dict):
            msg = "ACP runtime resume requires a decision object"
            raise TypeError(msg)
        request: TurnRequest | ResumeRequest = ResumeRequest(
            session_id,
            progress.turn_id,
            decision,
            progress.sequence,
            progress.status,
        )
    else:
        request = TurnRequest(
            session_id,
            progress.turn_id,
            _prompt.get() or "",
            str(workspace_root),
        )
    async for chunk in stream_native_chunks(
        coordinator,
        request,
        chunks,
        on_event=lambda event: _record_runtime_event(progress, event),
    ):
        yield chunk


class _AutoGraph:
    """Add trusted Auto control state to ACP graph runs."""

    def __init__(self, graph: Pregel[Any, Any, Any, Any], store: BaseStore) -> None:
        self._graph = graph
        self._store = store
        self.checkpointer = graph.checkpointer
        self._turn_id = ""

    async def astream(
        self,
        value: dict[str, Any] | Command,
        *,
        config: RunnableConfig,
        **kwargs: Any,
    ) -> AsyncIterator[Any]:
        session_id = config["configurable"]["thread_id"]
        prompt = _prompt.get()
        progress = _runtime_progress.get()
        turn_id = self._turn_id
        if progress is not None:
            turn_id = progress.turn_id
        elif prompt is not None:
            self._turn_id = uuid4().hex
            turn_id = self._turn_id
        key = approval_mode_key(session_id)
        self._store.put(
            APPROVAL_MODE_NAMESPACE,
            key,
            dict(approval_mode_payload(mode=ApprovalMode.AUTO)),
        )
        if prompt is not None and isinstance(value, dict):
            messages = list(value.get("messages", []))
            if messages:
                messages[-1] = HumanMessage(
                    content=messages[-1]["content"],
                    additional_kwargs={
                        USER_PROMPT_METADATA_KEY: user_prompt_metadata(
                            prompt, [], turn_id=turn_id
                        )
                    },
                )
                value = {**value, "messages": messages}
        context = CLIContextSchema(
            approval_mode=ApprovalMode.AUTO.value,
            auto_approve=True,
            approval_mode_key=key,
            thread_id=session_id,
            turn_id=turn_id,
        )
        graph = cast("Any", self._graph)
        if progress is not None:
            stream_mode = kwargs.get("stream_mode", ("messages", "updates"))
            if not isinstance(stream_mode, (list, tuple)):
                stream_mode = ("messages", "updates")
            async for chunk in _coordinated_acp_stream(
                graph,
                value,
                config=config,
                context=context,
                progress=progress,
                stream_mode=stream_mode,
                subgraphs=kwargs.get("subgraphs") is True,
            ):
                yield chunk
        else:
            async for chunk in graph.astream(
                value, config=config, context=context, **kwargs
            ):
                yield chunk

    async def aget_state(self, config: RunnableConfig) -> object:
        return await self._graph.aget_state(config)

    async def aupdate_state(
        self,
        config: RunnableConfig,
        values: dict[str, Any],
        *,
        as_node: str | None = None,
    ) -> RunnableConfig:
        return await self._graph.aupdate_state(config, values, as_node=as_node)

    def aget_state_history(self, config: RunnableConfig) -> AsyncIterator[Any]:
        return self._graph.aget_state_history(config)


class AgentServerACP(BaseAgentServerACP):
    """ACP server that supplies trusted classifier context in Auto mode."""

    def __init__(
        self,
        agent: Callable[[AgentSessionContext], Pregel[Any, Any, Any, Any]],
        *,
        store: BaseStore,
        **kwargs: Any,
    ) -> None:
        """Initialize the Auto-aware ACP server."""

        def build(context: AgentSessionContext) -> _AutoGraph:
            return _AutoGraph(agent(context), store)

        super().__init__(cast("Any", build), **kwargs)

    async def prompt(
        self,
        prompt: Sequence[Any],
        session_id: str,
        message_id: str | None = None,
        **kwargs: Any,
    ) -> PromptResponse:
        """Run an ACP prompt with trusted classifier metadata.

        Returns:
            The ACP prompt response.
        """
        from niki_agent.niki.runtime_mode import is_niki_runtime_enabled

        text = "\n".join(
            block.text for block in prompt if isinstance(block, TextContentBlock)
        )
        token = _prompt.set(text)
        progress = _RuntimeProgress(uuid4().hex) if is_niki_runtime_enabled() else None
        progress_token = _runtime_progress.set(progress)
        try:
            return await super().prompt(
                list(prompt), session_id, message_id=message_id, **kwargs
            )
        finally:
            _prompt.reset(token)
            _runtime_progress.reset(progress_token)
