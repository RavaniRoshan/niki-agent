"""Tests for the Niki runtime event contract."""

from __future__ import annotations

import json

import pytest

from deepagents_code.domain.events import (
    RunEvent,
    RunEventKind,
    TurnStatus,
    decode_run_event,
    encode_run_event,
)


def test_run_event_json_round_trip():
    event = RunEvent(
        schema_version=1,
        event_id="e1",
        session_id="s1",
        turn_id="t1",
        sequence=1,
        kind=RunEventKind.text_delta,
        payload={"text": "hello"},
    )
    raw = encode_run_event(event)
    decoded = decode_run_event(raw)
    assert decoded == event


def test_run_event_rejects_invalid_envelope():
    with pytest.raises(ValueError):
        decode_run_event(
            json.dumps(
                {
                    "schema_version": 2,
                    "event_id": "e1",
                    "session_id": "s1",
                    "turn_id": "t1",
                    "sequence": 1,
                    "kind": "text_delta",
                    "payload": {"text": "hi"},
                }
            )
        )
    with pytest.raises(ValueError):
        decode_run_event(
            json.dumps(
                {
                    "schema_version": 1,
                    "event_id": "e1",
                    "session_id": "s1",
                    "turn_id": "t1",
                    "sequence": -1,
                    "kind": "text_delta",
                    "payload": {"text": "hi"},
                }
            )
        )
    with pytest.raises(ValueError):
        RunEvent(
            schema_version=1,
            event_id="e1",
            session_id="s1",
            turn_id="t1",
            sequence=1,
            kind=RunEventKind.text_delta,
            payload={"bad": {1, 2}},
        )
    with pytest.raises(ValueError):
        decode_run_event(
            json.dumps(
                {
                    "schema_version": 1,
                    "event_id": "e1",
                    "session_id": "s1",
                    "turn_id": "t1",
                    "sequence": 1,
                    "kind": "not_a_kind",
                    "payload": {},
                }
            )
        )
    big = "x" * 70000
    evt = RunEvent(
        schema_version=1,
        event_id="e1",
        session_id="s1",
        turn_id="t1",
        sequence=1,
        kind=RunEventKind.text_delta,
        payload={"text": big},
    )
    with pytest.raises(ValueError):
        encode_run_event(evt)


def test_run_event_rejects_oversized_payload():
    big = "x" * 65536
    evt = RunEvent(
        schema_version=1,
        event_id="e1",
        session_id="s1",
        turn_id="t1",
        sequence=1,
        kind=RunEventKind.text_delta,
        payload={"text": big},
    )
    with pytest.raises(ValueError):
        encode_run_event(evt)


def test_turn_status_values():
    assert TurnStatus.created.value == "created"
    assert TurnStatus.running.value == "running"
    assert TurnStatus.waiting_for_approval.value == "waiting_for_approval"
    assert TurnStatus.completed.value == "completed"
    assert TurnStatus.failed.value == "failed"
    assert TurnStatus.cancelled.value == "cancelled"


def test_run_event_kind_count():
    expected = {
        "session_created", "session_resumed", "turn_started", "text_delta",
        "tool_started", "tool_finished", "approval_required", "task_started",
        "task_finished", "task_failed", "task_skipped", "workflow_started",
        "workflow_finished", "workflow_failed", "goal_progress", "context_compacted",
        "turn_finished", "turn_failed", "turn_cancelled",
    }
    assert {k.value for k in RunEventKind} == expected
