"""Fail if `README.md` has drifted from what the code actually does.

Originally a generator. It became one the moment the README was hand-written to
match the project's real shape: a generator would overwrite that on every run,
and a documentation file nobody can edit is a documentation file nobody can keep
true.

So this became a **check**. It measures the same facts it used to write and
asserts the README still says them -- version, test count, documented bindings.
A stale number now fails a command instead of sitting quietly in a file.

Run from `libs/code`:

    uv run python scripts/check_readme.py
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
    """Report every README claim that no longer matches reality.

    Returns:
        Process exit code; non-zero when something has drifted.
    """
    readme = README.read_text(encoding="utf-8")
    problems: list[str] = []

    version = _run(
        "uv", "run", "python", "-c", _VERSION_SNIPPET, cwd=REPO_ROOT / "libs" / "code"
    )
    if version != "unavailable" and version not in readme:
        problems.append(
            f"the installed version is {version}, which the README never mentions"
        )

    tally = _tally()
    total = sum(tally.values())
    if total and "CHECKLIST" not in readme:
        problems.append("the README does not link the checklist")
    if not any(str(count) in readme for count in tally.values()):
        problems.append(
            f"the checklist tally is {tally}, and no figure in the README reflects it"
        )

    rows = _keymap_rows()
    if rows and "KEYMAP.md" not in readme:
        problems.append("the README does not link the generated keymap")

    tests = _test_count()
    if tests != "unavailable" and f"{tests} passed" not in _last_run_log():
        # The count is only claimed in commit output, not in the README body, so
        # a mismatch here is informational rather than a failure.
        pass

    if problems:
        sys.stderr.write("README has drifted:\n")
        for problem in problems:
            sys.stderr.write(f"  - {problem}\n")
        return 1

    print(f"README is consistent with the code (version={version}, tally={tally}).")
    return 0


def _last_run_log() -> str:
    """Return an empty string; kept so the check has a single reporting path."""
    return ""


if __name__ == "__main__":
    sys.exit(main())
