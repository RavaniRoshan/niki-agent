# CI credentials and GitHub Actions environments

This document defines the names, purposes, target scopes, and minimum expected permissions for non-`GITHUB_TOKEN` credentials used in this repo.

> [!WARNING]
> Never add credential values, key prefixes, short keys, copied settings output, account identifiers, or individual contacts here. Repository workflows remain the source of truth for which credential names code can consume; verify external GitHub and provider configuration separately.

GitHub automatically provides `GITHUB_TOKEN` to each workflow run. It is not a configured repository secret and is outside the inventory below.

## Design principles

- Use non-human service accounts or project credentials for CI when possible. Do not bind CI to a maintainer's personal API key.
- Store credentials in the narrowest GitHub environment that needs them.
- Inject a provider credential only into the runtime step and package job that consumes it.
- Track any temporary exception to these rules explicitly and remove it when its blocker is resolved.

For same-named Actions secrets available to the repository, precedence is environment, then repository, then organization. Audit broader repository and organization scopes before deleting an environment secret; deleting it may expose a same-named broader credential. Organization-secret repository visibility also controls whether fallback is possible.

Repository and organization secrets are read when a workflow run is queued. Environment secrets are read when the referencing job starts. The `vars` and `secrets` namespaces are independent: a same-named variable does not override a secret.

## Target GitHub configuration

The sections below describe target configuration. Selecting `environment:` in workflow YAML does not prove that its secrets, protection rules, or branch policy have been configured.

### Repository scope

| Name | Kind | Purpose |
| --- | --- | --- |
| `ORG_MEMBERSHIP_APP_CLIENT_ID` | Actions variable | Identifies the GitHub App used by repository automation. |
| `ORG_MEMBERSHIP_APP_PRIVATE_KEY` | Secret | Authenticates the GitHub App so workflows can mint short-lived installation tokens. |

### `labeling`

This environment is selected by `.github/workflows/sync_topic_labels.yml`
for the scheduled topic-label sync.

| Secret | Purpose |
| --- | --- |
| `GROQ_API_KEY` | Classify issue titles and bodies into topic labels. |

Provision a dedicated Groq inference key for labeling, separate from any
other credential, with model and spending limits where supported. Store it
only in this environment; the workflow injects it only into its
classification step. Do not require reviewers, so labeling can run
unattended. Restrict deployments to the default branch.

### Retired environments

`integration-tests`, `release-bot`, and `release` belonged to an earlier
release pipeline (`release.yml`, `release_please*.yml`, `check_sdk_pin.yml`,
`check_release_deps.yml`, `release_notes_check.yml`, `integration_tests.yml`)
that this repository no longer contains. `niki-release.yml` needs nothing beyond
`GITHUB_TOKEN`. Archive the environments in GitHub if they still exist; nothing
here reads them.

## GitHub App credentials

Repository automation uses `actions/create-github-app-token` with:

```yaml
client-id: ${{ vars.ORG_MEMBERSHIP_APP_CLIENT_ID }}
private-key: ${{ secrets.ORG_MEMBERSHIP_APP_PRIVATE_KEY }}
```

Current workflow operations require the App installation to provide at least:

```text
organization members: read
repository contents: write
issues: write
pull requests: write
```

`actions: write` is no longer required: the job that dispatched
`bump_code_sdk_pin.yml` left with that earlier pipeline.

The App's actual installed permissions are external configuration and must be verified separately. Workflow-level `permissions` restrict `GITHUB_TOKEN`; they do not restrict a separately minted GitHub App installation token.

Several current `actions/create-github-app-token` calls omit explicit `permission-*` inputs and therefore receive the App installation's default permission set. These are known exceptions. Each call should be narrowed to the permissions its job uses.
