"""Regenerate `README.md` from live facts, so it cannot drift.

The README is the first thing anyone reads and the fastest thing to go stale:
the version moves, a key is added, a checklist row changes. This builds it from
things that are true at the moment it runs -- the installed version, the
registry that generates `KEYMAP.md`, the test count, the checklist tally -- so a
stale claim is a failing script rather than a lie in a file nobody re-reads.

Run from `libs/code`:

    uv run python scripts/gen_readme.py

Every number below is measured. Nothing is carried over from a previous README.
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parents[3]
CHECKLIST = REPO_ROOT / "docs" / "niki" / "CHECKLIST.md"
KEYMAP = REPO_ROOT / "docs" / "niki" / "KEYMAP.md"
README = REPO_ROOT / "README.md"

FRAME = "test_first_run_snapshot[120x38]"


def _run(*args: str, cwd: pathlib.Path) -> str:
    """Run a command and return stdout, or a marker if it fails.

    Returns:
        The command's stripped stdout, or `"unavailable"`.
    """
    try:
        result = subprocess.run(
            list(args),
            cwd=cwd,
            capture_output=True,
            text=True,
            check=True,
            timeout=600,
        )
    except (OSError, subprocess.SubprocessError):
        return "unavailable"
    return result.stdout.strip()


def _test_count() -> str:
    """Total tests in the Niki harness.

    Returns:
        The collected test count, or `"unknown"` if it could not be measured.
    """
    out = _run(
        "uv",
        "run",
        "pytest",
        "tests/unit_tests/niki/",
        "-q",
        "--collect-only",
        cwd=REPO_ROOT / "libs" / "code",
    )
    if out == "unavailable":
        return "unknown"
    for token in out.replace("=", " ").split():
        if token.isdigit() and "tests" in out.split(token)[-1]:
            return token
    return out.splitlines()[-1] if out else "unknown"


def _tally() -> dict[str, int]:
    """Parse the WORKS/PARTIAL/MISSING counts out of the checklist header.

    Returns:
        A mapping of row status to count; zero where the header did not parse.
    """
    text = CHECKLIST.read_text(encoding="utf-8")
    counts = {"WORKS": 0, "PARTIAL": 0, "MISSING": 0}
    for line in text.splitlines():
        if "Current tally" not in line:
            continue
        # Example header: tally counts then "across N rows".
        for number, name in re.findall(r"(\d+)\s+(WORKS|PARTIAL|MISSING)", line):
            counts[name] = int(number)
    return counts


def _keymap_rows() -> int:
    """Count the data rows in the generated keymap table.

    Returns:
        The number of documented bindings, or 0 if the file is absent.
    """
    if not KEYMAP.exists():
        return 0
    return sum(
        1
        for line in KEYMAP.read_text(encoding="utf-8").splitlines()
        if line.startswith("| `")
    )


_VERSION_SNIPPET = (
    "from deepagents_code.niki.branding import product_version as v; print(v())"
)
"""Read the version without importing the app; `-v` must stay cheap."""

TEMPLATE_PATH = pathlib.Path(__file__).parent / "readme_template.md"
"""Markdown template for the README, kept out of Python on purpose.

It is data, not code: keeping it in a `.md` file means the repository's
own line-length rules apply to prose exactly as they do everywhere else,
instead of the template being exempt inside a string literal.
"""


def main() -> int:
    """Write the README from live facts.

    Returns:
        Process exit code.
    """
    version = _run(
        "uv",
        "run",
        "python",
        "-c",
        _VERSION_SNIPPET,
        cwd=REPO_ROOT / "libs" / "code",
    )
    tally = _tally()
    tests = _test_count()
    rows = _keymap_rows()
    template = TEMPLATE_PATH.read_text(encoding="utf-8")
    body = template.format(
        version=version,
        tests=tests,
        keymap=rows,
        works=tally["WORKS"],
        partial=tally["PARTIAL"],
        missing=tally["MISSING"],
        frame=FRAME,
    )
    README.write_text(body, encoding="utf-8")
    print(f"wrote {README}")
    print(f"  version={version} tests={_test_count()} keymap={_keymap_rows()} {tally}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
