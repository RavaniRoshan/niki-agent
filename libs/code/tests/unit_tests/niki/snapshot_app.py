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
from unittest.mock import AsyncMock, MagicMock

# Set before the app is imported. `startup_tip.py:117` picks a tip with
# `random.choices`, so without this every launch renders a different splash and
# no snapshot can ever re-verify. Seeding `random` as well costs nothing and
# covers any other incidental sampling.
os.environ["DEEPAGENTS_CODE_HIDE_SPLASH_TIPS"] = "1"
random.seed(0)

from deepagents_code.niki.app import NikiApp  # noqa: E402

_niki_snapshot_app = NikiApp(agent=MagicMock(), thread_id="niki-snapshot")
_niki_snapshot_app._post_paint_init = AsyncMock()  # type: ignore[method-assign]

app = _niki_snapshot_app
"""The instance the snapshot plugin mounts."""
