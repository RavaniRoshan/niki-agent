# CI credentials and GitHub Actions environments

This document defines the names, purposes, target scopes, and minimum expected permissions for non-`GITHUB_TOKEN` credentials used in this repo.

> [!WARNING]
> Never add credential values, key prefixes, short keys, copied settings output, account identifiers, or individual contacts here. Repository workflows remain the source of truth for which credential names code can consume; verify external GitHub and provider configuration separately.

GitHub automatically provides `GITHUB_TOKEN` to each workflow run. It is not a configured repository secret and is outside the inventory below.

## Design principles

- Use non-human service accounts or project credentials for CI when possible. Do not bind CI to a maintainer's personal API key.
- **LangSmith:** Prefer a single-workspace service key over an organization-scoped key.
  - At the time of writing, existing workflows do not select a workspace with `LANGSMITH_WORKSPACE_ID`
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

### `openwiki`

This environment is selected by `.github/workflows/openwiki-update.yml`.

| Secret | Purpose | Minimum permissions |
| --- | --- | --- |
| `OPENWIKI_APP_PRIVATE_KEY` | Authenticate the dedicated OpenWiki GitHub App so the workflow can mint a short-lived installation token. | GitHub repository contents: read/write; pull requests: read/write |
| `LANGSMITH_API_KEY` | Ingest OpenWiki traces into the `openwiki` project. | LangSmith `runs:create` |
| `LS_GATEWAY_OPENAI_API_KEY` | Invoke the configured model through the workflow's current LangSmith Gateway endpoint. | LangSmith `gateway:invoke`, `workspaces:read` |

| Actions variable | Value | Purpose |
| --- | --- | --- |
| `OPENWIKI_APP_CLIENT_ID` | Dedicated OpenWiki GitHub App client ID | Identify the App installed only on `langchain-ai/deepagents`; do not reuse the shared repository automation App. |
| `OPENAI_BASE_URL` | `https://gateway.smith.langchain.com/openai/v1` | Set LangSmith gateway target. |

Use separate workspace-scoped service keys for tracing and Gateway invocation. The environment should not require reviewers because the workflow runs on a schedule. Restrict deployments to `main`. Install the dedicated OpenWiki App only on this repository and grant it only repository contents and pull requests read/write; the workflow further scopes each installation token to this repository.

### `labeling`

Create this environment for the `label-by-package` job in
`auto-label-by-package.yml`.

| Secret | Purpose |
| --- | --- |
| `GROQ_API_KEY` | Classify issue titles and bodies into topic labels. |

Provision a dedicated Groq inference key for labeling, separate from any
benchmarking credential, with model and spending limits where supported. Store it only in
this environment; the issue workflow injects it only into its classification step.
Do not require reviewers, so automatic labeling can run unattended. Restrict
deployments to the default branch, which the issue workflow uses.

### `integration-tests`

This environment is intentionally uncredentialed. `.github/workflows/integration_tests.yml` retains package-scoped `secrets.*` references, but absent environment credentials resolve to empty strings.

Will expand as needed.

### `release-bot`

This environment supports curated release-note drafting for every release-please
managed package.

| Secret | Condition | Purpose |
| --- | --- | --- |
| `OPENAI_API_KEY` | `RELEASE_BOT_MODEL` starts with `openai:` | Draft structured release notes with OpenAI. |

| Actions variable | Value | Type | Purpose |
| --- | --- | --- | --- |
| `RELEASE_BOT_MODEL` | Variable | Environment | Model used for changelog generation (`provider:model`). Must support JSON Schema structured output and a 32,768-token output ceiling. For `openai:…`, use a Chat Completions model. See [RELEASING.md](RELEASING.md) for the exact list. |
| `RELEASE_BOT_ID` | - | Repository | GitHub App bot account user ID. |
| `RELEASE_BOT_LOGIN` | `langchain-oss-automated-triage[bot]` | Repository | Login ID. |

Of the model-provider credentials, only the credential for the configured provider is required. Prefer a provider project or service-account key limited to model inference, with model allowlists and spend limits where supported. The workflow also uses the repository GitHub App credentials for repository mutations.

### `release`

The environment is retained for release protections and possible future release-time integration coverage. It intentionally has no provider credentials while the `Run integration tests` step in `.github/workflows/release.yml` is disabled with `if: false`.

The disabled step preserves package-scoped wiring so a future re-enablement has an explicit credential contract:

| Release package | Credentials if re-enabled |
| --- | --- |
| `deepagents` | `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `LANGSMITH_API_KEY` |
| `langchain-quickjs` | `ANTHROPIC_API_KEY` |
| `langchain-daytona` | `DAYTONA_API_KEY` |
| `langchain-modal` | `MODAL_TOKEN_ID`, `MODAL_TOKEN_SECRET` |
| `langchain-runloop` | `RUNLOOP_API_KEY` |
| `langchain-vercel-sandbox` | `VERCEL_TOKEN` plus `VERCEL_TEAM_ID` and `VERCEL_PROJECT_ID` Actions variables |

The disabled Vercel step currently references only `VERCEL_TOKEN`; its access-token authentication contract is incomplete. Add the two variables above, or deliberately adopt and document Vercel OIDC, before re-enabling that package's integration test.

## LangSmith permission caveats

The permission lists in this document describe operations required by the currently locked clients. Granular custom roles and service-key role assignment require LangSmith Enterprise RBAC; on other plans these minima are design targets rather than directly assignable roles.

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
actions: write
```

`actions: write` is required by `release.yml`'s `bump-code-sdk-pin` job to dispatch `bump_code_sdk_pin.yml` after a `deepagents` publish. It was added to the App installation as part of [PR #5298](https://github.com/langchain-ai/deepagents/pull/5298).

The App's actual installed permissions are external configuration and must be verified separately. Workflow-level `permissions` restrict `GITHUB_TOKEN`; they do not restrict a separately minted GitHub App installation token.

Several current `actions/create-github-app-token` calls omit explicit `permission-*` inputs and therefore receive the App installation's default permission set. These are known exceptions. Each call should be narrowed to the permissions its job uses.
