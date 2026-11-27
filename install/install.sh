#!/usr/bin/env bash
# Niki Agent installer for Linux and macOS.
#
# Installs a prebuilt, self-contained `niki` binary. There is no Python
# runtime, no virtual environment, and no package resolution at install time:
# the release archive holds one executable, and the release's SHA256SUMS proves
# it before anything is written to the prefix.
#
# The release publishes one archive per platform-architecture pair; the matching
# one is chosen from `uname -s` and `uname -m`:
#   linux-x86_64, macos-aarch64 (Apple silicon), macos-x86_64 (Intel).
#
#   curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
#
# Environment:
#   NIKI_PREFIX   install destination (default ~/.local/niki)
#   NIKI_VERSION  release to install, without the leading v (default: latest)
#   NIKI_ARCHIVE  install from this local archive instead of downloading
#   NIKI_SHELL=1  append the bin directory to ~/.bashrc and ~/.zshrc

set -euo pipefail

PREFIX="${NIKI_PREFIX:-$HOME/.local/niki}"
BINDIR="$PREFIX/bin"
ARCHIVE="${NIKI_ARCHIVE:-}"
VERSION="${NIKI_VERSION:-}"
REPO="RavaniRoshan/niki-agent"

log() { printf '  %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# Extracted archives are removed on every exit path, including `die`.
STAGE_DIR=""
cleanup() {
  if [ -n "${STAGE_DIR:-}" ]; then rm -rf "$STAGE_DIR"; fi
  return 0
}
trap cleanup EXIT

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

# --- platform ---------------------------------------------------------------
detect_platform() {
  case "$(uname -s)" in
    Linux)  printf 'linux' ;;
    Darwin) printf 'macos' ;;
    *) die "unsupported platform: $(uname -s)" ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64 | amd64) printf 'x86_64' ;;
    aarch64 | arm64) printf 'aarch64' ;;
    *) die "unsupported architecture: $(uname -m)" ;;
  esac
}

# --- locate the archive -----------------------------------------------------
# Prefer an archive next to this script (release tarball or a hand-downloaded
# release); otherwise download the matching asset from the named release.
find_archive() {
  local platform arch
  platform="$(detect_platform)"
  arch="$(detect_arch)"

  if [ -n "$ARCHIVE" ]; then
    [ -f "$ARCHIVE" ] || die "NIKI_ARCHIVE does not exist: $ARCHIVE"
    printf '%s' "$ARCHIVE"
    return 0
  fi

  local dir candidate source="${BASH_SOURCE[0]:-}"
  # `curl ... | bash` leaves `BASH_SOURCE[0]` empty or set to the interpreter
  # name, so there is no script directory to look in.
  if [ -n "$source" ] && [ "$source" != "bash" ] && [ "$source" != "main" ]; then
    dir="$(cd "$(dirname "$source")" && pwd)"
    candidate="$(ls -1 "$dir"/niki-*-"$platform"-"$arch".tar.gz 2>/dev/null | head -1 || true)"
    if [ -n "$candidate" ]; then
      local sums="$dir/SHA256SUMS"
      [ -f "$sums" ] && verify_sha256 "$candidate" "$sums" "$(basename "$candidate")"
      printf '%s' "$candidate"
      return 0
    fi
  fi

  command -v curl >/dev/null 2>&1 || die "curl is required to download a release"
  log "downloading the $platform-$arch release"
  local url json asset sums_asset tmp name
  if [ -n "$VERSION" ]; then
    url="https://api.github.com/repos/$REPO/releases/tags/v${VERSION#v}"
  else
    url="https://api.github.com/repos/$REPO/releases/latest"
  fi
  json="$(curl -fsSL "$url")" || die "could not read release metadata from $url"

  asset="$(printf '%s' "$json" \
    | grep -o '"browser_download_url": *"[^"]*/niki-[^"/]*-'"$platform"'-'"$arch"'\.tar\.gz"' \
    | head -1 | cut -d'"' -f4)" || true

  if [ -z "$asset" ]; then
    if [ "$platform" = "macos" ]; then
      cat >&2 <<EOF
error: this release publishes no niki-$platform-$arch archive.
macOS archives are named macos-aarch64 (Apple silicon) and macos-x86_64
(Intel). Releases published before the macOS archives existed carry neither;
otherwise this host's archive was not built for this release.
Build it from source instead:
  git clone https://github.com/$REPO
  cd niki-agent/rust
  cargo build --release -p niki --bin niki
  install -m 0755 target/release/niki "\$HOME/.local/niki/bin/niki"
EOF
      exit 1
    fi
    die "this release publishes no $platform-$arch artifact"
  fi

  sums_asset="$(printf '%s' "$json" \
    | grep -o '"browser_download_url": *"[^"]*SHA256SUMS"' \
    | head -1 | cut -d'"' -f4)" || true
  [ -n "$sums_asset" ] \
    || die "this release publishes no SHA256SUMS; refusing to install an unverified archive"

  tmp="$(mktemp -d)"
  name="$(basename "$asset")"
  curl -fsSL "$sums_asset" -o "$tmp/SHA256SUMS" || die "could not download SHA256SUMS"
  curl -fsSL "$asset" -o "$tmp/$name" || die "could not download $name"
  verify_sha256 "$tmp/$name" "$tmp/SHA256SUMS" "$name"
  printf '%s' "$tmp/$name"
}

install_binary() {
  local binary="$1"
  mkdir -p "$BINDIR"
  install -m 0755 "$binary" "$BINDIR/niki"

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

main() {
  local archive binary source="${BASH_SOURCE[0]:-}" sibling=""

  # An extracted release archive holds the installer beside the binary; that
  # binary is already the one the archive shipped, so install it directly
  # instead of downloading the same release again.
  if [ -n "$source" ] && [ "$source" != "bash" ] && [ "$source" != "main" ]; then
    local dir; dir="$(cd "$(dirname "$source")" && pwd)"
    [ -x "$dir/niki" ] && sibling="$dir/niki"
  fi

  if [ -n "$sibling" ] && [ -z "$ARCHIVE" ]; then
    log "installing $(basename "$(dirname "$sibling")")/niki"
    install_binary "$sibling"
    return 0
  fi

  archive="$(find_archive)"
  log "installing $(basename "$archive")"

  STAGE_DIR="$(mktemp -d)"
  tar -xzf "$archive" -C "$STAGE_DIR" || die "could not unpack $(basename "$archive")"
  binary="$(find "$STAGE_DIR" -type f -name niki | head -1)"
  [ -n "$binary" ] || die "archive contained no niki binary"

  install_binary "$binary"
}

main "$@"
