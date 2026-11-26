"""Static contracts for credential scoping in GitHub workflows."""

from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[4]
WORKFLOWS = ROOT / ".github" / "workflows"
APP_TOKEN_WORKFLOWS = (
    "close_unchecked_issues.yml",
    "pr_labeler.yml",
    "sync_topic_labels.yml",
    "tag-external-issues.yml",
)


def _load_workflow(name: str) -> dict:
    workflow = yaml.safe_load((WORKFLOWS / name).read_text())
    workflow["on"] = workflow.pop(True, workflow.get("on"))
    return workflow


def _find_step(workflow: dict, *, job: str, name: str) -> dict:
    matches = [
        step for step in workflow["jobs"][job]["steps"] if step.get("name") == name
    ]
    assert len(matches) == 1, (
        f"Expected one {name!r} step in {job!r}, got {len(matches)}"
    )
    return matches[0]


def test_github_app_client_id_uses_repository_variable() -> None:
    for name in APP_TOKEN_WORKFLOWS:
        workflow = _load_workflow(name)
        token_steps = [
            step
            for job in workflow["jobs"].values()
            for step in job["steps"]
            if str(step.get("uses", "")).startswith("actions/create-github-app-token@")
        ]
        assert token_steps, f"No GitHub App token step found in {name}"
        for step in token_steps:
            assert step["with"]["client-id"] == (
                "${{ vars.ORG_MEMBERSHIP_APP_CLIENT_ID }}"
            )
            assert step["with"]["private-key"] == (
                "${{ secrets.ORG_MEMBERSHIP_APP_PRIVATE_KEY }}"
            )


def test_openwiki_uses_dedicated_environment_and_token() -> None:
    workflow = _load_workflow("openwiki-update.yml")
    update = workflow["jobs"]["update"]
    assert workflow["permissions"] == {"contents": "read"}
    assert update["environment"] == "openwiki"

    checkout = _find_step(workflow, job="update", name="Check out repository")
    create_pr = _find_step(
        workflow, job="update", name="Create OpenWiki update pull request"
    )
    token_step = _find_step(
        workflow, job="update", name="Generate OpenWiki GitHub App token"
    )
    merge = _find_step(
        workflow, job="update", name="Merge OpenWiki update pull request"
    )
    token = "${{ steps.app-token.outputs.token }}"

    assert "token" not in checkout["with"]
    assert checkout["with"]["persist-credentials"] is False
    steps = update["steps"]
    assert steps.index(token_step) > steps.index(
        _find_step(workflow, job="update", name="Run OpenWiki")
    )
    assert token_step["with"] == {
        "client-id": "${{ vars.OPENWIKI_APP_CLIENT_ID }}",
        "private-key": "${{ secrets.OPENWIKI_APP_PRIVATE_KEY }}",
        "owner": "${{ github.repository_owner }}",
        "repositories": "${{ github.event.repository.name }}",
        "permission-contents": "write",
        "permission-pull-requests": "write",
    }
    assert create_pr["env"] == {
        "GH_TOKEN": token,
        "BRANCH": "openwiki/update",
        "BASE": "main",
    }
    assert "gh auth setup-git" in create_pr["run"]
    assert '-f head="${GITHUB_REPOSITORY_OWNER}:${BRANCH}"' in create_pr["run"]
    assert ".head.repo.full_name == $repository" in create_pr["run"]
    assert 'gh pr close "$pr_number" --delete-branch' in create_pr["run"]
    assert 'gh pr close "$BRANCH"' not in create_pr["run"]
    assert merge["env"] == {
        "GH_TOKEN": token,
        "PR_NUMBER": "${{ steps.create-pr.outputs.number }}",
        "HEAD_SHA": "${{ steps.create-pr.outputs.head-sha }}",
        "EXPECTED_BASE": "main",
        "EXPECTED_HEAD": "${{ github.repository_owner }}:openwiki/update",
    }
    assert merge["if"] == "${{ steps.create-pr.outputs.number != '' }}"


def test_issue_topic_classifier_uses_dedicated_environment() -> None:
    workflow = _load_workflow("sync_topic_labels.yml")
    job = workflow["jobs"]["classifier-dry-run"]
    assert job["environment"] == "labeling"
    assert "LANGSMITH_API_KEY" not in workflow.get("env", {})
    assert "LANGSMITH_API_KEY" not in job.get("env", {})
    step = _find_step(workflow, job="classifier-dry-run", name="Classify issue")
    assert step["env"]["LANGSMITH_API_KEY"] == "${{ secrets.LANGSMITH_API_KEY }}"
    assert step["env"]["TOPIC_CLASSIFIER_PROVIDER"] == "semif"
