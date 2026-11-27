# Releasing Niki Agent

Niki Agent ships as a self-contained Rust binary in a platform archive. There is
one product and four prebuilt artifacts: `linux-x86_64`, `macos-aarch64`,
`macos-x86_64`, and `windows-x86_64`. Every host with an archive installs with
one command; any other host builds from source.

## Version source

The version lives in `rust/Cargo.toml` under `[workspace.package] version`. Every
crate inherits it, and `niki --version` prints it. The release tag must match:
tag `v2.0.0` publishes archives named `niki-2.0.0-…`.

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

   A tag whose version contains a hyphen (`v2.2.0-beta.1`) publishes a GitHub
   pre-release; `latest` keeps pointing at the newest stable tag.

## What the workflow does

`Niki Release` builds each platform on its own runner with `cargo build --release
--locked -p niki --bin niki`: Linux on `ubuntu-latest`, macOS arm64 on
`macos-26`, macOS x86_64 on `macos-26-intel`, and Windows on `windows-latest`.
The macOS runner labels name an architecture, so each macOS job checks
`uname -m` before building; a label that moved to another host fails the job
instead of publishing a mislabelled archive. It stages the binary with
`install/install.sh`, `install/install.ps1`, `NOTICE`, `LICENSE`, `README.md`,
and `CHANGELOG.md`, then:

- runs the packaged installer into a scratch prefix and prints `niki --version`;
- on Linux and macOS arm64, runs the `niki-smoke` workspace binary against the
  installed binary: one headless turn against a loopback mock provider,
  asserting that a `write_file` tool call wrote the file and the tool result
  reached the model. macOS arm64 gets the same gate because it is the archive
  most macOS users download; the Intel macOS and Windows jobs exercise start-up
  only, and smoke-testing the Intel archive would add a whole macOS runner to
  every release for the same code at a different word size;
- uploads the archive, and the `release` job attaches every archive plus a
  `SHA256SUMS` file to a GitHub Release named `Niki Agent v<version>`, using
  `.github/RELEASE_NOTES_niki.md` as the body. The release step names all four
  archives, so a missing one fails the publish instead of shipping a release
  without it.

A version print alone cannot catch a binary that fails on its first turn, so the
smoke turn is the gate, not `--version`.

The macOS binaries are not notarized. The documented installer downloads with
`curl`, which does not set the quarantine attribute, so the binary runs; a
browser download can trip Gatekeeper, so the one-liner is the supported path.

## Installing a release

Linux, macOS, WSL, and Windows:

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
```

```powershell
irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
```

The installer downloads the archive for the host (`linux-x86_64`,
`macos-aarch64`, `macos-x86_64`, or `windows-x86_64`), verifies it against
`SHA256SUMS`, and installs one executable under `~/.local/niki`
(`%LOCALAPPDATA%\niki` on Windows). It prints `niki --version` on success.

## What a release must include

- One archive per platform-architecture pair (`linux-x86_64`, `macos-aarch64`,
  `macos-x86_64`, `windows-x86_64`), each holding `niki`, both installers,
  `NOTICE`, `LICENSE`, and `README.md`.
The release contains one archive per platform, `SHA256SUMS` covering every
archive, and release notes written for the person installing it, not a changelog
of merged pull requests. Nothing else ships: there is no package registry, no
wheel, and no second artifact line.

## Rolling back

GitHub Releases are immutable enough for this project: delete the release and
tag, then re-tag from the previous commit. There is no fan-out to other
packages, so no version-line coordination is needed.
