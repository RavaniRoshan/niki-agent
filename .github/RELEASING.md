# Releasing Niki Agent

Niki Agent ships as a self-contained Rust binary in a platform archive. There is
one product and two prebuilt artifacts: `linux-x86_64` and `windows-x86_64`.
macOS builds from source; the installer says so on Darwin.

## Version source

The version lives in `rust/Cargo.toml` under `[workspace.package] version`. Every
crate inherits it, and `niki --version` prints it. The release tag must match:
tag `v2.0.0` publishes archives named `niki-2.0.0-…`.

The Python package in `libs/code` keeps its last released version (1.1.2) and is
not part of a release. It is the reference implementation and the fixture source.

## Cutting a release

1. Bump `[workspace.package] version` in one commit: `release(rust): 2.0.0`.
2. Run the Rust gate in `rust/`:

   ```bash
   cargo fmt --all -- --check
   cargo clippy -j 4 --workspace --all-targets -- -D warnings
   cargo test -j 4 --workspace
   ```

3. Merge to `niki-agent`.
4. Tag the merge commit and push the tag:

   ```bash
   git tag v2.0.0
   git push origin v2.0.0
   ```

   Or dispatch **Niki Release** with the version (no leading `v`).

## What the workflow does

`Niki Release` builds each platform on its own runner with `cargo build --release
--locked -p niki --bin niki`. It stages the binary with `install/install.sh`,
`install/install.ps1`, `NOTICE`, `LICENSE`, and `README.md`, then:

- runs the packaged installer into a scratch prefix and prints `niki --version`;
- on Linux, runs `.github/scripts/smoke_turn.py` against the installed binary:
  one headless turn against a loopback mock provider, asserting that a
  `write_file` tool call wrote the file and the tool result reached the model;
- uploads the archive, and the `release` job attaches every archive plus a
  `SHA256SUMS` file to a GitHub Release named `Niki Agent v<version>`, using
  `.github/RELEASE_NOTES_niki.md` as the body.

A version print alone cannot catch a binary that fails on its first turn, so the
smoke turn is the gate, not `--version`.

## Installing a release

Linux/WSL and Windows:

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
```

```powershell
irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
```

The installer downloads the platform archive, verifies it against `SHA256SUMS`,
and installs one executable under `~/.local/niki` (`%LOCALAPPDATA%\niki` on
Windows). It prints `niki --version` on success.

## What a release must include

- The platform archive: `niki`, both installers, `NOTICE`, `LICENSE`, `README.md`.
- `SHA256SUMS` covering every archive.
- Release notes written for the person installing it, not a changelog of
  merged pull requests.

The Python tree stays in the repository. It is not packaged, not uploaded, and
not installed by any release path.

## Rolling back

GitHub Releases are immutable enough for this project: delete the release and
tag, then re-tag from the previous commit. There is no fan-out to other
packages, so no version-line coordination is needed.
