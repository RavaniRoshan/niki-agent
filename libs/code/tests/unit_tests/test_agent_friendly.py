"""Tests for agent-friendly CLI improvements.

Covers: --dry-run, idempotency, --stdin, error messages, agents subcommand,
update subcommand, and help screen examples.
"""

import asyncio
import io
import json
import sys
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest
from rich.console import Console

from niki_agent.ui import show_help

# ---------------------------------------------------------------------------
# Section 1: Help screen Examples sections
# ---------------------------------------------------------------------------


class TestHelpScreenExamples:
    """Verify Examples sections exist in all subcommand help screens."""

    @staticmethod
    def _render(fn: object) -> str:
        buf = io.StringIO()
        test_console = Console(file=buf, highlight=False, width=200)
        with patch("niki_agent.ui.console", test_console):
            fn()  # ty: ignore
        return buf.getvalue()


# ---------------------------------------------------------------------------
# Section 2: --dry-run for reset
# ---------------------------------------------------------------------------


class TestResetDryRun:
    """Tests for deepagents reset --dry-run."""

    def test_dry_run_text_no_mutation(self, tmp_path: Path) -> None:
        """--dry-run should not remove the agent directory."""
        from niki_agent.agent import reset_agent

        agent_dir = tmp_path / "coder"
        agent_dir.mkdir()
        (agent_dir / "AGENTS.md").write_text("original")

        buf = io.StringIO()
        test_console = Console(file=buf, highlight=False, width=200)
        with (
            patch("niki_agent.agent.credentials") as mock_settings,
            patch("niki_agent.agent.console", test_console),
            patch(
                "niki_agent.agent.get_default_coding_instructions",
                return_value="default",
            ),
        ):
            mock_settings.user_niki_dir = tmp_path
            reset_agent("coder", dry_run=True)

        assert agent_dir.exists()
        assert (agent_dir / "AGENTS.md").read_text() == "original"
        output = buf.getvalue()
        assert "Would" in output
        assert "No changes made" in output

    def test_dry_run_json(self, tmp_path: Path) -> None:
        """--dry-run --json should include dry_run: true."""
        from niki_agent.agent import reset_agent

        agent_dir = tmp_path / "coder"
        agent_dir.mkdir()
        (agent_dir / "AGENTS.md").write_text("original")

        stdout_buf = io.StringIO()
        with (
            patch("niki_agent.agent.credentials") as mock_settings,
            patch("niki_agent.agent.console"),
            patch(
                "niki_agent.agent.get_default_coding_instructions",
                return_value="default",
            ),
            patch("sys.stdout", stdout_buf),
        ):
            mock_settings.user_niki_dir = tmp_path
            reset_agent("coder", dry_run=True, output_format="json")

        result = json.loads(stdout_buf.getvalue())
        assert result["data"]["dry_run"] is True
        assert agent_dir.exists()


# ---------------------------------------------------------------------------
# Section 2b: --dry-run for threads delete
# ---------------------------------------------------------------------------


class TestThreadsDeleteDryRun:
    """Tests for deepagents threads delete --dry-run."""


# ---------------------------------------------------------------------------
# Section 3: agents subcommand
# ---------------------------------------------------------------------------


class TestAgentsSubcommand:
    """Tests for the agents resource subcommand."""


# ---------------------------------------------------------------------------
# Section 4: update subcommand
# ---------------------------------------------------------------------------


class TestUpdateSubcommand:
    """Tests for the update subcommand."""


# ---------------------------------------------------------------------------
# Section 5: Idempotency
# ---------------------------------------------------------------------------


class TestSkillsCreateIdempotency:
    """Skills create should be a no-op if skill already exists."""

    def test_already_exists_text_no_error(self, tmp_path: Path) -> None:
        """Re-creating an existing skill should print informational msg, not error."""
        from niki_agent.skills.commands import _create

        skill_dir = tmp_path / "my-skill"
        skill_dir.mkdir()
        (skill_dir / "SKILL.md").write_text("existing")

        buf = io.StringIO()
        test_console = Console(file=buf, highlight=False, width=200)
        mock_settings = MagicMock()
        mock_settings.project_root = None
        mock_settings.ensure_user_skills_dir.return_value = tmp_path
        with (
            patch("niki_agent.config.Credentials") as settings_cls,
            patch(
                "niki_agent.skills.commands.ensure_user_skills_dir",
                return_value=tmp_path,
            ),
            patch("niki_agent.config.console", test_console),
        ):
            settings_cls.from_environment.return_value = mock_settings
            _create("my-skill", "agent")

        output = buf.getvalue()
        assert "Error" not in output
        assert "already exists" in output

    def test_already_exists_json(self, tmp_path: Path) -> None:
        """Re-creating an existing skill in JSON mode returns already_existed."""
        from niki_agent.skills.commands import _create

        skill_dir = tmp_path / "my-skill"
        skill_dir.mkdir()
        (skill_dir / "SKILL.md").write_text("existing")

        stdout_buf = io.StringIO()
        mock_settings = MagicMock()
        mock_settings.project_root = None
        mock_settings.ensure_user_skills_dir.return_value = tmp_path
        with (
            patch("niki_agent.config.Credentials") as settings_cls,
            patch(
                "niki_agent.skills.commands.ensure_user_skills_dir",
                return_value=tmp_path,
            ),
            patch("niki_agent.config.console"),
            patch("sys.stdout", stdout_buf),
        ):
            settings_cls.from_environment.return_value = mock_settings
            _create("my-skill", "agent", output_format="json")

        result = json.loads(stdout_buf.getvalue())
        assert result["data"]["already_existed"] is True


class TestThreadsDeleteIdempotency:
    """Threads delete should be informational (not error) when thread not found."""

    def test_not_found_not_red(self) -> None:
        """Not-found message should not contain Error prefix."""
        from niki_agent import sessions

        buf = io.StringIO()
        test_console = Console(file=buf, highlight=False, width=200)
        with (
            patch.object(
                sessions,
                "delete_thread",
                new_callable=AsyncMock,
                return_value=False,
            ),
            patch("niki_agent.config.console", test_console),
        ):
            asyncio.run(sessions.delete_thread_command("missing"))

        output = buf.getvalue()
        assert "Error" not in output
        assert "not found or already deleted" in output


# ---------------------------------------------------------------------------
# Section 6: --stdin flag
# ---------------------------------------------------------------------------


class TestStdinFlag:
    """Tests for --stdin explicit flag."""


# ---------------------------------------------------------------------------
# Section 7: Error messages with corrective hints
# ---------------------------------------------------------------------------


class TestErrorMessageHints:
    """Tests for corrective hints in error messages."""

    def test_reset_source_not_found_has_hint(self, tmp_path: Path) -> None:
        """Reset with missing source agent should suggest agents list."""
        from niki_agent.agent import reset_agent

        buf = io.StringIO()
        test_console = Console(file=buf, highlight=False, width=200)
        with (  # separate to satisfy PT012
            patch("niki_agent.agent.credentials") as mock_settings,
            patch("niki_agent.agent.console", test_console),
        ):
            mock_settings.user_niki_dir = tmp_path
            with pytest.raises(SystemExit):
                reset_agent("coder", "nonexistent")

        output = buf.getvalue()
        assert "dcode agents list" in output


# ---------------------------------------------------------------------------
# Drift detection: new flags in argparse should appear in help screens
# ---------------------------------------------------------------------------


class TestHelpScreenDriftExtended:
    """Extended drift detection for new subcommands."""

    def test_show_help_includes_tools_subcommand(self) -> None:
        """show_help should mention the tools subcommand."""
        buf = io.StringIO()
        test_console = Console(file=buf, highlight=False, width=200)
        with patch("niki_agent.ui.console", test_console):
            show_help()
        assert "niki tools" in buf.getvalue()
