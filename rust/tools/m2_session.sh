#!/usr/bin/env bash
# M2 gate: drive a live engine session through a PTY and prove its final
# transcript is the committed replay frame.
#
# The engine is the scripted stub in rust/tools/stub-engine (same graph the
# fixture was captured from), so the reply is deterministic. Linux/WSL only:
# `script` from util-linux provides the PTY.
#
#   ./rust/tools/m2_session.sh            # start the stub, run, compare
#   NIKI_ENGINE_URL=http://127.0.0.1:2025 ./rust/tools/m2_session.sh
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
repo="$(cd "$root/.." && pwd)"
bin="${NIKI_BIN:-$root/target/debug/niki}"
url="${NIKI_ENGINE_URL:-http://127.0.0.1:2025}"
fixture="$root/crates/niki-proto/fixtures/hello.json"
work="$(mktemp -d)"
server_pid=""

cleanup() {
  if [ -n "$server_pid" ]; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT

command -v script >/dev/null 2>&1 || {
  echo "m2: util-linux 'script' is required for the PTY capture" >&2
  exit 1
}

if [ ! -x "$bin" ]; then
  echo "m2: building $bin" >&2
  cargo build -j 4 --manifest-path "$root/Cargo.toml" -p niki >/dev/null
fi

if ! curl -fsS -o /dev/null "$url/ok" 2>/dev/null &&
  ! curl -fsS -o /dev/null "$url/docs" 2>/dev/null; then
  echo "m2: starting the stub engine on $url" >&2
  (cd "$here/stub-engine" && "$repo/libs/code/.venv/bin/langgraph" dev \
    --port "${url##*:}" --no-browser >"$work/server.log" 2>&1) &
  server_pid=$!
  for _ in $(seq 1 60); do
    if curl -fsS -o /dev/null "$url/docs" 2>/dev/null; then break; fi
    sleep 1
  done
  if ! curl -fsS -o /dev/null "$url/docs" 2>/dev/null; then
    echo "m2: the stub engine did not start; log follows" >&2
    tail -40 "$work/server.log" >&2
    exit 1
  fi
fi

# The PTY proves the client runs attached to a terminal rather than a pipe.
# A PTY translates LF to CRLF on output; normalize before comparing frames.
script -qec \
  "$bin session --engine $url --prompt 'Say hello' --width 80 --height 24" \
  /dev/null | tr -d '\r' >"$work/live.txt"

"$bin" replay "$fixture" --width 80 --height 24 >"$work/replay.txt"

# The corner label is the only intended difference: `live` vs `replay`.
tail -n +2 "$work/live.txt" >"$work/live.body"
tail -n +2 "$work/replay.txt" >"$work/replay.body"

if ! diff -u "$work/replay.body" "$work/live.body" >"$work/diff.txt"; then
  echo "m2: live transcript differs from the replay frame" >&2
  cat "$work/diff.txt" >&2
  exit 1
fi

if ! grep -q 'Hello from the Niki protocol fixture\.' "$work/live.txt"; then
  echo "m2: the live transcript is missing the assistant reply" >&2
  cat "$work/live.txt" >&2
  exit 1
fi

echo "m2: live session transcript matches the replay frame ($(head -1 "$work/live.txt" | sed 's/ *$//'))"
