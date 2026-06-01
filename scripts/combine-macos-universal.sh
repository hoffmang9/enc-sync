#!/usr/bin/env bash
# Build a universal macOS tarball from per-arch dist archives. File operations only.
#
# Contract: dist.toml lists per-arch *-apple-darwin targets; prepare-release-bundle.sh
# expects enc-sync-universal-apple-darwin.tar.gz from this script (see dist-workspace.toml).
set -euo pipefail

DISTRIB="${DISTRIB_DIR:-target/distrib}"
ARM_TRIPLE="aarch64-apple-darwin"
X64_TRIPLE="x86_64-apple-darwin"
UNIVERSAL_TRIPLE="universal-apple-darwin"

find_archive() {
  local triple=$1
  find "$DISTRIB" -maxdepth 1 -type f -name "enc-sync-${triple}.tar.*" ! -name '*.sha256' | head -1
}

ARM_ARCHIVE="$(find_archive "$ARM_TRIPLE")"
X64_ARCHIVE="$(find_archive "$X64_TRIPLE")"

if [[ -z "$ARM_ARCHIVE" && -z "$X64_ARCHIVE" ]]; then
  echo "macOS per-arch archives not found; expected ${ARM_TRIPLE} and ${X64_TRIPLE}" >&2
  exit 1
fi

if [[ -z "$ARM_ARCHIVE" || -z "$X64_ARCHIVE" ]]; then
  echo "expected both ${ARM_TRIPLE} and ${X64_TRIPLE} archives" >&2
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

extract_archive() {
  local archive=$1 dest=$2
  mkdir -p "$dest"
  case "$archive" in
    *.tar.gz) tar xzf "$archive" -C "$dest" ;;
    *.tar.xz) tar xJf "$archive" -C "$dest" ;;
    *.tar.zst) tar --zstd -xf "$archive" -C "$dest" ;;
    *) echo "unsupported archive format: $archive" >&2; exit 1 ;;
  esac
}

extract_archive "$ARM_ARCHIVE" "$WORK/arm"
extract_archive "$X64_ARCHIVE" "$WORK/x64"

ARM_BIN="$WORK/arm/enc-sync-${ARM_TRIPLE}/enc-sync"
X64_BIN="$WORK/x64/enc-sync-${X64_TRIPLE}/enc-sync"

if [[ ! -f "$ARM_BIN" || ! -f "$X64_BIN" ]]; then
  echo "failed to locate enc-sync binaries at expected dist paths" >&2
  exit 1
fi

STAGING="$WORK/enc-sync-${UNIVERSAL_TRIPLE}"
mkdir -p "$STAGING"
cp -a "$WORK/arm/enc-sync-${ARM_TRIPLE}/." "$STAGING/"
lipo -create -output "$STAGING/enc-sync" "$X64_BIN" "$ARM_BIN"
chmod +x "$STAGING/enc-sync"

UNIVERSAL_ARCHIVE="$DISTRIB/enc-sync-${UNIVERSAL_TRIPLE}.tar.gz"
tar czf "$UNIVERSAL_ARCHIVE" -C "$WORK" "enc-sync-${UNIVERSAL_TRIPLE}"

rm -f "$ARM_ARCHIVE" "${ARM_ARCHIVE}.sha256" "$X64_ARCHIVE" "${X64_ARCHIVE}.sha256"

echo "Created universal macOS archive: $UNIVERSAL_ARCHIVE"
lipo -info "$STAGING/enc-sync"
