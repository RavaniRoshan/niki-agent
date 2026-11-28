# Niki Agent repository guide

Niki Agent is a terminal coding agent. The product is one native executable
built from `rust/`: the turn loop, the eleven built-in providers, the tools,
the policy, sessions, the MCP client, the ACP server, and the full-screen UI all
live in that workspace. Installing the release archive gives you `niki`; nothing
needs a language runtime.

Read [`docs/niki/INDEPENDENCE.md`](docs/niki/INDEPENDENCE.md) for what
"independent" means and [`docs/niki/ARCHITECTURE.md`](docs/niki/ARCHITECTURE.md)
for the crates and the turn loop.

## Layout

- `rust/` — the product. Crates: `niki` (binaries), `niki-agent` (turn loop),
  `niki-provider` (OpenAI-compatible + Anthropic + Google clients),
  `niki-config`, `niki-tools`, `niki-policy`, `niki-hooks`, `niki-skills`,
  `niki-mcp`, `niki-session`, `niki-acp`, `niki-tui-live`, `niki-tui`,
  `niki-render`, `niki-tui-themes`, `niki-engine-client` (the legacy HTTP/SSE
  front end), and the small protocol crates.
- `libs/` — local scratch space. Nothing here is tracked by git and nothing
  here ships.
- `install/` — the release installers, `install.sh` and `install.ps1`. They
  unpack a release archive and place the binary; they do not build anything.
- `docs/niki` — architecture, independence, keymap, and provider guides.
- `site/` — the marketing site (Vite + React + Tailwind, Bun toolchain).
- `launch/` — Product Hunt copy and gallery images. The images are committed
  assets, regenerated outside the repository; nothing here ships in the binary.
- `docs-site/` — the VitePress documentation site, deployed as its own Vercel
  project. It mirrors six repository documents; `npm run sync` refreshes the
  mirrors and `npm run check:brand` gates branding.
- `SECURITY.md` — reporting, supported versions, and trust boundaries.
- `CHANGELOG.md` — the release history readers see; the docs site has its own
  page at `docs-site/docs/changelog.md`.

## Commands

`rust/` is authoritative. Run this before claiming a Rust change is done:

```bash
cd rust
cargo fmt --all -- --check
cargo clippy -j 2 --workspace --all-targets -- -D warnings
cargo test -j 2 --workspace
```

The machine has 8 GB of RAM; keep cargo at `-j 2` or lower. To exercise the
shipped binary end to end, build it and run one turn against a scripted
provider:

```bash
cargo build --release --locked -p niki --bin niki
cargo run -q -p niki-smoke -- --niki rust/target/release/niki
```

```bash
cd site && bun run typecheck && bun run check:brand && bun run build
cd docs-site && npm run sync && npm run check:brand && npm run build
```

## Development rules

- Fix what the task asks for. Do not refactor nearby code or add features.
- Read a file before changing it. Do not propose changes to unread code.
- Add tests only when they protect changed observable behavior.
- Keep user-visible language Niki Agent. Attribution belongs in `NOTICE` and
  `LICENSE`; the version line and help screen do not credit another product.
- `niki` is the only shipped binary name. The older engine client builds as
  `niki-engine` and is not packaged. Do not add compatibility aliases for the
  pre-rename product names.

## Branches and commits

- Branch names: `<github-username>/<scope>/<short-description>`.
- Conventional Commits, with a scope. Allowed scopes are defined in
  `.github/workflows/pr_lint.yml`: `ci`, `deps`, `docs`, `examples`, `infra`,
  `repo`, `rust`, `site`.
- Text after `type(scope):` starts lowercase unless it begins with a proper noun
  or a code entity. Wrap code identifiers in backticks.
- Never create a `main` branch. The default branch is `niki-agent`.

## Releases

Push a `v<version>` tag or dispatch **Niki Release**
(`.github/workflows/niki-release.yml`). It builds `niki` for Linux x86_64, macOS
arm64, macOS x86_64, and Windows x86_64, packs one archive per platform plus
`SHA256SUMS`, runs the installer and one headless turn from the installed binary
on Linux and macOS arm64, and publishes a GitHub Release. See
`.github/RELEASING.md`.

## CI

`.github/workflows/ci.yml` runs the `rust/` gate and the helper-script tests,
and exposes the aggregate check **✅ CI Success**.
`.github/workflows/site.yml` typechecks, brand-checks, and builds `site/`;
`docs-site.yml` does the same for `docs-site/`.
