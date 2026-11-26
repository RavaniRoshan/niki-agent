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


def test_issue_topic_classifier_uses_dedicated_environment() -> None:
    workflow = _load_workflow("sync_topic_labels.yml")
    job = workflow["jobs"]["classifier-dry-run"]
    assert job["environment"] == "labeling"
    assert "LANGSMITH_API_KEY" not in workflow.get("env", {})
    assert "LANGSMITH_API_KEY" not in job.get("env", {})
    step = _find_step(workflow, job="classifier-dry-run", name="Classify issue")
    assert step["env"]["LANGSMITH_API_KEY"] == "${{ secrets.LANGSMITH_API_KEY }}"
    assert step["env"]["TOPIC_CLASSIFIER_PROVIDER"] == "semif"
