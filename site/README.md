# Niki Agent marketing site

The single-page marketing site for Niki Agent: Vite, React 19, TypeScript,
Tailwind CSS v4, GSAP scroll choreography, and a Three.js hero shader. The copy
describes the product; the visual contract is [`DESIGN.md`](DESIGN.md).

## Run it locally

Requires [Bun](https://bun.sh); the lockfile is `bun.lock`.

```bash
cd site
bun install
bun run dev      # http://localhost:3000
```

## Build, check, and deploy

```bash
bun run build       # static output in dist/, with hashed assets
bun run preview     # serve the built output
bun run typecheck   # tsc --noEmit; `bun run lint` runs the same check
bun run check:brand # fail on lineage branding in src/, index.html, DESIGN.md
```

`build` does not run `check:brand` — CI runs both. The brand check bans any
upstream project name in page copy and assets; there is no exception for a
configuration snippet.

Vercel builds this directory with `bun run build` and serves `dist/`;
`vercel.json` carries the SPA rewrite to `index.html`. The Vercel project's
**Root Directory must be `site`**.
