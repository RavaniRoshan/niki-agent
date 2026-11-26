# Contributing to Niki Agent

Thanks for helping. This page covers the local loop: how to run the product, the
checks CI runs, and the conventions a pull request has to follow.

## Set up

Requires Python 3.12+ and [uv](https://docs.astral.sh/uv/).

```bash
git clone https://github.com/RavaniRoshan/niki-agent.git
cd niki-agent/libs/code
uv sync --all-groups
uv run niki            # the product
```

For the Rust client, the docs site, and the marketing site, see the subproject
notes in [`rust/MAP.md`](rust/MAP.md), [`docs-site/README.md`](docs-site/README.md),
and [`site/README.md`](site/README.md).

## Checks

`make check` is the local CI for the Python package. Run it before you open a
pull request:

```bash
cd libs/code
make check
```

It runs Ruff, `ty`, the import boundaries, the command catalogue, the test
suite, and the repository gates (brand, version equality, docs mirrors). CI runs
the same target.

The other surfaces have their own gates:

```bash
cd rust && cargo fmt --all -- --check && cargo clippy -j 2 --workspace --all-targets -- -D warnings && cargo test -j 2 --workspace
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
