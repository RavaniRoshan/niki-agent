# Niki Agent — Progress Log

## Ground rules
- SDK package `libs/deepagents` is **never** modified. Final `git diff BASE_SHA -- libs/deepagents` must be empty.
- Local commits only unless the owner asks otherwise. No push without instruction.
- Rebrand user-visible surfaces only. `dcode` untouched; new `niki` entry point.
- Every touched upstream file logged in `docs/niki/UPSTREAM_DIFF.md`.
- A row is WORKS only with a passing test or measured probe. Unverified is never reported as done.

## BASE_SHA
`f57c6f383b7018024ca5cde2dc565048ea83202a`
Clone: `https://github.com/RavaniRoshan/deepagents.git` → `/home/shiva/projects/nik-agent`, pinned detached at BASE_SHA, branched `niki-agent`.
`libs/code` at BASE_SHA: `deepagents-code` 0.1.80, SDK `deepagents` 0.7.21, textual 8.2.8, Python 3.12.3.

## Phase 0 — design note (COMPLETE)
Bootstrap, contributor docs read, subsystems mapped, baselines measured, `docs/niki/DESIGN.md` written (35 lines).
Confirmed the current terminal agent is `libs/code` (`deepagents-code` 0.1.80, module `deepagents_code`); `deepagents-cli` no longer exists.

## Phase 1 — harness (COMPLETE)
Fixture model, Pilot driver, snapshots at four tiers, PTY lifecycle, perf probes, lint rules. 179 tests at that point.

## Session 2 — polish, palette, safety, providers

### Completed
- Approval prompt no longer opens focused on `Approve`; it follows the approval mode (manual → Reject). Needed an upstream patch after proving extension was impossible.
- **Security fix:** `ErrorMessage.render()` passed bodies through unstripped — an OSC 52 clipboard write in an error body could reach the user's clipboard. Now sanitised, newlines preserved.
- Palette v2 (cool, flat greys, one accent) plus `niki-light`, `niki-contrast`, `niki-dim`. Four registered variants, all 0 contrast violations.
- Anti-copy test: Niki reuses **none** of the 20 recorded hex values from Codex or Kimi Code.
- Motion language: `◐◓◑◒` ping-pong, adaptive cadence, `NIKI_REDUCED_MOTION` / `NO_MOTION`.
- Composer: four-sided box → single rule, matching "at most one border level".
- V14, K9, K10, F5, M4 verified. K11 and F6 characterised as genuinely unbuilt.
- `docs/niki/PROVIDERS.md` and `scripts/gen_readme.py` (README generated from live facts).

### Findings worth keeping
1. **The L4 suite tested the sanitizer, not the call site.** It passed while `ErrorMessage.render` shipped an unstripped OSC 52. A security test on a helper is not a security test on where untrusted data goes.
2. **The `ApprovalMenu` subclass was a silent no-op.** `app.py` imports it *inside functions*, so there was no module attribute to rebind. Only a test exposed it.
3. **Two error-colour tests asserted the wrong thing** — matching Niki's token when Textual derives `$error` through its own scale. Replaced with a two-theme comparison proving "themed, not hardcoded".
4. **Composer styling that does not apply fails silently.** An undefined `$niki-background` made the whole sheet unparseable while every snapshot still passed.

### Owner decisions recorded
- Network: **match upstream** (update check and auto-update ON), with `NIKI_DISABLE_*` opt-outs.
- Approval: focus the option matching the current mode.
- Mount window: **keep 150**; owner verifies scrolling.
- Palette: direction-only from Codex and Kimi Code, original values.
- Push after each slice.

### Outstanding
- **Owner-gated:** auto-update sign-off was implemented as instructed and still wants explicit confirmation; a real API key; real-terminal verification via `docs/niki/OWNER_VERIFY.md`.
- **Not built:** K11 command palette, F6 post-run summary, mouse-release toggle, 16-colour and light-terminal appearance, and every on-screen rendering row a synthetic mount cannot decide (layout height 0).
- **Open gaps by design:** PgUp does not reach the transcript while the composer holds focus; `Esc` still interrupts rather than denying; no reduced-motion redraw cost regression.