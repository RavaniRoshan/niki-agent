# Releasing Niki Agent

Niki Agent ships as a Python wheel plus a small installer. There is one
distribution (`niki-agent`) and four platform artifacts.

## Version source

The version lives in two places and must agree:

- `libs/code/pyproject.toml` → `[project] version`
- `libs/code/niki_agent/_version.py` → `__version__`

`make check` runs `.github/scripts/checks/check_version_equality.py`, which fails
on drift.

## Cutting a release

1. Bump both version fields in one commit: `release(code): 1.1.0`.
2. Run `make check` in `libs/code`.
3. Merge to `niki-agent`.
4. Tag the merge commit and push the tag:

   ```bash
   git tag v1.1.0
   git push origin v1.1.0
   ```

   Or dispatch **Niki Release** with the version (no leading `v`).

`Niki Release` builds on `ubuntu-latest`, `macos-14`, `macos-13`, and
`windows-latest`. Each job runs `uv build --wheel --sdist` in `libs/code`,
copies the wheel plus `install/install.sh` or `install/install.ps1` and
`NOTICE`/`README.md` into `niki-<version>-<platform>.{tar.gz,zip}`, and uploads
it. The `release` job attaches every artifact to a GitHub Release named
`Niki Agent v<version>`, using `.github/RELEASE_NOTES_niki.md` as the body.

## Installing a release

Linux/macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
```

The installer creates an isolated virtual environment under
`~/.local/niki` (`%LOCALAPPDATA%\niki` on Windows), installs the wheel, and
writes a `niki` launcher. It prints `niki --version` on success.

## PyPI

The distribution name is `niki-agent`. Publishing to PyPI is a separate,
credentialed step and is not part of the tag workflow. Until it is published,
`uv tool install niki-agent` resolves nothing; use the installer above.

## What a release must include

- The wheel and sdist for `niki-agent`.
- `NOTICE` (MIT attribution for the vendored SDK and ACP runtime).
- `README.md` at the archive root.
- The platform installer.

The vendored trees carry their own `PROVENANCE.md`; do not strip them from the
wheel.

## Rolling back

GitHub Releases are immutable enough for this project: delete the release and
tag, then re-tag from the previous commit. There is no fan-out to other
packages, so no version-line coordination is needed.
