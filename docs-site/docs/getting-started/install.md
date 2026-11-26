# Install

Two routes: the release installer, which needs only Python 3.12+ and a terminal,
and a source checkout, which is for working on Niki itself.

## From a release

### Linux and macOS

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
```

### Windows

```powershell
irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
```

### What the installer does

- Creates an isolated environment under `~/.local/niki`
  (`%LOCALAPPDATA%\niki` on Windows) and installs the release wheel into it.
- Verifies the download against the release's `SHA256SUMS`. A missing entry or a
  mismatch is fatal: an unverified archive is never installed.
- Writes a `niki` launcher onto your `PATH`.
- Leaves your shell profile alone. Re-run with `NIKI_SHELL=1` (Linux and macOS)
  or `$env:NIKI_SHELL = '1'` (Windows) to append the `PATH` line.

Choose a different location with `NIKI_PREFIX` on Linux and macOS, or `-Prefix`
on Windows:

```bash
NIKI_PREFIX="$HOME/.local/niki" bash install/install.sh
```

```powershell
.\install.ps1 -Prefix C:\Tools\niki
```

Then confirm the binary:

```bash
niki --version
```

`niki --version` answers without importing the agent runtime, so it proves the
package installed, not that a turn runs.

## From source

Requires Python 3.12+ and [uv](https://docs.astral.sh/uv/).

```bash
git clone https://github.com/RavaniRoshan/niki-agent.git
cd niki-agent/libs/code
uv sync --all-extras
uv run niki
```

`--all-extras` installs the provider and sandbox integrations. Install one
provider's extra instead with, for example, `uv sync --extra anthropic`.

`uv run` uses the project environment with the package installed editable, so a
source change takes effect on the next launch.

### Working on the source

All commands run from `libs/code`:

```bash
make bootstrap     # dependencies plus the local git hooks
make check         # lint, types, imports, tests, and repository gates
```

`make check` is the same gauntlet CI runs. See the repository's `AGENTS.md`
files for contributor conventions.

## Upgrade

Re-run the install command. The installer replaces the environment under
`~/.local/niki` and leaves `~/.niki` — your configuration, sessions, and skills —
untouched.

Inside a session, `/update` checks for and installs updates. The `niki update`
command does the same outside the UI.

## Uninstall

```bash
rm -rf ~/.local/niki          # the application
rm -rf ~/.niki                # configuration, sessions, and skills
```

Remove the `PATH` line from your shell profile if you added one.
