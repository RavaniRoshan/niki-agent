# 2.3.0 — the launch side channel

2.2.0 is the last release that has to exist before a Product Hunt launch: the
binary, the four archives, the site, the docs site, and the release notes are
shipped and verified. Everything that remains is around the product rather than
inside it, and it is what this plan calls the side channel. It lands as 2.3.0 so
the launch note and the release note are the same document.

Nothing here changes the turn loop. If a change touches `rust/`, it belongs in a
normal release, not this one.

## Order

The launch date is Tuesday 2026-12-08, 00:00 Pacific. Each step below names the
gate that closes it.

| When | Step | Closed when |
| --- | --- | --- |
| Now | Connect the Product Hunt account to `https://mcp.producthunt.com/mcp` and export `PRODUCT_HUNT_TOKEN` | `node launch/ph-launch.mjs --check 2026-12-08` returns composer limits instead of `invalid_token` |
| By 2026-12-04 | Create the draft and fill it from `launch/product-hunt.md` | `--show <id>` lists tagline, description, topics, links, and makers correctly |
| By 2026-12-04 | Re-render the gallery from the 2.2.0 binary and stage it | The staged images match what the shipped binary draws today — `02-one-binary.png` currently reads `2.1.3` and every slide predates the 2.2.0 welcome line |
| 2026-12-07 | Submit for the 12-08 date | `--submit <id>` returns the scheduled launch, and the editor shows the date |
| 2026-12-08 | Run the day-of checklist in `launch/product-hunt.md` | Every box ticked, comments answered in-thread |
| 2026-12-09 | Write down what launch week changed | A 2.3.0 section in `CHANGELOG.md` and the release notes |

## What 2.3.0 ships besides the launch

- **Beta onboarding.** The beta page, the feedback form, and the issue template
  agree on one version line and one label, so a report arriving from the listing
  lands in the same place as one from the docs.
- **Analytics readback.** Vercel Analytics records page views once the dashboard
  toggle is on; the first week's numbers go into the launch retrospective, not
  into the site.
- **The download page carries every platform**, including the install one-liners
  and the checksum file, with the Windows path stated plainly rather than left
  to the reader.
- **Docs front page points at the download page**, and the changelog exists in
  both places with identical text.

## What 2.3.0 does not include

- Interactive trust prompt, `/fork`, `/archive`, `/ps`, `/stop`, `/recap`,
  two-axis permissions, multi-select questions. These are issue #114 items and
  they need engine work.
- A sandbox. The approval gate decides what asks; it does not constrain what an
  approved command can do, and the docs say so.
- Any change to the model list or provider behaviour.

## Stop rule

The side channel is done when the listing is live, the checklist is complete,
and the 2.3.0 note describes what actually changed rather than what was planned.
Until then, this file is the plan; afterwards, it is a record.
