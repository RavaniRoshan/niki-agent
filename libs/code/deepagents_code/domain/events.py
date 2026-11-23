"""Framework-free runtime event and request types."""

from __future__ import annotations

import json
from dataclasses import dataclass
from enum import StrEnum
from typing import Any

SCHEMA_VERSION = 1
MAX_ENCODED_BYTES = 65536


class RunEventKind(StrEnum):
    session_created = "session_created"
    session_resumed = "session_resumed"
    turn_started = "turn_started"
    text_delta = "text_delta"
    tool_started = "tool_started"
    tool_finished = "tool_finished"
    approval_required = "approval_required"
    task_started = "task_started"
    task_finished = "task_finished"
    task_failed = "task_failed"
    task_skipped = "task_skipped"
    workflow_started = "workflow_started"
    workflow_finished = "workflow_finished"
    workflow_failed = "workflow_failed"
    goal_progress = "goal_progress"
    context_compacted = "context_compacted"
    turn_finished = "turn_finished"
    turn_failed = "turn_failed"
    turn_cancelled = "turn_cancelled"


class TurnStatus(StrEnum):
    created = "created"
    running = "running"
    waiting_for_approval = "waiting_for_approval"
    completed = "completed"
    failed = "failed"
    cancelled = "cancelled"


# Recursive JSON value types (stdlib only).
JsonPrimitive = str | int | float | bool | None
JsonValue = "JsonPrimitive | list[JsonValue] | dict[str, JsonValue]"
JsonObject = dict[str, Any]


def _validate_json_value(value: Any, path: str = "") -> Any:
    if value is None or isinstance(value, bool) or isinstance(value, (str, int, float)):
        if isinstance(value, float):
            if value != value or value in (float("inf"), float("-inf")):
                raise ValueError(f"non-finite float at {path}")
        return value
    if isinstance(value, list):
        return [_validate_json_value(v, f"{path}[]") for v in value]
    if isinstance(value, dict):
        result: dict[str, Any] = {}
        for key, val in value.items():
            if not isinstance(key, str):
                raise ValueError(f"non-string key at {path}")
            result[key] = _validate_json_value(val, f"{path}.{key}")
        return result
    raise ValueError(f"non-JSON value at {path}: {type(value)!r}")


@dataclass(frozen=True, slots=True)
class RunEvent:
    schema_version: int
    event_id: str
    session_id: str
    turn_id: str
    sequence: int
    kind: RunEventKind
    payload: JsonObject

    def __post_init__(self) -> None:
        if isinstance(self.schema_version, bool) or not isinstance(self.schema_version, int):
            raise ValueError("schema_version must be int")
        if self.schema_version != SCHEMA_VERSION:
            raise ValueError(f"unsupported schema_version: {self.schema_version}")
        if isinstance(self.sequence, bool) or not isinstance(self.sequence, int):
            raise ValueError("sequence must be int")
        if self.sequence < 1:
            raise ValueError(f"sequence must be >= 1, got {self.sequence}")
        if not isinstance(self.kind, RunEventKind):
            try:
                object.__setattr__(self, "kind", RunEventKind(self.kind))
            except (ValueError, TypeError) as exc:
                raise ValueError(f"invalid kind: {self.kind!r}") from exc
        validated = _validate_json_value(self.payload)
        if not isinstance(validated, dict):
            raise ValueError("payload must be a JSON object")
        object.__setattr__(self, "payload", validated)


@dataclass(frozen=True, slots=True)
class TurnRequest:
    session_id: str
    turn_id: str
    prompt: str
    workspace_root: str


@dataclass(frozen=True, slots=True)
class ResumeRequest:
    session_id: str
    turn_id: str
    decision: JsonObject
    last_sequence: int
    prior_status: TurnStatus

    def __post_init__(self) -> None:
        if isinstance(self.last_sequence, bool) or not isinstance(self.last_sequence, int):
            raise ValueError("last_sequence must be int")
        if self.last_sequence < 0:
            raise ValueError(f"last_sequence must be >= 0, got {self.last_sequence}")
        if not isinstance(self.prior_status, TurnStatus):
            try:
                object.__setattr__(self, "prior_status", TurnStatus(self.prior_status))
            except (ValueError, TypeError) as exc:
                raise ValueError(f"invalid prior_status: {self.prior_status!r}") from exc
        validated = _validate_json_value(self.decision)
        if not isinstance(validated, dict):
            raise ValueError("decision must be a JSON object")
        object.__setattr__(self, "decision", validated)


@dataclass(frozen=True, slots=True)
class GoalProgressEvent:
    session_id: str
    status: str  # active | paused | complete | blocked
    note: str


def encode_run_event(event: RunEvent) -> str:
    if not isinstance(event, RunEvent):
        raise ValueError("event must be a RunEvent")
    obj = {
        "schema_version": event.schema_version,
        "event_id": event.event_id,
        "session_id": event.session_id,
        "turn_id": event.turn_id,
        "sequence": event.sequence,
        "kind": event.kind.value,
        "payload": event.payload,
    }
    raw = json.dumps(obj, separators=(",", ":"), ensure_ascii=False)
    if len(raw.encode("utf-8")) > MAX_ENCODED_BYTES:
        raise ValueError("encoded event exceeds 65536 bytes")
    return raw


def decode_run_event(raw: str) -> RunEvent:
    if not isinstance(raw, str):
        raise ValueError("raw must be str")
    try:
        obj = json.loads(raw)
    except (json.JSONDecodeError, ValueError) as exc:
        raise ValueError(f"invalid JSON: {exc}") from exc
    if not isinstance(obj, dict):
        raise ValueError("envelope must be an object")
    required = {"schema_version", "event_id", "session_id", "turn_id", "sequence", "kind", "payload"}
    if not required.issubset(obj.keys()):
        raise ValueError("missing required fields")
    schema_version = obj["schema_version"]
    if isinstance(schema_version, bool) or not isinstance(schema_version, int):
        raise ValueError("schema_version must be int")
    if schema_version != SCHEMA_VERSION:
        raise ValueError(f"unsupported schema_version: {schema_version}")
    sequence = obj["sequence"]
    if isinstance(sequence, bool) or not isinstance(sequence, int):
        raise ValueError("sequence must be int")
    if sequence < 1:
        raise ValueError(f"sequence must be >= 1, got {sequence}")
    kind_raw = obj["kind"]
    try:
        kind = RunEventKind(kind_raw)
    except (ValueError, TypeError) as exc:
        raise ValueError(f"invalid kind: {kind_raw!r}") from exc
    payload = obj["payload"]
    if not isinstance(payload, dict):
        raise ValueError("payload must be an object")
    validated_payload = _validate_json_value(payload)
    if not isinstance(validated_payload, dict):
        raise ValueError("payload must be a JSON object")
    return RunEvent(
        schema_version=schema_version,
        event_id=str(obj["event_id"]),
        session_id=str(obj["session_id"]),
        turn_id=str(obj["turn_id"]),
        sequence=sequence,
        kind=kind,
        payload=validated_payload,
    )
