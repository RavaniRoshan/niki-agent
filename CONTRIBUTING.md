# Contributing to Niki Agent

Thanks for helping. This page covers the local loop: how to build the product,
the checks CI runs, and the conventions a pull request has to follow.

## Set up

The product is the Rust workspace in `rust/`. Requires a Rust toolchain (1.85+).

```bash
git clone https://github.com/RavaniRoshan/niki-agent.git
cd niki-agent/rust
cargo build --release -p niki --bin niki
cargo run -q -p niki --bin niki -- --version
```

For the docs site and the marketing site, see
[`docs-site/README.md`](docs-site/README.md) and [`site/README.md`](site/README.md).

## Checks

The Rust gate is the local CI for the shipped client. Run it before you open a
pull request:

```bash
cd rust
cargo fmt --all -- --check
cargo clippy -j 2 --workspace --all-targets -- -D warnings
cargo test -j 2 --workspace
```

The other surfaces have their own gates:

```bash
cd site && bun install --frozen-lockfile && bun run typecheck && bun run check:brand && bun run build
cd docs-site && npm ci && npm run check:brand && npm run build
```

`docs-site` mirrors five repository Markdown files; edit the source document and
run `npm run sync` there, never the generated copy.

## Pull requests

- Branch names are `<github-username>/<scope>/<short-description>`.
- Titles follow Conventional Commits: `type(scope): description`. Allowed
  scopes are `ci`, `code`, `deps`, `docs`, `examples`, `infra`, `quickjs`,
  `repo`, `rust`, `sdk`, and `site`. Use `rust` for the workspace, `site` for
  the marketing site, `docs` for the documentation set, and `repo` for
  repository-wide changes. A title scope has to cover every package directory
  the PR touches.
- Keep a PR to one concern. Add a test when behaviour a user can see changes.
- New Markdown files need the `ci:ack-markdown` label.
- Never commit a `Co-Authored-By` trailer for an AI tool.

## Language and attribution

User-visible language is the product's own: Niki Agent, everywhere. Attribution
for third-party code belongs in [`NOTICE`](NOTICE) and [`LICENSE`](LICENSE).
The version line and the help screen do not credit another product, and public
copy does not name a competing agent.

## Releases

Releases are cut from a `v<version>` tag. The workflow builds `niki` for Linux
x86_64, macOS arm64, macOS x86_64, and Windows x86_64, packs one archive per
platform plus `SHA256SUMS`, and publishes the GitHub Release. A version that
contains a hyphen is published as a pre-release. The steps and the release-notes
template are in [`.github/RELEASING.md`](.github/RELEASING.md).

## Where to start

- [`docs/niki/ARCHITECTURE.md`](docs/niki/ARCHITECTURE.md) maps the crates and
  the turn loop.
- The public roadmap is pinned in
  [issue #114](https://github.com/RavaniRoshan/niki-agent/issues/114); items
  labelled `good first issue` are scoped for a first pull request.
- The beta feedback form collects reproducible reports from people running a
  pre-release.

## Reporting

- Bugs and feature requests: the issue templates.
- Security problems: privately, through
  [GitHub security advisories](https://github.com/RavaniRoshan/niki-agent/security/advisories/new).
  See [`SECURITY.md`](SECURITY.md).
