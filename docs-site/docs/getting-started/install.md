# Install

Two routes: the release archive, which requires no toolchain, and a source
build, which is for macOS and for working on Niki itself.

## From a release archive

Each release publishes archives named `niki-<version>-<platform>-<arch>.tar.gz`
(Windows: `.zip`) and a `SHA256SUMS` file that covers every archive. Download
the archive and `SHA256SUMS` for the same release, verify the checksum, then
either run the installer inside the archive or put the binary on `PATH`
yourself.

### Linux x86_64

```bash
VERSION=2.0.0
curl -fLO "https://github.com/RavaniRoshan/niki-agent/releases/download/v${VERSION}/niki-${VERSION}-linux-x86_64.tar.gz"
curl -fLO "https://github.com/RavaniRoshan/niki-agent/releases/download/v${VERSION}/SHA256SUMS"
grep "niki-${VERSION}-linux-x86_64.tar.gz" SHA256SUMS | sha256sum -c -
tar -xzf "niki-${VERSION}-linux-x86_64.tar.gz"
```

A missing entry or a mismatch is fatal: do not install an unverified archive.
Then run the installer from the unpacked directory:

```bash
bash "niki-${VERSION}-linux-x86_64/install.sh"
```

The installer puts `niki` in `~/.local/niki/bin` and prints the `PATH` line to
add; `NIKI_PREFIX` chooses another prefix. Run it with `NIKI_SHELL=1` to append
that line to `~/.bashrc` and `~/.zshrc`. When the installer downloads an archive
instead of using one beside it, it verifies the archive against `SHA256SUMS`
first. If you would rather place it yourself:

```bash
install -m 755 "niki-${VERSION}-linux-x86_64/niki" "$HOME/.local/bin/niki"
```

### Windows x86_64

The release publishes a `.zip` archive, but it has not been exercised on a
Windows machine, so treat it as untested. Verify it against `SHA256SUMS`, then
unpack it and put `niki.exe` on `PATH`:

```powershell
$Version = "2.0.0"
Invoke-WebRequest "https://github.com/RavaniRoshan/niki-agent/releases/download/v$Version/niki-$Version-windows-x86_64.zip" -OutFile "niki-$Version-windows-x86_64.zip"
Invoke-WebRequest "https://github.com/RavaniRoshan/niki-agent/releases/download/v$Version/SHA256SUMS" -OutFile "SHA256SUMS"
Get-FileHash "niki-$Version-windows-x86_64.zip" -Algorithm SHA256   # compare with SHA256SUMS
Expand-Archive "niki-$Version-windows-x86_64.zip"
```

### macOS

There is no prebuilt macOS archive. Build from source:

```bash
git clone https://github.com/RavaniRoshan/niki-agent.git
cd niki-agent/rust
cargo build --release -p niki --bin niki
install -m 755 target/release/niki "$HOME/.local/bin/niki"
```

The source build needs Rust 1.85 or newer. `target/release/niki` is the whole
product; nothing else has to be installed beside it.

### Confirm the binary

```bash
niki --version
```

`niki --version` prints the version line and exits before a provider is built,
so it proves the binary runs, not that a turn succeeds.

## `NIKI_HOME`

Configuration, skills, hooks and the session store live under `~/.niki` by
default. Set `NIKI_HOME` to move the whole tree:

```bash
export NIKI_HOME="$HOME/.config/niki"
```

`--config <PATH>` selects a single configuration file without moving the rest of
the tree.

## Upgrade

Download the new archive, verify it, and replace the binary. Your `NIKI_HOME`
tree — configuration, skills, hooks, and `sessions.db` — is left untouched. An
older `config.toml` keeps loading: unknown keys are ignored and both
`[providers.<name>]` and `[models.providers.<name>]` are read.

## Uninstall

```bash
rm -rf "${NIKI_PREFIX:-$HOME/.local/niki}"   # installer prefix
rm -f "$HOME/.local/bin/niki"                # a binary placed by hand
rm -rf "${NIKI_HOME:-$HOME/.niki}"           # configuration, skills, hooks, sessions
```

## From source for development

Requires Rust 1.85 or newer. All commands run from `rust/`:

```bash
git clone https://github.com/RavaniRoshan/niki-agent.git
cd niki-agent/rust
cargo build --release -p niki --bin niki
cargo test --workspace
```

The workspace gates are:

```bash
cd rust
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`cargo test --workspace` runs the unit tests and the headless gate that binds a
mock provider on loopback and runs the binary as a child process. See the
repository's `AGENTS.md` files for contributor conventions.
