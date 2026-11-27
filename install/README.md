# Niki Agent installers

`install.sh` (Linux, WSL and macOS) and `install.ps1` (Windows) place one
release binary under a prefix. They do not build anything and they do not need
a language runtime: they download the archive for the host platform from the
newest GitHub release, verify it against the release's `SHA256SUMS`, unpack
`niki`, and print its `--version`.

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
```

```powershell
irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
```

From a checkout the same script runs locally:

```bash
bash install/install.sh
```

```powershell
.\install.ps1
```

## Environment

| Variable | Effect |
| --- | --- |
| `NIKI_PREFIX` | Install destination (default `~/.local/niki`). The binary lands in `$NIKI_PREFIX/bin`. |
| `NIKI_VERSION` | Release to install, without the leading `v` (default: the newest release). |
| `NIKI_ARCHIVE` | Install from this local archive instead of downloading. |
| `NIKI_SHELL=1` | Append the bin directory to `~/.bashrc` and `~/.zshrc`. Without it the script only prints the `PATH` line. |

`install.ps1` takes `-Prefix` and reads the same version variable.

## What a release contains

| Platform | Archive |
| --- | --- |
| Linux x86_64 (also WSL) | `niki-<version>-linux-x86_64.tar.gz` |
| macOS arm64 | `niki-<version>-macos-aarch64.tar.gz` |
| macOS x86_64 | `niki-<version>-macos-x86_64.tar.gz` |
| Windows x86_64 | `niki-<version>-windows-x86_64.zip` |

Every archive unpacks to a directory holding `niki`, `README.md`, `LICENSE`,
`NOTICE` and both installers. `SHA256SUMS` covers all four archives; the
installer refuses an archive whose checksum does not match.

## Proving the installed binary runs a turn

`niki --version` only prints a version line, so the release workflow runs
[`../.github/scripts/smoke_turn.py`](../.github/scripts/smoke_turn.py) against
the installed binary. It starts a loopback mock model server — no API key, no
external network — runs one headless turn, and asserts that the file-writing
tool ran and that the model saw the tool result:

```bash
python3 .github/scripts/smoke_turn.py --niki "$HOME/.local/niki/bin/niki"
```

The script is stdlib-only and prints a PASS/FAIL line per check. On failure it
also prints the child's stdout and stderr.

- `--timeout SECONDS` — wall-clock limit for the child (default 180).
- `--keep-temp` — keep the throwaway profile and workspace and print the path.

## Building from source instead

macOS on another architecture, or a platform with no archive, builds with two
commands:

```bash
cargo build --release -p niki --bin niki   # from rust/
install -m 0755 target/release/niki ~/.local/niki/bin/niki
```
