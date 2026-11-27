# Launch assets

This directory holds the product-launch material. Nothing here is referenced by
the `niki` binary or by the installers.

| Path | What it is |
| --- | --- |
| `product-hunt.md` | Listing copy, maker comment, FAQ, timing, the MCP submission flow, and the day-of checklist |
| `ph-launch.mjs` | Creates the draft, stages assets, and schedules the launch through the Product Hunt MCP server |
| `PLAN-2.3.0.md` | What the launch side channel ships, in order, and what it leaves out |
| `gallery/01-hero.png` … `05-long-sessions.png` | 1270×760 gallery images |
| `gallery/thumbnail-240.png` | 240×240 listing thumbnail |

## Rebuilding the gallery

The screenshots come from the README recording, and the rendered results are
committed assets. No gallery-rendering script is part of this repository:
regenerate the images outside the tree from the committed `intro.gif`, then
replace the files under `launch/gallery/`. Frame selection for the approval card
and the final answer is a property of that external render.

## Rules for this copy

- Match shipped behaviour. Features under "Known gaps" never appear as shipped.
- No comparison that names another product. Say what Niki does, not what it is
  like.
- The install commands and version numbers must match `install/install.sh` and
  the newest tag.
