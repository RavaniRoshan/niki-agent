"""Adapter from LangGraph remote streams to Niki runtime events."""

from __future__ import annotations

from collections.abc import AsyncIterator
from typing import Any

from deepagents_code.application.ports.runtime import AgentRuntime
from deepagents_code.application.run_turn import RunProtocolError
from deepagents_code.domain.events import (
    RunEvent,
    RunEventKind,
    ResumeRequest,
    TurnRequest,
)


ALLOWED_MODES = {"messages", "updates", "events", "custom"}


class LangGraphEventEncoder:
    """Encode allowlisted LangGraph observations into Niki RunEvents."""

    def encode(
        self, namespace: tuple[str, ...], mode: str, data: object
    ) -> RunEvent | None:
        if mode not in ALLOWED_MODES:
            return None
        if not isinstance(data, dict):
            return None
        if "kind" not in data:
            return None
        # Public event: validate strictly.
        try:
            schema_version = data.get("schema_version", 1)
            if isinstance(schema_version, bool) or not isinstance(schema_version, int):
                raise ValueError("schema_version must be int")
            event_id = data.get("event_id", "")
            session_id = data["session_id"]
            turn_id = data["turn_id"]
            sequence = data["sequence"]
            kind = RunEventKind(data["kind"])
            payload = data.get("payload", {})
            if not isinstance(session_id, str):
                raise ValueError("session_id must be str")
            if not isinstance(turn_id, str):
                raise ValueError("turn_id must be str")
            if isinstance(sequence, bool) or not isinstance(sequence, int):
                raise ValueError("sequence must be int")
            if sequence < 1:
                raise ValueError("sequence must be >= 1")
            if not isinstance(payload, dict):
                raise ValueError("payload must be object")
        except (KeyError, ValueError, TypeError) as exc:
            raise RunProtocolError(f"malformed public event: {exc}") from exc
        from deepagents_code.domain.events import _validate_json_value

        try:
            validated_payload = _validate_json_value(payload)
        except ValueError as exc:
            raise RunProtocolError(f"malformed public event: {exc}") from exc
        if not isinstance(validated_payload, dict):
            raise RunProtocolError("malformed public event: payload must be object")
        return RunEvent(
            schema_version=schema_version,
            event_id=str(event_id),
            session_id=session_id,
            turn_id=turn_id,
            sequence=sequence,
            kind=kind,
            payload=validated_payload,
        )


class LangGraphRemoteRuntime:
    """AgentRuntime backed by a RemoteAgent stream."""

    def __init__(
        self,
        remote_agent: Any,
        event_encoder: LangGraphEventEncoder,
    ) -> None:
        self.remote_agent = remote_agent
        self.event_encoder = event_encoder

    async def stream_turn(self, request: TurnRequest) -> AsyncIterator[RunEvent]:
        async for ns, mode, data in self.remote_agent.astream(
            {"messages": [{"role": "user", "content": request.prompt}]},
            stream_mode=["messages", "updates", "events", "custom"],
            subgraphs=True,
            config={"configurable": {"thread_id": request.session_id}},
        ):
            event = self.event_encoder.encode(tuple(ns) if ns else (), mode, data)
            if event is None:
                continue
            if event.session_id != request.session_id or event.turn_id != request.turn_id:
                raise RunProtocolError("cross-session or cross-turn event")
            yield event

    async def resume_turn(self, request: ResumeRequest) -> AsyncIterator[RunEvent]:
        async for ns, mode, data in self.remote_agent.astream(
            None,
            stream_mode=["messages", "updates", "events", "custom"],
            subgraphs=True,
            config={"configurable": {"thread_id": request.session_id}},
        ):
            event = self.event_encoder.encode(tuple(ns) if ns else (), mode, data)
            if event is None:
                continue
            if event.session_id != request.session_id or event.turn_id != request.turn_id:
                raise RunProtocolError("cross-session or cross-turn event")
            yield event

    async def cancel_turn(self, session_id: str, turn_id: str) -> None:
        cancel = getattr(self.remote_agent, "acancel_active_runs", None)
        if cancel is not None:
            await cancel({"configurable": {"thread_id": session_id}})
        else:
            cancel_one = getattr(self.remote_agent, "cancel_turn", None)
            if cancel_one is not None:
                await cancel_one(session_id, turn_id)
