"""Framework-free runtime event and request types."""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from enum import StrEnum
from typing import Literal, cast

SCHEMA_VERSION = 1
MAX_ENCODED_BYTES = 65536


class RunEventKind(StrEnum):
    """Kinds of runtime events carried by the versioned envelope."""

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
    """Lifecycle states reported for a turn."""

    created = "created"
    running = "running"
    waiting_for_approval = "waiting_for_approval"
    completed = "completed"
    failed = "failed"
    cancelled = "cancelled"


# Recursive JSON value types (stdlib only).
JsonPrimitive = str | int | float | bool | None
JsonValue = str | int | float | bool | None | list["JsonValue"] | dict[str, "JsonValue"]
JsonObject = dict[str, JsonValue]
GoalProgressStatus = Literal["active", "paused", "complete", "blocked"]


def _validate_json_value(value: JsonValue, path: str = "") -> JsonValue:
    if value is None or isinstance(value, (bool, str, int)):
        return value
    if isinstance(value, float):
        if not math.isfinite(value):
            msg = f"non-finite float at {path}"
            raise ValueError(msg)
        return value
    if isinstance(value, list):
        return [_validate_json_value(v, f"{path}[]") for v in value]
    if isinstance(value, dict):
        result: dict[str, JsonValue] = {}
        for key, val in value.items():
            if not isinstance(key, str):
                msg = f"non-string key at {path}"
                raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
            result[key] = _validate_json_value(val, f"{path}.{key}")
        return result
    msg = f"non-JSON value at {path}: {type(value)!r}"
    raise ValueError(msg)


@dataclass(frozen=True, slots=True)
class RunEvent:
    """One validated, versioned runtime event."""

    schema_version: int
    event_id: str
    session_id: str
    turn_id: str
    sequence: int
    kind: RunEventKind
    payload: JsonObject

    def __post_init__(self) -> None:
        """Validate and normalise the envelope fields.

        Raises:
            ValueError: If any field fails validation.
        """
        if isinstance(self.schema_version, bool) or not isinstance(
            self.schema_version, int
        ):
            msg = "schema_version must be int"
            raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
        if self.schema_version != SCHEMA_VERSION:
            msg = f"unsupported schema_version: {self.schema_version}"
            raise ValueError(msg)
        if isinstance(self.sequence, bool) or not isinstance(self.sequence, int):
            msg = "sequence must be int"
            raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
        if self.sequence < 1:
            msg = f"sequence must be >= 1, got {self.sequence}"
            raise ValueError(msg)
        if not isinstance(self.kind, RunEventKind):
            try:
                object.__setattr__(self, "kind", RunEventKind(self.kind))
            except (ValueError, TypeError) as exc:
                msg = f"invalid kind: {self.kind!r}"
                raise ValueError(msg) from exc
        validated = _validate_json_value(self.payload)
        if not isinstance(validated, dict):
            msg = "payload must be a JSON object"
            raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
        object.__setattr__(self, "payload", validated)


@dataclass(frozen=True, slots=True)
class TurnRequest:
    """A prompt and workspace for one turn."""

    session_id: str
    turn_id: str
    prompt: str
    workspace_root: str


@dataclass(frozen=True, slots=True)
class ResumeRequest:
    """An approval decision that resumes a waiting turn."""

    session_id: str
    turn_id: str
    decision: JsonObject
    last_sequence: int
    prior_status: TurnStatus

    def __post_init__(self) -> None:
        """Validate and normalise the resume decision.

        Raises:
            ValueError: If any field fails validation.
        """
        if isinstance(self.last_sequence, bool) or not isinstance(
            self.last_sequence, int
        ):
            msg = "last_sequence must be int"
            raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
        if self.last_sequence < 0:
            msg = f"last_sequence must be >= 0, got {self.last_sequence}"
            raise ValueError(msg)
        if not isinstance(self.prior_status, TurnStatus):
            try:
                object.__setattr__(self, "prior_status", TurnStatus(self.prior_status))
            except (ValueError, TypeError) as exc:
                msg = f"invalid prior_status: {self.prior_status!r}"
                raise ValueError(msg) from exc
        validated = _validate_json_value(self.decision)
        if not isinstance(validated, dict):
            msg = "decision must be a JSON object"
            raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
        object.__setattr__(self, "decision", validated)


@dataclass(frozen=True, slots=True)
class GoalProgressEvent:
    """Validated goal lifecycle data projected by runtime and UI adapters."""

    session_id: str
    status: GoalProgressStatus
    note: str

    def __post_init__(self) -> None:
        """Validate the projected goal status.

        Raises:
            ValueError: If any field fails validation.
        """
        if not isinstance(self.session_id, str) or not self.session_id.strip():
            msg = "session_id must be a non-empty string"
            raise ValueError(msg)
        if not isinstance(self.status, str) or self.status not in {
            "active",
            "paused",
            "complete",
            "blocked",
        }:
            msg = f"invalid goal progress status: {self.status!r}"
            raise ValueError(msg)
        if not isinstance(self.note, str):
            msg = "goal progress note must be a string"
            raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract

    @classmethod
    def from_run_event(cls, event: RunEvent) -> GoalProgressEvent:
        """Decode the validated payload of a `goal_progress` envelope.

        Returns:
            The validated goal progress data.

        Raises:
            ValueError: If the envelope or payload is not goal progress.
        """
        if event.kind != RunEventKind.goal_progress:
            msg = "event kind must be goal_progress"
            raise ValueError(msg)
        status = event.payload.get("status")
        note = event.payload.get("note", "")
        if not isinstance(status, str) or status not in {
            "active",
            "paused",
            "complete",
            "blocked",
        }:
            msg = f"invalid goal progress status: {status!r}"
            raise ValueError(msg)
        if not isinstance(note, str):
            msg = "goal progress note must be a string"
            raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
        if status == "complete" and event.payload.get("verified") is not True:
            msg = "complete goal progress requires checkpoint verification"
            raise ValueError(msg)
        return cls(
            session_id=event.session_id,
            status=cast("GoalProgressStatus", status),
            note=note,
        )


def encode_run_event(event: RunEvent) -> str:
    """Serialise one validated event into its compact JSON envelope.

    Returns:
        The compact JSON string.

    Raises:
        ValueError: If the event is invalid or too large.
    """
    if not isinstance(event, RunEvent):
        msg = "event must be a RunEvent"
        raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
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
        msg = "encoded event exceeds 65536 bytes"
        raise ValueError(msg)
    return raw


def decode_run_event(raw: str) -> RunEvent:
    """Decode and validate one compact JSON event envelope.

    Returns:
        The decoded event.

    Raises:
        ValueError: If the envelope is malformed or invalid.
    """
    if not isinstance(raw, str):
        msg = "raw must be str"
        raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
    try:
        obj = json.loads(raw)
    except (json.JSONDecodeError, ValueError) as exc:
        msg = f"invalid JSON: {exc}"
        raise ValueError(msg) from exc
    if not isinstance(obj, dict):
        msg = "envelope must be an object"
        raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
    required = {
        "schema_version",
        "event_id",
        "session_id",
        "turn_id",
        "sequence",
        "kind",
        "payload",
    }
    if not required.issubset(obj.keys()):
        msg = "missing required fields"
        raise ValueError(msg)
    schema_version = obj["schema_version"]
    if isinstance(schema_version, bool) or not isinstance(schema_version, int):
        msg = "schema_version must be int"
        raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
    if schema_version != SCHEMA_VERSION:
        msg = f"unsupported schema_version: {schema_version}"
        raise ValueError(msg)
    sequence = obj["sequence"]
    if isinstance(sequence, bool) or not isinstance(sequence, int):
        msg = "sequence must be int"
        raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
    if sequence < 1:
        msg = f"sequence must be >= 1, got {sequence}"
        raise ValueError(msg)
    kind_raw = obj["kind"]
    try:
        kind = RunEventKind(kind_raw)
    except (ValueError, TypeError) as exc:
        msg = f"invalid kind: {kind_raw!r}"
        raise ValueError(msg) from exc
    payload = obj["payload"]
    if not isinstance(payload, dict):
        msg = "payload must be an object"
        raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
    validated_payload = _validate_json_value(payload)
    if not isinstance(validated_payload, dict):
        msg = "payload must be a JSON object"
        raise ValueError(msg)  # noqa: TRY004 - validation uses one ValueError contract
    return RunEvent(
        schema_version=schema_version,
        event_id=str(obj["event_id"]),
        session_id=str(obj["session_id"]),
        turn_id=str(obj["turn_id"]),
        sequence=sequence,
        kind=kind,
        payload=validated_payload,
    )
