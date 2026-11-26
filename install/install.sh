#!/usr/bin/env bash
# Niki Agent installer for Linux and macOS.
#
# Creates an isolated environment and installs the pinned wheel that ships
# beside this script. Nothing is written outside the chosen prefix, and nothing
# from the user's shell profile is sourced.
#
#   curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
#
# Override the destination with NIKI_PREFIX (default ~/.local/niki).

set -euo pipefail

PREFIX="${NIKI_PREFIX:-$HOME/.local/niki}"
BINDIR="$PREFIX/bin"
WHEEL="${NIKI_WHEEL:-}"

log() { printf '  %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# --- integrity --------------------------------------------------------------
# Every downloaded archive is checked against the release's SHA256SUMS before it
# is unpacked. A missing file, a missing entry, or a mismatch is fatal: an
# unverified archive is never installed.
sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$1" | awk '{print $NF}'
  else
    die "no sha256sum, shasum, or openssl available to verify the download"
  fi
}

verify_sha256() {
  local file="$1" sums="$2" name="$3" expected actual
  expected="$(awk -v n="$name" '$2 == n {print $1}' "$sums" | head -1)"
  [ -n "$expected" ] || die "SHA256SUMS has no entry for $name"
  actual="$(sha256_of "$file")"
  [ "$actual" = "$expected" ] \
    || die "checksum mismatch for $name (expected $expected, got $actual)"
  log "verified $name"
}

# --- locate the wheel -------------------------------------------------------
# Prefer a wheel next to this script (release tarball); otherwise download the
# newest release asset for this platform.
find_wheel() {
  if [ -n "$WHEEL" ]; then
    printf '%s' "$WHEEL"; return 0
  fi
  local dir candidate sums source="${BASH_SOURCE[0]:-}"
  # `curl ... | bash` leaves `BASH_SOURCE[0]` empty or set to the interpreter
  # name, so there is no script directory to look in.
  if [ -n "$source" ] && [ "$source" != "bash" ] && [ "$source" != "main" ]; then
    dir="$(cd "$(dirname "$source")" && pwd)"
    candidate="$(ls -1 "$dir"/niki_agent-*.whl 2>/dev/null | head -1 || true)"
    if [ -n "$candidate" ]; then
      sums="$dir/SHA256SUMS"
      [ -f "$sums" ] && verify_sha256 "$candidate" "$sums" "$(basename "$candidate")"
      printf '%s' "$candidate"; return 0
    fi
  fi

  log "no wheel alongside the installer; downloading the latest release"
  local url platform file_asset sums_asset
  url="https://api.github.com/repos/RavaniRoshan/niki-agent/releases/latest"
  command -v curl >/dev/null 2>&1 || die "curl is required to download a release"

  case "$(uname -s)" in
    Darwin) platform="macos" ;;
    Linux)  platform="linux" ;;
    *) die "unsupported platform: $(uname -s)" ;;
  esac
  local release_json; release_json="$(curl -fsSL "$url")" \
    || die "could not read the release metadata"
  file_asset="$(printf '%s' "$release_json" \
    | grep -o '"browser_download_url": *"[^"]*'"$platform"'[^"]*"' \
    | head -1 | cut -d'"' -f4)" || true
  [ -n "$file_asset" ] || die "could not find a release asset for $platform"
  sums_asset="$(printf '%s' "$release_json" \
    | grep -o '"browser_download_url": *"[^"]*SHA256SUMS"' \
    | head -1 | cut -d'"' -f4)" || true
  [ -n "$sums_asset" ] \
    || die "this release publishes no SHA256SUMS; refusing to install an unverified archive"

  local tmp archive_name; tmp="$(mktemp -d)"
  archive_name="$(basename "$file_asset")"
  curl -fsSL "$sums_asset" -o "$tmp/SHA256SUMS" || die "could not download SHA256SUMS"
  curl -fsSL "$file_asset" -o "$tmp/$archive_name" || die "could not download $archive_name"
  verify_sha256 "$tmp/$archive_name" "$tmp/SHA256SUMS" "$archive_name"

  tar -xzf "$tmp/$archive_name" -C "$tmp" || die "could not unpack $archive_name"
  candidate="$(ls -1 "$tmp"/niki-*/niki_agent-*.whl 2>/dev/null | head -1 || true)"
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