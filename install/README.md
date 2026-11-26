# Niki Agent installers

`install.sh` (Linux/macOS) and `install.ps1` (Windows) create an isolated
virtual environment under a prefix, install the wheel that ships beside them
(or the newest release asset when piped from `curl`), and write a `niki`
launcher.

```bash
NIKI_PREFIX="$HOME/.local/niki" bash install/install.sh
```

```powershell
.\install.ps1 -Prefix C:\Tools\niki
```

The installer ends by printing `niki --version`. That imports almost nothing, so
it cannot prove the wheel runs a turn. The release workflow follows it with
[`../.github/scripts/smoke_turn.py`](../.github/scripts/smoke_turn.py), which
runs one headless turn against a loopback mock model server — no API key and no
external network — and asserts a tool call wrote a file and the tool result
reached the model.

## Check a locally built wheel

From the repository root, three commands:

```bash
(cd libs/code && uv build --wheel)
python3 -m venv /tmp/niki-smoke && /tmp/niki-smoke/bin/pip install --quiet libs/code/dist/niki_agent-*.whl
python3 .github/scripts/smoke_turn.py --niki /tmp/niki-smoke/bin/niki
```

The script is stdlib-only and prints a PASS/FAIL line per check. If it fails, it
also prints the child's `niki` stdout and stderr, so the real traceback is in the
output. Useful flags:

- `--no-interpreter` — pass the product's `--no-interpreter` opt-out, to tell an
  interpreter-only regression apart from the rest of the turn.
- `--timeout SECONDS` — wall-clock limit for the child (default 180).
- `--keep-temp` — keep the throwaway profile and workspace and print the path.
- `--python PATH` — run `python -m niki_agent` from that interpreter instead of
  a `--niki` launcher (useful for a synced checkout).
