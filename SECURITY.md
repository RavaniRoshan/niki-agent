# Security policy

Niki Agent runs commands and edits files on your machine, so its security
boundary is worth stating plainly.

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
| 1.1.x | Yes |
| 1.0.x | Security fixes only |
| < 1.0 | No |

## What the agent can do

Niki executes shell commands and writes files with your user's permissions. It
is not a sandbox by itself.

- **Manual** asks before every gated action.
- **Auto** runs routine actions and sends anything uncertain to a classifier;
  a failed review still asks.
- **YOLO** runs what the policy allows without asking. The first switch shows a
  one-time acknowledgement per state directory, and `startup.yolo_switcher` can
  keep it out of the cycle.

For a hard boundary, use `--sandbox` with a sandbox backend, restrict the shell
allow-list, or run Niki inside a container or a dedicated user account.

## Trust boundaries

| Boundary | What crosses it |
| --- | --- |
| Model provider | Your prompt, the context Niki assembles, and tool results. |
| MCP servers you configure | They run as you, with your filesystem and network access. |
| Skills and hooks | Local code. Read one before you install it. |
| Release artifacts | `SHA256SUMS` covers every archive; the installers verify it before installing. |
| Update and pricing checks | A version request to `pypi.org`, an hourly model-pricing catalog refresh, and an optional self-update. Turn them off with `NIKI_DISABLE_UPDATE_CHECK=1`, `NIKI_DISABLE_AUTO_UPDATE=1`, `NIKI_PRICES_AUTO_UPDATE=0`, or the matching `[update]` settings. A remote `managed_config.toml` is fetched only when a deployment configures one; `NIKI_DISABLE_REMOTE_CONFIG=1` blocks it. |
| Configuration | `~/.niki/config.toml` is written `0600`, and credentials stored by `/auth` live in `~/.niki/.state/auth.json` with the same mode. A system administrator can pin policy in `/etc/niki/managed_config.toml`, which the user configuration cannot override. |

## Out of scope

- Actions you approve in Manual mode, or that a policy you configured allows.
- Vulnerabilities in a model provider's service.
- A malicious MCP server, skill, or hook that you installed deliberately.
- Prompt injection that only produces actions your policy already permits;
  report the policy gap instead.

## Hardening checklist

```bash
niki --version                          # confirm the version you run
ls -l ~/.niki/config.toml               # expect -rw------- (0600)
ls -l ~/.niki/.state/auth.json          # expect -rw------- (0600)
niki doctor                             # environment and managed-policy report
```

Keep `NIKI_SHELL_ALLOW_LIST` narrow. Setting it to `all` disables pattern checks
and is intended for throwaway containers.
