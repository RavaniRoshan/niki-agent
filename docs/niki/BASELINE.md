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