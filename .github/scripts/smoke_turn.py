#!/usr/bin/env python3
"""Prove that an installed ``niki`` can run one real turn.

The release workflow installs a built wheel and only checks ``niki --version``.
That imports almost nothing: v1.1.1 shipped a wheel whose first real turn
raised ``TypeError`` while ``--version`` printed happily. This script closes
that gap end to end, with no network access beyond loopback and no third-party
packages (stdlib only, Python 3.12+).

How it works:

1. Starts a threaded HTTP server on ``127.0.0.1:0`` that speaks
   ``POST /v1/chat/completions`` (streaming and non-streaming).
2. Writes a throwaway ``NIKI_HOME`` whose ``config.toml`` declares a mock
   provider (``class_path = "langchain_openai:ChatOpenAI"``) pointed at the
   server, and makes it the default model.
3. Runs ``niki -n "<prompt>"`` in a throwaway workspace, headless.
4. Answers the first model request with a ``write_file`` tool call; any request
   that already carries a tool result is answered with a short text message and
   ``finish_reason: "stop"``.
5. Asserts the child exited 0, the file exists with the expected content, and
   the tool result came back to the model.

Exit status is 0 only when every check passes.

Usage::

    python3 .github/scripts/smoke_turn.py --niki /path/to/bin/niki
    python3 .github/scripts/smoke_turn.py --python /path/to/.venv/bin/python

The interpreter is part of the product's default configuration, so it stays
enabled by default here: a wheel that cannot construct its interpreter
middleware must fail this gate. ``--no-interpreter`` passes the product's own
opt-out (useful while diagnosing an interpreter-only regression).
"""

from __future__ import annotations

import argparse
import json
import os
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

PROVIDER = "smoke"
MODEL = "smoke-model"
MODEL_SPEC = f"{PROVIDER}:{MODEL}"
API_KEY_ENV = "NIKI_SMOKE_API_KEY"
API_KEY = "smoke-turn-not-a-secret"
TOOL_NAME = "write_file"
TOOL_CALL_ID = "call_smoke_turn_1"
TARGET_FILE = "smoke_turn.txt"
EXPECTED_CONTENT = "smoke turn ok"
FINAL_TEXT = "Smoke turn complete."
DEFAULT_TIMEOUT = 180.0
EXPECTED_REQUESTS = 2
"""One request for the tool call, one carrying the result back."""

PROMPT = (
    f"Use the {TOOL_NAME} tool to create the file {TARGET_FILE} in the current "
    f"directory with the exact text {EXPECTED_CONTENT!r}. Then reply with one "
    "short sentence confirming it."
)

CONFIG_TEMPLATE = """\
# Written by .github/scripts/smoke_turn.py: a throwaway release-gate profile.
[models]
default = "{spec}"

[models.providers.{provider}]
class_path = "langchain_openai:ChatOpenAI"
base_url = "{base_url}"
api_key_env = "{api_key_env}"
models = ["{model}"]
display_name = "Smoke turn (loopback)"

[models.providers.{provider}.profile]
tool_calling = true
max_input_tokens = 128000
"""


class MockProvider:
    """Loopback OpenAI-compatible server that drives exactly one tool turn."""

    def __init__(self, target: Path) -> None:
        self.target = target
        self._lock = threading.Lock()
        self.requests: list[dict[str, Any]] = []
        self.tool_call_issued = False
        self._server = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
        self._server.daemon_threads = True
        self._server.provider = self  # type: ignore[attr-defined]
        self._thread = threading.Thread(
            target=self._server.serve_forever, name="smoke-mock", daemon=True
        )

    @property
    def base_url(self) -> str:
        host, port = self._server.server_address[:2]
        return f"http://{host}:{port}/v1"

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._server.shutdown()
        self._server.server_close()
        self._thread.join(timeout=5)

    def arguments(self) -> str:
        return json.dumps(
            {"file_path": str(self.target), "content": EXPECTED_CONTENT},
            ensure_ascii=False,
        )

    def decide(self, body: dict[str, Any]) -> str:
        """Record one request and choose the reply.

        Returns:
            ``"tool_call"`` for the first request, ``"text"`` afterwards.
        """
        messages = body.get("messages")
        messages = messages if isinstance(messages, list) else []
        has_tool_result = any(
            isinstance(message, dict) and message.get("role") == "tool"
            for message in messages
        )
        with self._lock:
            self.requests.append(body)
            if has_tool_result or self.tool_call_issued:
                return "text"
            self.tool_call_issued = True
            return "tool_call"

    def snapshot(self) -> list[dict[str, Any]]:
        with self._lock:
            return list(self.requests)


class _Handler(BaseHTTPRequestHandler):
    """Minimal OpenAI chat-completions endpoint."""

    protocol_version = "HTTP/1.1"
    server_version = "niki-smoke-turn/1"

    def log_message(self, format: str, *args: object) -> None:  # noqa: A002
        """Silence the default stderr access log; the report is the output."""

    @property
    def provider(self) -> MockProvider:
        return self.server.provider  # type: ignore[attr-defined, no-any-return]

    def do_POST(self) -> None:  # BaseHTTPRequestHandler API
        if self.path.split("?", 1)[0].rstrip("/") != "/v1/chat/completions":
            self._send_json(404, {"error": {"message": f"unexpected path {self.path}"}})
            return
        try:
            length = int(self.headers.get("Content-Length") or 0)
        except ValueError:
            self._send_json(400, {"error": {"message": "bad Content-Length"}})
            return
        raw = self.rfile.read(length) if length > 0 else b""
        try:
            body = json.loads(raw or b"{}")
        except json.JSONDecodeError:
            self._send_json(400, {"error": {"message": "request body is not JSON"}})
            return
        if not isinstance(body, dict):
            message = "request body is not an object"
            self._send_json(400, {"error": {"message": message}})
            return
        kind = self.provider.decide(body)
        if body.get("stream"):
            self._send_stream(kind, body)
        else:
            self._send_completion(kind)

    # --- responses ---------------------------------------------------------

    def _tool_calls(self) -> list[dict[str, Any]]:
        return [
            {
                "id": TOOL_CALL_ID,
                "type": "function",
                "function": {
                    "name": TOOL_NAME,
                    "arguments": self.provider.arguments(),
                },
            }
        ]

    def _send_completion(self, kind: str) -> None:
        if kind == "tool_call":
            message: dict[str, Any] = {
                "role": "assistant",
                "content": None,
                "tool_calls": self._tool_calls(),
            }
            finish_reason = "tool_calls"
        else:
            message = {"role": "assistant", "content": FINAL_TEXT}
            finish_reason = "stop"
        payload = {
            "id": f"chatcmpl-{kind}",
            "object": "chat.completion",
            "created": int(time.time()),
            "model": MODEL,
            "choices": [
                {
                    "index": 0,
                    "message": message,
                    "finish_reason": finish_reason,
                    "logprobs": None,
                }
            ],
            "usage": {"prompt_tokens": 12, "completion_tokens": 6, "total_tokens": 18},
        }
        self._send_json(200, payload)

    def _send_stream(self, kind: str, body: dict[str, Any]) -> None:
        self._start_sse()
        if kind == "tool_call":
            self._delta({"role": "assistant", "content": None})
            self._delta(
                {
                    "tool_calls": [
                        {
                            "index": 0,
                            "id": TOOL_CALL_ID,
                            "type": "function",
                            "function": {"name": TOOL_NAME, "arguments": ""},
                        }
                    ]
                }
            )
            # Split the arguments across chunks, as a real provider would, so
            # the client's tool-call assembly is exercised too.
            arguments = self.provider.arguments()
            for start in range(0, len(arguments), 40):
                self._delta(
                    {
                        "tool_calls": [
                            {
                                "index": 0,
                                "function": {
                                    "arguments": arguments[start : start + 40]
                                },
                            }
                        ]
                    }
                )
            self._delta({}, finish_reason="tool_calls")
        else:
            self._delta({"role": "assistant", "content": ""})
            for index, word in enumerate(FINAL_TEXT.split(" ")):
                self._delta({"content": word if index == 0 else f" {word}"})
            self._delta({}, finish_reason="stop")
        if self._wants_usage(body):
            self._write_event(
                {
                    "id": "chatcmpl-usage",
                    "object": "chat.completion.chunk",
                    "created": int(time.time()),
                    "model": MODEL,
                    "choices": [],
                    "usage": {
                        "prompt_tokens": 12,
                        "completion_tokens": 6,
                        "total_tokens": 18,
                    },
                }
            )
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()
        self.close_connection = True

    @staticmethod
    def _wants_usage(body: dict[str, Any]) -> bool:
        options = body.get("stream_options")
        return isinstance(options, dict) and bool(options.get("include_usage"))

    def _delta(self, delta: dict[str, Any], finish_reason: str | None = None) -> None:
        self._write_event(
            {
                "id": "chatcmpl-smoke",
                "object": "chat.completion.chunk",
                "created": int(time.time()),
                "model": MODEL,
                "choices": [
                    {
                        "index": 0,
                        "delta": delta,
                        "finish_reason": finish_reason,
                        "logprobs": None,
                    }
                ],
            }
        )

    def _write_event(self, payload: dict[str, Any]) -> None:
        self.wfile.write(b"data: " + json.dumps(payload).encode("utf-8") + b"\n\n")
        self.wfile.flush()

    def _start_sse(self) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Connection", "close")
        self.end_headers()
        self.close_connection = True

    def _send_json(self, status: int, payload: dict[str, Any]) -> None:
        encoded = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(encoded)
        self.wfile.flush()


def _write_config(home: Path, base_url: str) -> Path:
    config_path = home / "config.toml"
    config_path.write_text(
        CONFIG_TEMPLATE.format(
            spec=MODEL_SPEC,
            provider=PROVIDER,
            model=MODEL,
            base_url=base_url,
            api_key_env=API_KEY_ENV,
        ),
        encoding="utf-8",
    )
    return config_path


def _child_env(home: Path, cache: Path, tmp: Path) -> dict[str, str]:
    """Environment for the child: loopback only, no profile or cache leakage."""
    env = os.environ.copy()
    env.pop("DEEPAGENTS_HOME", None)
    env.update(
        {
            "NIKI_HOME": str(home),
            API_KEY_ENV: API_KEY,
            "NIKI_OFFLINE": "1",
            "NIKI_NO_UPDATE_CHECK": "1",
            "NIKI_AUTO_UPDATE": "0",
            "NIKI_ONBOARDING": "0",
            "NIKI_PRICES_AUTO_UPDATE": "0",
            "NIKI_PLUGIN_AUTO_UPDATE": "0",
            "NIKI_HIDE_SPLASH_TIPS": "1",
            "XDG_CACHE_HOME": str(cache),
            "PYTHONUNBUFFERED": "1",
            "NO_COLOR": "1",
            "TERM": "dumb",
        }
    )
    for name in ("TMPDIR", "TMP", "TEMP"):
        env[name] = str(tmp)
    if os.name == "nt":  # pragma: no cover - Windows
        env["LOCALAPPDATA"] = str(cache)
    return env


def _terminate(proc: subprocess.Popen[str]) -> None:
    if proc.poll() is not None:
        return
    try:
        if os.name == "posix":
            os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
        else:  # pragma: no cover - Windows
            proc.kill()
    except OSError:
        proc.kill()


def _run(
    argv: list[str], *, cwd: Path, env: dict[str, str], timeout: float
) -> tuple[int, str, str, bool]:
    kwargs: dict[str, Any] = {
        "cwd": str(cwd),
        "env": env,
        "stdout": subprocess.PIPE,
        "stderr": subprocess.PIPE,
        "text": True,
        "encoding": "utf-8",
        "errors": "replace",
    }
    if os.name == "posix":
        kwargs["start_new_session"] = True
    else:  # pragma: no cover - Windows
        kwargs["creationflags"] = getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
    proc = subprocess.Popen(argv, **kwargs)
    try:
        stdout, stderr = proc.communicate(timeout=timeout)
        return proc.returncode, stdout, stderr, False
    except subprocess.TimeoutExpired:
        _terminate(proc)
        stdout, stderr = proc.communicate()
        return proc.returncode, stdout, stderr, True


def _resolve_niki(args: argparse.Namespace) -> list[str]:
    if args.niki:
        # An existing file is one argv token: split only a command line. This
        # keeps a Windows path (`C:\...\niki.exe`) or a path with spaces intact,
        # since `shlex` would eat the backslashes.
        if Path(args.niki).expanduser().is_file():
            return [str(Path(args.niki).expanduser())]
        argv = shlex.split(args.niki)
        if not argv:
            raise SystemExit("--niki is empty")
        executable = argv[0]
        if os.sep in executable or (os.altsep and os.altsep in executable):
            if not Path(executable).is_file():
                raise SystemExit(f"--niki does not exist: {executable}")
        elif shutil.which(executable) is None:
            raise SystemExit(f"--niki is not on PATH: {executable}")
        return argv
    python = args.python or sys.executable
    probe = subprocess.run(
        [python, "-c", "import niki_agent"],
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    if probe.returncode != 0:
        lines = (probe.stderr or probe.stdout).strip().splitlines()
        tail = lines[-1] if lines else "no output"
        raise SystemExit(
            f"{python} cannot import niki_agent ({tail}); "
            "install the wheel into it or pass --niki"
        )
    return [python, "-m", "niki_agent"]


def _tool_result_request(requests: list[dict[str, Any]], target: Path) -> int | None:
    """Index of the first request carrying our tool result back to the model."""
    for index, body in enumerate(requests[1:], start=1):
        messages = body.get("messages")
        if not isinstance(messages, list):
            continue
        for message in messages:
            if not isinstance(message, dict) or message.get("role") != "tool":
                continue
            if message.get("tool_call_id") != TOOL_CALL_ID:
                continue
            content = message.get("content")
            text = content if isinstance(content, str) else json.dumps(content)
            if str(target) in text or "Updated file" in text:
                return index
    return None


def _replayed_tool_call(body: dict[str, Any]) -> bool:
    messages = body.get("messages")
    if not isinstance(messages, list):
        return False
    for message in messages:
        if not isinstance(message, dict) or message.get("role") != "assistant":
            continue
        calls = message.get("tool_calls")
        if not isinstance(calls, list):
            continue
        if any(
            isinstance(call, dict) and call.get("id") == TOOL_CALL_ID for call in calls
        ):
            return True
    return False


def _tail(text: str, limit: int = 4000) -> str:
    text = text.strip()
    if len(text) <= limit:
        return text
    return "...\n" + text[-limit:]


def _print_requests(requests: list[dict[str, Any]]) -> None:
    """Print the message roles of every request, for failure diagnosis."""
    print("--- model requests ---")
    for index, body in enumerate(requests, start=1):
        messages = body.get("messages")
        messages = messages if isinstance(messages, list) else []
        roles = [
            message.get("role") for message in messages if isinstance(message, dict)
        ]
        print(f"  #{index}: {', '.join(str(role) for role in roles)}")
    print()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Run one real niki turn against a loopback mock provider.",
    )
    parser.add_argument(
        "--niki",
        help=(
            "Path to the installed niki launcher, or a full command line such "
            "as 'uv run --project libs/code niki'."
        ),
    )
    parser.add_argument(
        "--python",
        default=None,
        help=(
            "Interpreter with niki-agent installed; runs 'python -m niki_agent' "
            "when --niki is omitted (default: this interpreter)."
        ),
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=DEFAULT_TIMEOUT,
        help=f"Wall-clock limit for the child process (default: {DEFAULT_TIMEOUT:g}s).",
    )
    parser.add_argument(
        "--no-interpreter",
        action="store_true",
        help="Pass the product's own --no-interpreter opt-out to the child.",
    )
    parser.add_argument(
        "--keep-temp",
        action="store_true",
        help="Keep the temp profile and workspace, and print their path.",
    )
    args = parser.parse_args(argv)
    if args.timeout <= 0:
        parser.error("--timeout must be positive")

    niki_argv = _resolve_niki(args)
    root = Path(tempfile.mkdtemp(prefix="niki-smoke-turn-"))
    home = root / "home"
    workspace = root / "workspace"
    cache = root / "cache"
    tmp = root / "tmp"
    for path in (home, workspace, cache, tmp):
        path.mkdir()

    target = workspace / TARGET_FILE
    provider = MockProvider(target)
    provider.start()
    try:
        config_path = _write_config(home, provider.base_url)
        env = _child_env(home, cache, tmp)
        command = list(niki_argv)
        if args.no_interpreter:
            command.append("--no-interpreter")
        command += ["-n", PROMPT]

        print("smoke_turn: one real turn from an installed niki")
        print(f"  niki       : {' '.join(shlex.quote(part) for part in niki_argv)}")
        print(f"  provider   : {MODEL_SPEC} -> {provider.base_url}")
        print(f"  config     : {config_path}")
        print(f"  workspace  : {workspace}")
        print(f"  interpreter: {'disabled' if args.no_interpreter else 'default'}")
        print(f"  timeout    : {args.timeout:g}s")
        print()

        try:
            returncode, stdout, stderr, timed_out = _run(
                command, cwd=workspace, env=env, timeout=args.timeout
            )
        except OSError as exc:
            print(f"FAIL: could not start {command[0]}: {exc}")
            shutil.rmtree(root, ignore_errors=True)
            return 1
        requests = provider.snapshot()
    finally:
        provider.stop()

    try:
        written = target.read_text(encoding="utf-8") if target.exists() else None
    except OSError:
        written = None

    tool_result_index = _tool_result_request(requests, target)
    follow_up = requests[tool_result_index] if tool_result_index is not None else None
    content_ok = written is not None and written.rstrip("\r\n") == EXPECTED_CONTENT

    checks = [
        (
            "child exited 0",
            returncode == 0 and not timed_out,
            "timed out" if timed_out else f"exit {returncode}",
        ),
        (
            f"{TARGET_FILE} written by {TOOL_NAME}",
            content_ok,
            (
                f"{target} ({len(written)} bytes)"
                if written is not None
                else "file is missing"
            ),
        ),
        (
            "model saw the tool result",
            tool_result_index is not None,
            (
                f"request #{tool_result_index + 1} carried {TOOL_CALL_ID}"
                if tool_result_index is not None
                else f"no request carried {TOOL_CALL_ID} back"
            ),
        ),
        (
            "conversation replayed the tool call",
            follow_up is not None and _replayed_tool_call(follow_up),
            "assistant tool_calls present" if follow_up is not None else "no follow-up",
        ),
        (
            "turn ended after the final answer",
            len(requests) == EXPECTED_REQUESTS,
            f"{len(requests)} model request(s), expected {EXPECTED_REQUESTS}",
        ),
    ]

    print()
    for label, passed, detail in checks:
        print(f"  [{'PASS' if passed else 'FAIL'}] {label}: {detail}")
    print()

    ok = all(passed for _, passed, _ in checks)
    if not ok:
        if written is not None and not content_ok:
            print(f"  file content was {written!r}, expected {EXPECTED_CONTENT!r}")
            print()
        print("--- child stdout (tail) ---")
        print(_tail(stdout) or "<empty>")
        print()
        print("--- child stderr (tail) ---")
        print(_tail(stderr) or "<empty>")
        print()
        if requests:
            _print_requests(requests)

    if args.keep_temp:
        print(f"kept: {root}")
    else:
        shutil.rmtree(root, ignore_errors=True)

    print("PASS: the installed niki ran a real turn." if ok else "FAIL: smoke turn")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
