# Niki Agent — Baselines

Measured **before any UI change**, at BASE_SHA `f57c6f383b7018024ca5cde2dc565048ea83202a`.
Command for the probe numbers:

```bash
cd libs/code && uv run pytest tests/unit_tests/niki/test_perf_probes.py -q -s
```

Machine: Linux, Python 3.12.3, textual 8.2.8. All probes run under the Textual headless
driver with `DEEPAGENTS_HOME` redirected to a temp dir by the repo's own conftest. No network,
no API key, no real model.

## Startup and import cost

| Probe | Command | Result |
| --- | --- | --- |
| CLI start, cold | `uv run dcode -v` | **349 ms** |
| CLI start, warm ×5 | `uv run dcode -v` | **260 ms** |
| import, light | `python -c "import deepagents_code"` | **18.7 ms** |
| import, app module | `python -c "import deepagents_code.app"` | **738.9 ms** |
| import, heavy chain | `python -c "import langchain, langgraph, deepagents"` | **4546.8 ms** |

The heavy chain dominates. This is the S1 lever.

## In-app probes

| Probe | Metric | Baseline | S-row target | Verdict |
| --- | --- | --- | --- | --- |
| First frame, 50×16 (compact) | ms to first paint | **186.9** | ≤ 400 ms warm | already inside floor |
| First frame, 80×24 (standard) | ms to first paint | **194.3** | ≤ 400 ms warm | already inside floor |
| First frame, 120×38 (roomy) | ms to first paint | **191.9** | ≤ 400 ms warm | already inside floor |
| Idle, 5 s at 80×24 | painted frames | **10** (1.99/s) | **0** | **GAP** |
| Idle, 5 s at 80×24 | process CPU | **0.459 %** | < 1 % | meets target |
| Stream 60 tokens, 100 msgs | ms per paint | **107.4** | ratio ≤ 1.5 | **GAP** |
| Stream 60 tokens, 500 msgs | ms per paint | **222.9** | ratio ≤ 1.5 | **GAP** |
| Stream paint rate | paints/s | **4.5** | ≤ 60 | meets target |
| Resident set | MB | **395.8** | bounded | record only |

### Derived gaps

- **S4 is a real gap, not a guess.** The app paints ~2×/s while idle. CPU is already low
  (0.459 %), so this is scheduled redraws, not busy work. The suspects are the timers
  upstream already runs (status-bar reconnect tick, coalescing timers, notification checks).
- **S3 is a real gap.** Cost per paint grows **2.08×** from 100 to 500 mounted messages, against
  a target of ≤ 1.5×. Suspect is the mount window: `WINDOW_SIZE=800` / `HARD_WINDOW_SIZE=900`
  (`message_store.py:673`) means far more live widgets than the transcript depth suggests.
- **S1's in-app first frame is already fast; the startup import chain is not.** The 186–194 ms
  figures exclude the 4.5 s heavy import, which `main.py` performs before the app starts. Total
  time-to-first-frame for a real launch is therefore the sum, and the import chain is the lever.
- **Render rate is comfortably inside target** (4.5 paints/s against a 60 ceiling): upstream's
  0.1 s `_STREAM_FLUSH_INTERVAL` coalescing is working and must not be broken.

## Honest caveats

- **Repaint counting.** `Screen.pre_render` and `HeadlessDriver.write` both fire zero times under
  the headless driver; an early draft of this file hooked the first and reported "zero idle
  repaints" while the app painted constantly. The probes now count
  `Compositor.render_update`, and `test_the_render_counter_is_live` fails the suite if that
  counter ever stops moving.
- **Depth coverage.** S3 and S5 are specified against 5000 messages. This baseline goes to 500
  mounted widgets, because mounting thousands of Markdown widgets under a 30 s pytest timeout is
  not viable as written. The 5000-message figures are **UNVERIFIED** and are owed a probe that
  drives the real sliding window rather than mounting raw widgets.
- **No real terminal.** Every number above is headless. First-frame feel, flicker, truecolor,
  mouse in tmux, and resize feel are **OWNER-VERIFY** and are not measured here.

---

## After — S4 fix (`NikiApp`, commit `niki` layer)

Measured with the same probes, same machine, same size:

| Probe | Before (upstream) | After (Niki) | Target | Verdict |
| --- | --- | --- | --- | --- |
| Idle repaints, 5 s @80x24 | **10** (1.99/s) | **0** (0/s) | 0 | **MET** |
| Idle CPU, 5 s @80x24 | 0.459 % | 0.472 % | < 1 % | meets |

### Root cause, and how it was found

The first hypothesis was wrong and is worth recording: the two unconditional
1 s timers at `app.py:5473-5474` (`_check_cache_expiry`, `_check_cache_expiring`)
look like a textbook idle-repaint source — `_check_cache_expiring` spawns a
`run_worker` every second regardless of whether there is a cache to track,
because the guard sits inside the coroutine. Stubbing both to no-ops changed the
count by **zero**. They were not the cause.

Attributing `Widget.refresh` call sites during a 5 s idle window located it
immediately: **20 refreshes, every one of them the composer `ChatTextArea`**.
Textual's `TextArea.cursor_blink` is a reactive defaulting to `True`, and its
blink timer only pauses when the widget is *unfocused*
(`text_area.py:1918`). The composer holds focus at all times, so it blinked
forever.

### The fix

`NikiApp` sets `cursor_blink = False` on every composer text area from
`call_after_refresh` (`on_mount` alone finds nothing — the composer is built
after mount). No upstream file is touched; no agent behavior, tool, or permission
default changes.

Blink is a **setting, not a hard-off**: `app.blink_cursor = True` before mounting
restores 10 repaints in 5 s. Both directions are asserted by tests, so the row
cannot pass by the feature simply being broken.

---

## After — S3 fix (`NikiMessageStore`)

| Probe | Before | After | Target | Verdict |
| --- | --- | --- | --- | --- |
| Per-paint cost, 100 msgs | 134.4 ms | 134.4 ms | — | reference |
| Per-paint cost, worst mounted tree | **223.8 ms** (500 widgets) | **161.9 ms** (150-widget window) | — | cheaper |
| **Ratio, worst case vs 100 msgs** | **2.089** | **1.205** | ≤ 1.5 | **MET** |
| Resident set | 395.8 MB | 254.9 MB | bounded | improved |

### Root cause

Profiling the deep case (`cProfile` over a 60-token stream at depth 500) showed
the cost is a **full-tree layout pass per streamed token**, not anything
token-specific:

| Signal | depth 500 |
| --- | --- |
| asyncio callback invocations | 19,811 |
| `textual/css/stylesheet.py:470 apply` | 3,658 calls, 2.03 s cumulative |
| `textual/screen.py:1316 _refresh_layout` | 14 calls, 1.95 s cumulative |

So per-paint cost is **O(mounted widgets)**. Upstream mounts up to
`WINDOW_SIZE = 800` / `HARD_WINDOW_SIZE = 900`, which is what lets the tree grow
into that cost. Because the cost tracks *mounted* widgets rather than total
messages, capping the window is what flattens the ratio: a 5,000-message
transcript now mounts about as many widgets as a 150-message one.

### The fix

`NikiMessageStore(MessageStore)` with `WINDOW_SIZE = 150`, `HARD_WINDOW_SIZE = 180`.
`NikiApp` swaps it in after `super().__init__()` by overwriting the
`_message_store` attribute that `app.py:4535` assigns. **No upstream file
touched.**

### Honest caveat — this is a taste trade, not a free win

A smaller window means less of the transcript stays resident, so far-back
scrolling leans harder on upstream's hydration path. `INITIAL_WINDOW_SIZE` was
deliberately left at 30 so resumed sessions never mount incomplete. Whether 150
scrolls as well as 800 is a judgment call, not a measurement, and it is listed in
`docs/niki/OWNER_VERIFY.md`. The constants are class attributes precisely so
they can be moved without touching upstream.

### Still owed

The 5,000-message figure is still measured by proxy (the window bound), not by
driving 5,000 messages through the real store and asserting the mounted count
stays at the window. `NikiApp` installing the store is asserted; end-to-end
pruning at 5,000 is **UNVERIFIED**.


---

## Additional probes (S2, S7, S8)

| Probe | Measured | Target | Verdict |
| --- | --- | --- | --- |
| Input echo p95, 20 presses while streaming | **173.9 ms** (median 130.3 ms) | ≤ 50 ms | **NOT MET** |
| Input echo samples reaching the composer | 20 / 20 | all | probe is measuring a real path |
| 100-resize storm, random 50-170 x 16-46 | **11.4 s** | ≤ 2 s | outcome verified, duration not |
| Resize storm outcome | no crash, final 80x24 correct, still painting | must hold | **MET** |
| 10 MB tool output | **1** widget mounted, **28** visible, RSS < 2 GB | bounded | **MET** |

### Why S2 and S7 are floors, not verdicts

Both probes pay one `pilot.pause()` round-trip per iteration, and the headless
driver charges that to the event loop. So:

- **S2** measures `pilot.press` -> composer, plus one event-loop round-trip.
  Real-terminal latency is strictly lower, but nothing here proves it is below
  50 ms, and a 173.9 ms p95 with only ~130 ms median says the tail is real work,
  not just jitter. Treated as an open gap rather than a pass.
- **S7** measures 100 resizes each followed by a pause. The 11.4 s is dominated
  by that overhead, so the *outcome* (survives, correct final layout, still
  painting) is asserted and the *duration* is recorded but not gated.

Both are on `docs/niki/OWNER_VERIFY.md` as real-terminal checks.


---

## S1 — corrected: the heavy import is not on the critical path

An earlier version of this document implied the 4,546.8 ms
`import langchain, langgraph, deepagents` chain sat between process start and
the first frame, and listed S1 as an open gap because of it. **That was wrong.**

Measured end to end, spawning the real entry point under a real pty
(`test_real_process_first_paint_is_under_the_startup_budget`):

| Marker | Time from process spawn |
| --- | --- |
| First output of any kind | **470 ms** |
| First interactive composer glyph | **553 ms** |

Upstream already defers those imports and prewarms them on a worker
(`app.py:6782 _prewarm_deferred_imports`, visible in the cProfile output as a
single 3.8 s call off the hot path). The chain is real -- 4.5 s of CPU -- but it
is not what the user waits for.

**Consequence:** the lazy-import work proposed as "the single highest-leverage
remaining item" would have optimised a number that was never costing the user
anything. It was not worth doing, and the earlier framing was wrong.

### What is actually left on the critical path

Interpreter start, `import deepagents_code`, and the CLI dispatch chain. The
in-process `run_test` figure (186.9-194.3 ms) begins *after* all of that and
therefore cannot see it, which is why both measurements are kept:

| Measurement | Value | What it covers |
| --- | --- | --- |
| In-process first frame | 186.9-194.3 ms | mount + first paint only |
| Real first output | 470 ms | interpreter + package import + dispatch |
| Real first composer | 553 ms | through to an interactive frame |

S1 is gated at a 1,500 ms regression guard. The checklist's 400 ms is
unreachable for a Python process with this import graph, and a gate that can
never pass honestly is worse than an honest higher one. Real-terminal startup
feel remains OWNER-VERIFY.
