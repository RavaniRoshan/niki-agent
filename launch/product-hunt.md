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
- Install: https://github.com/RavaniRoshan/niki-agent#install

**Gallery order and captions:**

1. `gallery/01-hero.png` — A quiet coding agent, in one native binary.
2. `gallery/02-one-binary.png` — One archive, one executable, one command.
3. `gallery/03-approval-gate.png` — It asks before it acts. Manual, Auto, YOLO.
4. `gallery/04-providers.png` — Your model, your files, one TOML block.
5. `gallery/05-long-sessions.png` — Built for the long session.
6. `gallery/thumbnail-240.png` — Thumbnail.

The gallery is rendered from the README recording by `render_gallery.py`, so the
screenshots are the product, not a mock.

## Maker comment

> I built Niki because every coding agent I tried made the same trade: a chat box
> that wants the whole screen, a runtime I had to keep installed, and a trust
> model that amounted to "hope".
>
> Niki is the opposite of that. It is one native executable with no language
> runtime behind it, so startup is a process launch. The screen is quiet: an
> idle session draws nothing, and the composer is a single rule. Sessions, skills,
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
A 64-bit Linux, macOS, or Windows machine, or WSL. No Python, Node, or package
manager. Build from source with Rust 1.85+ if your platform has no archive.

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
- [ ] The gallery images are the current UI (`python3 launch/render_gallery.py`).
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
