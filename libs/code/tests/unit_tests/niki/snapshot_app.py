"""The app instance `pytest-textual-snapshot` imports for visual comparisons.

The plugin takes a module *path* and imports it itself, so the app cannot be
built by a fixture. This module exists to be that path: it builds one
`DeepAgentsApp` with a mock agent and a stubbed post-paint init, which is the
same construction `test_pilot_driver.py` and `test_lint_rules.py` use.

`DEEPAGENTS_HOME` has already been redirected to a temp dir by the suite
conftest before this module is imported, so a snapshot run never reads or
writes the developer's real `~/.deepagents`.
"""

from __future__ import annotations

import os
import random
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock

# Set before the app is imported. `startup_tip.py:117` picks a tip with
# `random.choices`, so without this every launch renders a different splash and
# no snapshot can ever re-verify. Seeding `random` as well costs nothing and
# covers any other incidental sampling.
os.environ["DEEPAGENTS_CODE_HIDE_SPLASH_TIPS"] = "1"
random.seed(0)

from deepagents_code.niki.app import NikiApp  # noqa: E402

# The footer prints the working directory and git branch, which would otherwise
# bind every snapshot to the machine and checkout that generated it. Pin both
# so the visual baseline describes the layout, not the developer's environment.
_SNAPSHOT_CWD = Path.home() / "projects" / "nik-agent" / "libs" / "code"
_SNAPSHOT_BRANCH = "niki-agent"


class _SnapshotApp(NikiApp):
    """Niki app with an environment-independent status bar."""

    def _schedule_git_branch_refresh(self) -> None:
        if self._status_bar:
            self._status_bar.branch = _SNAPSHOT_BRANCH

    async def _refresh_git_branch(self) -> None:
        if self._status_bar:
            self._status_bar.branch = _SNAPSHOT_BRANCH


_niki_snapshot_app = _SnapshotApp(
    agent=MagicMock(), cwd=_SNAPSHOT_CWD, thread_id="niki-snapshot"
)
_niki_snapshot_app._post_paint_init = AsyncMock()  # type: ignore[method-assign]

app = _niki_snapshot_app
"""The instance the snapshot plugin mounts."""
