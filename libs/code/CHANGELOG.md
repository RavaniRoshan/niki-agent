<!-- markdownlint-disable MD024 -->

# Niki Agent changelog

What changed in each released version of the product. Each tag also publishes
the same notes on the repository's Releases page.

## 1.1.2 (2026-11-26)

### Fixed

- **A shipped release completes a real turn.** The wheel used to install and
  answer `niki --version`, then fail on the first request because the JS
  interpreter it uses for code execution came from a published package that
  did not match the interface the agent calls. The interpreter now ships inside
  the wheel, so nothing is resolved from outside the product at runtime.
- The provider-table example in the README and the provider guide used
  `[providers.<name>]`, a table the configuration reader ignores; it is now
  `[models.providers.<name>]`.
- The command palette was documented on `Ctrl+K`, which has no binding; it is
  `Ctrl+Shift+P`.

### Changed

- Project configuration is read from `.niki/`. A `.deepagents/` directory is
  still read when it is the only one present and is never written to.
- The README, provider guide, architecture note, independence page, security
  policy, issue forms, and pull-request template are rewritten for the person
  using the agent; unverifiable performance figures are gone.
- The release notes and this changelog describe the product rather than its
  development history.

## Earlier releases

1.0.0 through 1.1.1 established the independent product: one-command installers
for Linux, macOS, Windows, and WSL with checksum verification; the Textual
interface with four contrast-tested themes and three approval modes; and the
first Rust client crates. Their notes remain on the Releases page.
