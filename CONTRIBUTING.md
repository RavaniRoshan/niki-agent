# Contributing to Niki Agent

Thanks for helping. This page covers the local loop: how to run the product, the
checks CI runs, and the conventions a pull request has to follow.

## Set up

The product is the Rust workspace in `rust/`. Requires a Rust toolchain (1.85+).

```bash
git clone https://github.com/RavaniRoshan/niki-agent.git
cd niki-agent/rust
cargo build --release -p niki --bin niki
cargo run -q -p niki --bin niki -- --version
```

The Python reference implementation in `libs/code` needs Python 3.12+ and
[uv](https://docs.astral.sh/uv/); it supplies the behavioural fixtures and is not
the shipped client:

```bash
cd libs/code
uv sync --all-groups
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

`make check` in `libs/code` is the gate for the Python reference package (Ruff,
`ty`, import boundaries, tests, and repository gates). Run it when you change
`libs/code`.

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
  `repo`, `rust`, `sdk`, and `site`. A title scope has to cover every package
  directory the PR touches.
- Keep a PR to one concern. Add a test when behaviour a user can see changes.
- New Markdown files need the `ci:ack-markdown` label.
- Never commit a `Co-Authored-By` trailer for an AI tool.

## Language and attribution

User-visible language is the product's own: Niki Agent, not the project it grew
out of. Attribution belongs in [`NOTICE`](NOTICE), [`LICENSE`](LICENSE), and the
`PROVENANCE.md` files beside each vendored directory. The vendored trees under
`libs/code/niki_agent/{sdk,acp_sdk,_vendor}` are excluded from lint on purpose,
so their diff against `PROVENANCE.md` stays auditable.

## Releases

Releases are cut from a `v<version>` tag; the workflow builds the wheel,
assembles the three archives, and publishes the GitHub Release. The steps and
the release-notes template are in [`.github/RELEASING.md`](.github/RELEASING.md).

## Reporting

- Bugs and feature requests: the issue templates.
- Security problems: privately, through
  [GitHub security advisories](https://github.com/RavaniRoshan/niki-agent/security/advisories/new).
  See [`SECURITY.md`](SECURITY.md).
