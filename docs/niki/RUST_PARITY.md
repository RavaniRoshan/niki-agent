# Rust client parity record

Phase 3 of the independence plan builds a Rust client that speaks the same
protocol as the Python engine. This file records the three gates — M0 protocol
fixtures, M1 replay parity, M2 live session — with the evidence and every
intended difference from the Python TUI.

The Rust client is a **transcript renderer with a live client**, not a port of
the Textual app. Composer editing, modals, approvals, tool cards, and the
command palette stay in Python until the ratatui screen lands; that is the
M3 scope, and nothing here claims otherwise.

## M0 — protocol fixtures replay deterministically

`rust/crates/niki-proto/fixtures/hello.json` is a real capture, not a
hand-written document:

| Field | Value |
| --- | --- |
| `format` | `niki.protocol.fixture.v1` |
| `name` | `hello` |
| Captured from | `langgraph dev` — `langgraph_api` 0.15.1 / `langgraph` 1.2.12 |
| Graph | the scripted stub in `rust/tools/stub-engine` |
| Normalization | UUIDs replaced in first-appearance order |
| Frames | 16 (metadata, values, messages/metadata, 10× messages/partial, updates, values) |

Re-capture or re-normalize with `rust/tools/capture_protocol.py`:

```bash
# live capture against a running stub engine; also writes hello.sse and
# hello.request.json beside the fixture
python3 rust/tools/capture_protocol.py --url http://127.0.0.1:2025 \
  --name hello --out rust/crates/niki-proto/fixtures/hello.json

# re-normalize a recorded capture (no server needed)
python3 rust/tools/capture_protocol.py \
  --raw rust/crates/niki-proto/fixtures/hello.sse \
  --request rust/crates/niki-proto/fixtures/hello.request.json \
  --name hello --out rust/crates/niki-proto/fixtures/hello.json
```

Determinism is asserted by tests, not by inspection:

- `niki-proto`: `hello_fixture_parses_and_replays_deterministically`
  (two parses compare equal; first frame is `metadata`, last is `values`).
- `niki-proto`: `every_wire_name_round_trips` and
  `unknown_event_is_rejected` — a protocol addition fails loudly instead of
  being silently dropped.
- `niki-engine-client`: `decoder_handles_every_split_point` — the SSE decoder
  is fed the same capture split at every byte boundary and must yield identical
  frames each time. `a_multibyte_character_split_across_chunks_survives` covers
  the UTF-8 case a text-buffered decoder would corrupt.

## M1 — replay frames match the Python TUI

Both clients render the same 80×24 terminal. The Python reference is the
committed snapshot `docs/niki/review/test_first_run_snapshot[80x24].raw`
(Textual's SVG export, text nodes in reading order), rendered by
`tests/unit_tests/niki/test_snapshots.py::test_first_run_snapshot`.

Python, first run with an empty transcript:

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│› niki  v1.0.1 (local)│
└──────────────────────────────────────────────────────────────────────────────┘
▔────────────────────────────────────────────────────────────────────────────▔
›Type a message, / for commands, @ for files
manual ~/projects/nik-agent/libs/code ↗ niki-agent
```

Rust, replaying the `hello` fixture (same 80×24):

```text
Niki Agent                                                               replay
────────────────────────────────────────────────────────────────────────────────
› Say hello

⏺ Hello from the Niki protocol fixture.

ctrl+c cancel · ctrl+p palette · / commands
```

`cargo test -p niki-render parity_frames_are_pinned` pins 120×38, 80×24, and
40×20: header, separator, both turns, and the footer, with every line padded to
exactly the frame width (measured in terminal cells, not bytes).

### Justified differences

| Difference | Why |
| --- | --- |
| Bordered composer box at the top vs. a plain header line | The composer is Python-only until M3. The Rust frame draws the transcript region and a one-line header instead of an input box it cannot accept keystrokes for. |
| Status bar `manual <cwd> ↗ <branch>` | Live git state would make the frame non-deterministic and un-pinnable. The footer carries key hints instead. |
| Transcript body (`›` / `⏺` turns) where Python shows a blank first-run screen | Different state, not a different renderer: Python's first run has no messages. The glyphs and turns match Python's transcript roles. |
| `Niki Agent … replay` header label | The corner label reports the mode (`replay` offline, `live` from an engine) so a captured frame is self-describing. |
| Footer wording | Python's placeholder `›Type a message, / for commands, @ for files` is reused verbatim when the transcript is empty; with messages the Rust footer shows the app-level key hints. Narrow frames drop the `@ for files` / palette segments rather than truncating mid-word. |
| No styling, no spinner, no colour | The Rust client emits plain text frames. Styling is the terminal backend's job in M3; `--ascii` already switches the glyph registry. |
| Body is tail-scrolled and padded below | A transcript taller than the region shows its newest lines, and a shorter body pads so the footer stays on the last row. |

## M2 — scripted PTY session reaches the same transcript

`rust/tools/m2_session.sh` starts the stub engine (if it is not already up),
runs `niki session` **inside a PTY** via `script -qec`, and diffs the live frame
against `niki replay` of the fixture:

```bash
bash rust/tools/m2_session.sh
# m2: live session transcript matches the replay frame (Niki Agent … live)
```

Only the header label differs (`live` vs `replay`); the check compares every
other line, so the assistant text, wrapping, padding, and footer must be
identical. The gate is Linux/WSL only — it depends on util-linux `script`, which
is the PTY provider the plan names.

## Performance

Measured on the development machine (16 cores, Linux, Rust 1.99.0), release
build, 20 runs each, wall clock per process:

| Command | Rust client | Python client (from `docs/niki/BASELINE.md`) |
| --- | --- | --- |
| `niki --version` | **1 ms** | 349 ms cold / 260 ms warm (`dcode -v`) |
| First replay frame, 80×24 | **4 ms** | 194.3 ms to first paint |
| First replay frame, 120×38 | **4 ms** | 191.9 ms to first paint |
| Binary size | 5.5 MB | n/a (interpreter + wheel) |

The comparison is like-for-like only for "start, produce the first frame": the
Python number includes Textual startup, which the Rust client does not have yet.
M3 must move the Rust client to the same feature surface before the speedup can
be quoted as a product claim.

## Repeating the gates

```bash
cd rust
cargo test -j 4 --workspace      # M0 + pinned M1 frames
bash tools/m2_session.sh         # M2, needs libs/code/.venv for the stub engine
```
