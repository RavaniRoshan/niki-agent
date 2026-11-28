# Readiness audit — 2.2.0

Written 2026-11-27 against the shipped v2.2.0 release and the two deployed web
properties. Every claim below was checked by running the command shown, not from
memory. Where a check was not possible, the row says so and names why.

## Verdict

The product is ready, both deployments are ready, and the launch is booked:
Product Hunt launch `1333841` is `scheduled` for 2026-12-08 at 00:00 Pacific.

Vercel Web Analytics is on for both projects, and both sites serve
`/_vercel/insights/script.js` as `200 application/javascript`. The OAuth
connection to `https://mcp.producthunt.com/mcp` is in place, and
`launch/ph-token.mjs` refreshes the stored token so launch-day calls do not need
a browser.

No switch is left open.

## Release and artifacts

| Surface | Check | Result |
| --- | --- | --- |
| Tag | `gh release view v2.2.0` | Tag exists, 5 assets |
| Linux archive | `sha256sum -c` in `/tmp/niki-verify` | OK |
| macOS arm64 archive | `sha256sum -c` | OK, inspected only (never executed) |
| macOS x86_64 archive | `sha256sum -c` | OK, inspected only (never executed) |
| Windows archive | `sha256sum -c` | OK, inspected only (never executed) |
| Binary identity | `niki --version` from the installed tree | `niki 2.2.0` |
| Binary links | `ldd` | Only `libc`, `libm`, `libgcc`, `vdso` |
| Real turn | `cargo run -p niki-smoke -- --niki <installed>` | `PASS: ran one tool turn end to end` |
| Asset reachability | `curl -L` on all five release download URLs | 200 (Windows follows a redirect) |

macOS and Windows archives are built and checksummed but were never executed on
their own platforms. That is a CI-matrix property, not a 2.2.0 defect; the beta
page states it plainly.

## Source gate

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy -j 2 --workspace --all-targets -- -D warnings` | clean |
| `cargo test -j 2 --workspace` | pass |
| Helper-script tests (Node) | 329 pass |
| `git ls-files '*.py'` | 0 files |

## Marketing site — `https://niki-site.vercel.app`

| Route | Status | Title | Canonical |
| --- | --- | --- | --- |
| `/` | 200 | `Niki Agent — a terminal coding agent that stays quiet` | self |
| `/download` | 200 | `Download Niki Agent — one binary, no runtime` | self |
| `/changelog` | 200 | `Niki Agent changelog — every release, in order` | self |

Also live: `/sitemap.xml` (3 URLs, matching the three routes), `/robots.txt`,
`/og.png`. Each route carries its own meta description and Open Graph block,
prerendered at build time by `site/scripts/prerender.mjs`.

The download page lists all four platforms, both installer one-liners, and
`SHA256SUMS` verification steps for `sha256sum`, `shasum`, and `Get-FileHash`.

## Documentation site — `https://niki-docs.vercel.app`

| Route | Status | Title | Canonical |
| --- | --- | --- | --- |
| `/` | 200 | `Niki Agent` | self |
| `/getting-started/install` | 200 | `Install — Niki Agent` | self |
| `/beta` | 200 | `Beta — Niki Agent` | self |

Also live: `/sitemap.xml` (18 URLs, one per page), `/robots.txt`. Every page
shares one meta description, which is the VitePress default; per-page
descriptions are a future improvement, not a launch blocker.

## Cross-linking and onboarding

| Link | Where it points | Status |
| --- | --- | --- |
| Docs front page → download | `https://niki-site.vercel.app/download` | present |
| Docs front page → install | `/getting-started/install` | present |
| Marketing install button | `/download` | present |
| Beta page → feedback form | `issues/new?template=beta-feedback.yml` | present |
| Beta feedback template label | `type:beta` | label exists in the repo |
| `CHANGELOG.md` vs docs changelog | mirror header + source footer only | identical body |
| Installers | `install/install.sh`, `install/install.ps1` on `niki-agent` | both 200 |

## Repository surfaces

| Surface | State |
| --- | --- |
| About description | Names the product and links `https://niki-site.vercel.app` |
| Homepage field | `https://niki-site.vercel.app` |
| Roadmap issue #114 | 2.2.0 comment posted |
| `SECURITY.md` | Declares 2.2.x supported |
| README demo | `intro.gif`, recorded from the 2.2.0 binary, 31.5 s |
| Launch gallery | Four slides rendered from the same recording, OCR-verified |

## Known non-blockers

- Route bodies are client-rendered; only the `<head>` is prerendered. Crawlers
  that execute JavaScript see the full page, and every route has correct
  metadata regardless.
- The repository history begins at the 2.2.0 tree. Earlier development trees are
  not published, and no published commit contains a file from them.
- `libs/` holds about 1.2 GB of local evaluation scratch. It is untracked and
  not shipped.
- The docs site uses one shared meta description for every page.

## Analytics

| Check | Result |
| --- | --- |
| `vercel project web-analytics enable niki-site` | enabled, then redeployed |
| `vercel project web-analytics enable niki-docs` | enabled, then redeployed |
| `curl -I /_vercel/insights/script.js` on both hosts | `200 application/javascript` |
| Script tag in the rendered marketing page | injected by `@vercel/analytics`; a headless Chromium run shows `src="/_vercel/insights/script.js"` in the DOM |
| Script tag in the docs HTML | present in the served markup |

The marketing site injects the tag at runtime from `site/src/main.tsx`; the docs
site has it in the page. Both requests reach the platform route, so page views
are recorded.

## Product Hunt

| Check | Result |
| --- | --- |
| OAuth against `https://mcp.producthunt.com/mcp` | connected as `roshan_ravani_singh` (`8913310`), scopes `mcp:account launches:read launches:write` |
| `get_launch_context` for the repository link | eligible, no blockers; date window 2026-11-28 to 2026-12-29 |
| Draft `U3SR3K6SCALYK3CBK6RG475MQPF7SQ5M7C` | assessment `no_known_blockers` |
| `submit_launch` | outcome `applied`; launch `1333841`, status `scheduled`, 2026-12-08 00:00 Pacific |
| Listing content | name, 47-character tagline, description, maker comment, four gallery images, thumbnail, four links, tags and categories |

The listing can still be edited before launch day at
https://www.producthunt.com/posts/niki-agent/edit.

To drive the server again, mint a token first — the stored one expires after an
hour and the helper refreshes it from the refresh token:

```bash
export PRODUCT_HUNT_TOKEN=$(node launch/ph-token.mjs)
node launch/ph-launch.mjs --show 1333841
```

`launch/product-hunt.md` carries the copy, the timing, and the day-of checklist.
