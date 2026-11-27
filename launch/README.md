# Launch assets

This directory holds the product-launch material. Nothing here is referenced by
the `niki` binary or by the installers.

| Path | What it is |
| --- | --- |
| `product-hunt.md` | Listing copy, maker comment, FAQ, timing, the MCP submission flow, and the day-of checklist |
| `ph-launch.mjs` | Creates the draft, stages assets, and schedules the launch through the Product Hunt MCP server |
| `ph-token.mjs` | Prints a live bearer token, refreshing the stored one from its refresh token |
| `PLAN-2.3.0.md` | What the launch side channel ships, in order, and what it leaves out |
| `READINESS.md` | The 2.2.0 audit: every surface checked, with evidence, and the two open switches |
| `gallery/01-hero.png` … `04-proof.png` | 1270×760 gallery images, rendered from the 2.2.0 recording |
| `gallery/thumbnail-240.png` | 240×240 listing thumbnail |

## Rebuilding the gallery

The screenshots come from the README recording, and the rendered results are
committed assets. No gallery-rendering script is part of this repository:
regenerate the images outside the tree from the committed `intro.gif`, then
replace the files under `launch/gallery/`. Each slide is a 1270×760 canvas: the
terminal frame in a bordered panel, and a kicker, headline and one-line caption
under it. Frame and crop per slide: welcome at 2 s, prompt at 5 s, approval card
at 8 s, test result and answer at 28 s.

## Rules for this copy

- Match shipped behaviour. Features under "Known gaps" never appear as shipped.
- No comparison that names another product. Say what Niki does, not what it is
  like.
- The install commands and version numbers must match `install/install.sh` and
  the newest tag.
