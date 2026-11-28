<!--
  Mirrored from SECURITY.md by docs-site/scripts/sync-docs.mjs.
  Do not edit this copy: edit the source file and run `npm run sync`.
  source-sha256: 1b86e39e15d6a631055eb5ba26d33d823dc6bedae239f807fe9dea02b48c4b62
-->
# Security policy

Niki Agent runs commands and edits files on your machine, so its security
boundary is worth stating plainly. This page describes what the Niki Agent
binary actually does, including the gaps.

## Report a vulnerability

Do not open a public issue. Use GitHub's private reporting flow:

**https://github.com/RavaniRoshan/niki-agent/security/advisories/new**

Include the version (`niki --version`), your platform, the smallest reproduction
you have, and what an attacker gains. If you cannot use GitHub, open a minimal
issue that asks for a private channel and contains no details.

What to expect:

| Stage | Target |
| --- | --- |
| Acknowledgement | 3 business days |
| Triage and severity | 10 business days |
| Fix for high or critical | 30 days |
| Public advisory | after a fixed release ships |

Reporters are credited in the advisory unless they ask not to be.

## Supported versions

| Version | Supported |
| --- | --- |
| 2.2.x | Yes |
| 2.1.x | Security fixes only |
| 2.0.x | No |
| < 2.0 | No |

## What the agent can do

Niki runs shell commands and reads and writes files with your user's
permissions. It is not a sandbox by itself.

- **Manual** asks before every gated action that is not on the shell
  allow-list: a shell command, a write, an edit, a delete, or a mutating MCP
  tool.
- **Auto** runs shell commands that match the allow-list and writes whose path
  resolves inside the workspace; everything else asks.
- **YOLO** runs every action the policy does not refuse, without asking.
- **Built-in writes are gated.** `write_file` and `edit_file` go through the
  same approval callback as a shell command; an approved write rule re-checks
  the workspace boundary on every call.
- **Mutating MCP tools are gated.** A tool whose server does not declare a
  coherent `readOnlyHint` goes through the same approval callback a shell
  command uses. A tool the server marks read-only is not gated.
- **The path boundary is enforced.** Every tool path is resolved against the
  workspace root; `..`, symlinks or an absolute path that leaves the root are
  refused, not clamped.
- **A headless run has nobody to ask.** `niki -n` installs no approval callback,
  so a gated action is reported to the model as a refusal. An ACP session asks
  the editor through `session/request_permission`, and that answer feeds the
  same gate.

For a hard boundary, run Niki inside a container or a dedicated user account,
keep the shell allow-list narrow, and stay in Manual mode. There is no sandbox
flag in this build.

## Trust boundaries

| Boundary | What crosses it |
| --- | --- |
| Model provider | Your prompt, the context Niki assembles, and tool results. |
| MCP servers you configure | They run as you, with your filesystem and network access. |
| Hooks | `hooks.json` commands are shell code and run as you. A project file is read only after `niki trust` records the workspace; review it before granting that. |
| Skills | Instruction files the model reads. A skill cannot execute code by itself, but it can direct the model to call a tool, and those calls meet the same policy gate. |
| Release artifacts | `SHA256SUMS` covers every archive; the installer verifies it before installing. |
| Configuration and keys | Niki reads `config.toml` and reads provider keys from the environment variables it names. It does not write the configuration and does not store keys. |
| Network | The only outbound requests are to the provider endpoint and the MCP endpoints you configure. This build makes no update check, telemetry request, pricing refresh, or remote-configuration fetch. |
| Session data | `sessions.db` holds prompts, answers and tool output under `~/.niki` (or `NIKI_HOME`). Protect that directory like any other transcript. |
| The composer | A draft is cleaned of invisible characters — the tag block, bidi overrides, zero-width spaces, joiners' non-printing neighbours — before it is sent. The cleaned text goes back into the composer with a notice, and the next Enter sends it, so an instruction hidden in a paste is never sent unseen. |

## Out of scope

- Actions you approve in Manual mode, or that a policy you configured allows.
- Vulnerabilities in a model provider's service.
- A malicious MCP server, hook or skill that you installed deliberately.
- Prompt injection that only produces actions your policy already permits;
  report the policy gap instead.

## Known gaps in this build

- No sandbox and no sandbox flag. Approval decides what asks, not what an
  approved command can do.
- There is no separate read-only mode: "what may run" and "what may write" are
  one axis.
- The workspace trust decision is asked once, in the app, when the workspace
  holds project content: "Trust this workspace" writes the same grant `niki
  trust` records, and declining writes nothing so the next launch asks again.
  The question starts on "Not now", so Enter alone never grants. Project hooks
  are loaded at startup, so they begin on the next launch; skills and
  instruction files are recomposed for the next turn. The decision is per
  directory and does not inherit to subdirectories.
- MCP `permissionDecision` values other than `deny` are not implemented; the
  hook runner reports them as warnings.

## Hardening checklist

```bash
niki --version                       # confirm the version you run
niki --mode manual                   # ask before every non-allow-listed command
niki --allow 'git*,cargo*'           # a narrow shell allow-list
```

- Do not set the shell allow-list to `all` outside a throwaway container.
- Review `.niki/hooks.json` and every `.niki/skills/` directory in a repository
  before you run `niki trust` there or answer the app's question. Project hooks,
  project skills and project instruction files are not loaded until that grant
  exists; `niki trust --list` shows what you have allowed and `niki untrust`
  withdraws it.
- Keep provider keys in the environment, not in a file Niki reads.
- Run Niki as a user that cannot write outside the project when you need a hard
  boundary.

---

Source: [`SECURITY.md`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/SECURITY.md) in the Niki Agent repository.
