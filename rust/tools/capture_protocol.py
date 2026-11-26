#!/usr/bin/env python3
"""Capture and normalize a LangGraph SSE transcript into a Niki fixture.

Two modes:

* ``--url`` starts a real run against a running ``langgraph dev`` server,
  records the request and the raw SSE body, then normalizes.
* ``--raw`` re-normalizes an existing SSE capture, so a fixture can be rebuilt
  without a server.

Normalization replaces every UUID with a stable, ordered placeholder. Message
text, event names, ordering, and JSON shape are untouched, which is what the
Rust replay test pins.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

FORMAT = "niki.protocol.fixture.v1"
UUID_RE = re.compile(
    r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
    re.IGNORECASE,
)
EVENT_RE = re.compile(r"^event: (.+)$")
DATA_RE = re.compile(r"^data: (.+)$")


def parse_sse(text: str) -> list[tuple[str, object]]:
    frames: list[tuple[str, object]] = []
    event: str | None = None
    data: list[str] = []
    for line in text.replace("\r\n", "\n").split("\n"):
        if not line:
            if event is not None:
                frames.append((event, json.loads("\n".join(data))))
            event, data = None, []
        elif match := EVENT_RE.match(line):
            event = match.group(1)
        elif match := DATA_RE.match(line):
            data.append(match.group(1))
    return frames


def normalizer() -> object:
    seen: dict[str, str] = {}

    def replace(match: re.Match[str]) -> str:
        value = match.group(0).lower()
        if value not in seen:
            seen[value] = f"00000000-0000-7000-8000-{len(seen) + 1:012d}"
        return seen[value]

    return replace


def normalize(value: object, replace: object) -> object:
    if isinstance(value, str):
        return UUID_RE.sub(replace, value)  # type: ignore[arg-type]
    if isinstance(value, list):
        return [normalize(item, replace) for item in value]
    if isinstance(value, dict):
        return {key: normalize(item, replace) for key, item in value.items()}
    return value


def build(name: str, raw: str, request: dict[str, object], source: str) -> dict[str, object]:
    frames = parse_sse(raw)
    replace = normalizer()
    return {
        "format": FORMAT,
        "name": name,
        "provenance": {
            "captured_with": source,
            "graph": "langgraph dev (scripted fixture graph)",
            "normalization": "UUIDs replaced in first-appearance order",
        },
        "request": normalize(request, replace),
        "frames": [
            {"event": event, "data": normalize(data, replace)} for event, data in frames
        ],
    }


def capture(url: str, assistant: str, prompt: str) -> tuple[str, dict[str, object]]:
    import httpx

    with httpx.Client(base_url=url, timeout=60) as client:
        thread = client.post("/threads", json={}).json()
        request = {
            "assistant_id": assistant,
            "input": {"messages": [{"type": "human", "content": prompt}]},
            "stream_mode": ["messages", "updates", "values"],
        }
        with client.stream(
            "POST", f"/threads/{thread['thread_id']}/runs/stream", json=request
        ) as response:
            response.raise_for_status()
            raw = "".join(response.iter_text())
    return raw, request


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", required=True)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--raw", type=Path, help="re-normalize an existing SSE capture")
    parser.add_argument("--request", type=Path, help="request JSON paired with --raw")
    parser.add_argument("--url", help="running langgraph dev base URL")
    parser.add_argument("--assistant", default="agent")
    parser.add_argument("--prompt", default="Say hello")
    parser.add_argument("--source", default="langgraph_api 0.15.1 / langgraph 1.2.12")
    args = parser.parse_args()

    if args.raw:
        if not args.request:
            parser.error("--raw requires --request")
        raw = args.raw.read_text()
        request = json.loads(args.request.read_text())
    elif args.url:
        raw, request = capture(args.url, args.assistant, args.prompt)
        (args.out.parent / f"{args.name}.sse").write_text(raw)
        (args.out.parent / f"{args.name}.request.json").write_text(
            json.dumps(request, indent=2) + "\n"
        )
    else:
        parser.error("one of --url or --raw is required")

    fixture = build(args.name, raw, request, args.source)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(fixture, indent=2) + "\n")
    print(f"wrote {args.out} ({len(fixture['frames'])} frames)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
