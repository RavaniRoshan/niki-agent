#!/usr/bin/env bash
# Niki Agent installer for Linux and macOS.
#
# Creates an isolated environment and installs the pinned wheel that ships
# beside this script. Nothing is written outside the chosen prefix, and nothing
# from the user's shell profile is sourced.
#
#   curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/main/install/install.sh | bash
#
# Override the destination with NIKI_PREFIX (default ~/.local/niki).

set -euo pipefail

PREFIX="${NIKI_PREFIX:-$HOME/.local/niki}"
BINDIR="$PREFIX/bin"
WHEEL="${NIKI_WHEEL:-}"

log() { printf '  %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# --- locate the wheel -------------------------------------------------------
# Prefer a wheel next to this script (release tarball); otherwise download the
# newest release asset for this platform.
find_wheel() {
  if [ -n "$WHEEL" ]; then
    printf '%s' "$WHEEL"; return 0
  fi
  local dir candidate
  dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  candidate="$(ls -1 "$dir"/deepagents_code-*.whl 2>/dev/null | head -1 || true)"
  if [ -n "$candidate" ]; then printf '%s' "$candidate"; return 0; fi

  log "no wheel alongside the installer; downloading the latest release"
  local url asset
  url="https://api.github.com/repos/RavaniRoshan/niki-agent/releases/latest"
  command -v curl >/dev/null 2>&1 || die "curl is required to download a release"

  case "$(uname -s)" in
    Darwin) asset="macos" ;;
    Linux)  asset="linux" ;;
    *) die "unsupported platform: $(uname -s)" ;;
  esac
  asset="$(curl -fsSL "$url" \
    | grep -o '"browser_download_url": *"[^"]*'"$asset"'[^"]*"' \
    | head -1 | cut -d'"' -f4)" || true
  [ -n "$asset" ] || die "could not find a release asset for $asset"

  local tmp; tmp="$(mktemp -d)"
  curl -fsSL "$asset" -o "$tmp/pkg.tar.gz"
  tar -xzf "$tmp/pkg.tar.gz" -C "$tmp"
  candidate="$(ls -1 "$tmp"/niki-*/deepagents_code-*.whl 2>/dev/null | head -1 || true)"
  [ -n "$candidate" ] || die "release asset contained no wheel"
  printf '%s' "$candidate"
}

main() {
  command -v python3 >/dev/null 2>&1 || die "python3 is required"
  local pyver; pyver="$(python3 -c 'import sys;print("%d.%d"%sys.version_info[:2])')"
  case "$pyver" in
    3.1[0-1]|2.*) die "Python 3.12+ required, found $pyver" ;;
  esac

  local wheel; wheel="$(find_wheel)"
  log "installing $(basename "$wheel")"

  mkdir -p "$PREFIX"
  python3 -m venv "$PREFIX/venv"
  "$PREFIX/venv/bin/python" -m pip install --quiet --upgrade pip
  "$PREFIX/venv/bin/python" -m pip install --quiet "$wheel"

  mkdir -p "$BINDIR"
  cat > "$BINDIR/niki" <<EOF
#!/usr/bin/env bash
exec "$PREFIX/venv/bin/niki" "\$@"
EOF
  chmod +x "$BINDIR/niki"

  # A shell-mode flag would be overwritten on every install; only offer it when
  # the user asked for it.
  if [ "${NIKI_SHELL:-0}" = "1" ]; then
    line="export PATH=\"$BINDIR:\$PATH\""
    for rc in "$HOME/.bashrc" "$HOME/.zshrc"; do
      [ -f "$rc" ] || continue
      grep -qF "$line" "$rc" || printf '\n# Niki Agent\n%s\n' "$line" >> "$rc"
    done
  fi

  "$BINDIR/niki" --version
  log ""
  log "installed to $PREFIX"
  log "add to PATH:  export PATH=\"$BINDIR:\$PATH\""
  log "re-run with NIKI_SHELL=1 to append that to your shell profile"
}

main "$@"