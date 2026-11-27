# Product Hunt launch kit

Everything needed to submit Niki Agent to Product Hunt. Copy below is written to
match shipped behaviour; check it against `CHANGELOG.md` before a launch and
never claim a feature that is still under "Known gaps".

## Listing

**Name:** Niki Agent

**Tagline (48 chars):** Your terminal coding agent, in one Rust binary.

**Short description (under 260 chars):**

> A quiet terminal coding agent in one native Rust binary. Eleven providers,
> sessions on your disk, and an approval gate before shell commands and file writes. No runtime,
> no account, no telemetry.

**Topics:** Developer Tools · Artificial Intelligence · Open Source · Productivity

**Links:**

- Website: https://niki-site.vercel.app
- Docs: https://niki-docs.vercel.app
- GitHub: https://github.com/RavaniRoshan/niki-agent
- Download: https://niki-site.vercel.app/download
- Changelog: https://niki-site.vercel.app/changelog

**Gallery order and captions:**

1. `gallery/01-hero.png` — One native binary. No runtime, no account.
2. `gallery/02-read-before-write.png` — It reads before it writes.
3. `gallery/03-approval-gate.png` — It asks before it acts. That card is the gate.
4. `gallery/04-proof.png` — Then it proves the change: cargo test, two tests green.
5. `gallery/thumbnail-240.png` — Thumbnail.

The gallery is rendered from sessions recorded from the shipped binary against a
loopback model, so the screenshots are the product, not a mock.

## Maker comment

> I built Niki because every coding agent I tried made the same trade: a chat box
> that wants the whole screen, a runtime I had to keep installed, and a trust
> model that amounted to "hope".
>
> Niki is the opposite of that. It is one native executable with no language
> runtime behind it, so startup is a process launch. The screen is quiet: a fresh
> session draws a welcome line and one composer rule, not a box. Sessions, skills,
> and credentials stay in `~/.niki` — no account, no sync, no telemetry. And every
> shell command and file write passes an approval gate that names the exact
> command or path.
>
> It speaks to the model you already pay for: Anthropic, OpenAI, Gemini, Groq,
> OpenRouter, Together, DeepSeek, NVIDIA, Mistral, xAI, and Ollama are built in,
> and any OpenAI-compatible endpoint is one TOML block.
>
> What I would love from you: install it, run one real task, and tell me where
> the approval prompt got in the way — or where it should have appeared and
> didn't. That is the part I care most about getting right.

## First comment

> Install (Linux, WSL, macOS):
>
> ```
> curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
> ```
>
> Windows:
>
> ```
> irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
> ```
>
> Then run `niki`. You do not need an account or a key to look around; add one
> when you want a turn:
>
> ```
> export ANTHROPIC_API_KEY=sk-ant-...
> ```
>
> The docs are at https://niki-docs.vercel.app. Beta builds, known gaps, and how
> to report something are at https://niki-docs.vercel.app/beta. Niki is MIT
> licensed, and the whole product is in the open:
> https://github.com/RavaniRoshan/niki-agent

## FAQ

**Is it free?**
The client is MIT licensed. You pay your model provider directly; Niki has no
subscription and no account.

**What do I need to run it?**
A 64-bit Linux, macOS, or Windows machine, or WSL. No interpreter, Node, or
package manager. Build from source with Rust 1.85+ if your platform has no archive.

**Which models work?**
Eleven providers are built in, including local Ollama. Any OpenAI-compatible
gateway works as a `[providers.<name>]` block in `~/.niki/config.toml`.

**Does it send telemetry?**
No. The only network requests are the ones your provider and MCP servers make.

**Where are my sessions?**
In a SQLite database under `~/.niki`. `NIKI_HOME` moves the whole tree. Nothing
is uploaded.

**Is it a sandbox?**
No, and the docs say so plainly. Approval modes decide what asks before it runs;
they do not constrain what an approved command can do. Use a container or a
dedicated user when you need a hard boundary.

**How is this different from other terminal agents?**
One native binary, a quiet interface, a real approval gate, workspace trust for
project hooks and skills, and sessions you can name, fork, archive, and export.
The README's "Why this one" table is the short version.

**What is still missing?**
In-app trust prompts, `/fork` and `/archive` slash commands, `/ps` and `/stop`,
`/recap`, two-axis permissions, and multi-select questions. The public roadmap
is issue #114.

## Day-of checklist

Before submitting:

- [ ] `niki --version` and the README badge match the newest tag.
- [ ] `CHANGELOG.md` has an entry for that tag.
- [ ] Both install one-liners were run into a fresh `HOME` on Linux.
- [ ] The gallery images are the current UI.
- [ ] The site and docs are deployed and the beta/changelog links resolve.
- [ ] The repo About line and topics match the listing.
- [ ] A test issue submitted through the beta form reaches the right label.

On launch day:

- [ ] Submit after 12:01 a.m. Pacific; the day starts there.
- [ ] Post the maker comment immediately; it is the first thing a visitor reads.
- [ ] Reply to every comment in the first four hours, in the thread, as the maker.
- [ ] Share the listing once on each channel you actually use; no more.
- [ ] Collect every bug report into an issue the same day, with the version line.

After launch:

- [ ] Tag the release notes with what launch week changed.
- [ ] Move shipped roadmap items from "Next" to "Shipped" on issue #114.
- [ ] Update this file if the listing copy changed during the day.

## Timing

Product Hunt opens a launch day at 00:00 Pacific and ranks on the votes and
comments collected while the listing is live. The day you pick matters less than
being present for it: a Tuesday pins you against the strongest weekly slate, and
a Friday wastes the weekend when nobody is browsing.

The plan here is **Tuesday, 2026-12-08, 00:00 America/Los_Angeles** (08:00 UTC).
Submit no later than the Monday before; the server schedules the launch for its
`launchDate`, so an early submission costs nothing and removes launch-morning
risk. If a gate below is not closed by Friday 2026-12-04, move the date to
Tuesday 2026-12-15 rather than launching half-ready — a first launch is the only
one you get on that product.

Gates before the date is booked:

- [ ] The release the listing links to is tagged, and every archive installs.
- [ ] The site, docs, changelog, and download pages are live and cross-linked.
- [ ] The gallery images show the shipped binary, not an earlier build.
- [ ] The Product Hunt account is connected to the MCP server (below) so the
      draft can be created and scheduled without a browser form.

Launch-day clock, Pacific:

| Time | What happens |
| --- | --- |
| 00:00 | Listing goes live. Post the maker comment immediately. |
| 00:00–04:00 | Every comment answered in-thread, by the maker. These hours decide the rank. |
| 11:00 | One share per channel you actually use; no reminders, no vote asks. |
| 23:59 | Day closes. Reply to the last comments the next morning. |

## Submitting through the Product Hunt MCP server

`https://mcp.producthunt.com/mcp` is a streamable-HTTP MCP server with the launch
tools built in. It answers unauthenticated calls with
`Connect your Product Hunt account to use this tool`, so the account has to be
connected once through OAuth; after that the flow below runs unattended.

The tools, in order:

| Step | Tool | What it does |
| --- | --- | --- |
| 1 | `get_launch_context` | Composer limits, whether the date is free, and the account's eligibility. |
| 2 | `start_launch` | Creates the private draft for `https://github.com/RavaniRoshan/niki-agent` (a new product). |
| 3 | `update_launch` | Fills tagline, description, topics, links, and makers from this file. |
| 4 | `prepare_launch_asset` | Stages the thumbnail and gallery from a public URL, then attaches them by asset id. |
| 5 | `submit_launch` | Schedules the draft for its launch date. Two calls: the first returns a preview and a token, the second submits it. |

`launch/ph-launch.mjs` drives those calls over the same endpoint with the bearer
token from `PRODUCT_HUNT_TOKEN`, and reads only until `--submit` is passed:

```bash
export PRODUCT_HUNT_TOKEN=...          # OAuth, scope launches:write
node launch/ph-launch.mjs --check 2026-12-08        # limits and date availability
node launch/ph-launch.mjs --draft values.json       # create the draft
node launch/ph-launch.mjs --assets <id> <url> ...    # stage thumbnail, then gallery
node launch/ph-launch.mjs --submit <id>              # preview, then schedule
```

`values.json` keys must match the `editable_fields` that `--check` returns; an
empty draft is fine and can be filled later. Images are staged from public URLs,
so the gallery has to be on the default branch first:

```
https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/launch/gallery/01-hero.png
```

The draft does not move the date; the server schedules a submitted launch for its
`launchDate`, which is set from the values it already has when `--submit` runs.
