#!/usr/bin/env bash
# Merge per-arch macOS dist archives into one universal tarball (lipo).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=release-targets.sh
source "$ROOT/scripts/release-targets.sh"

DISTRIB="${DISTRIB_DIR:-target/distrib}"
UNIX_ARCHIVE="$(unix_archive_suffix)"

mapfile -t mac_triples < <(mac_per_arch_triples) || {
  echo "dist.toml must list exactly two *-apple-darwin targets for universal macOS builds" >&2
  exit 1
}

ARM_TRIPLE=""
X64_TRIPLE=""
for triple in "${mac_triples[@]}"; do
  case "$triple" in
    aarch64-apple-darwin) ARM_TRIPLE=$triple ;;
    x86_64-apple-darwin) X64_TRIPLE=$triple ;;
    *)
      echo "unsupported macOS target for lipo: ${triple}" >&2
      exit 1
      ;;
  esac
done

find_archive() {
  local triple=$1
  find "$DISTRIB" -maxdepth 1 -type f -name "enc-sync-${triple}.tar.*" ! -name '*.sha256' | head -1
}

ARM_ARCHIVE="$(find_archive "$ARM_TRIPLE")"
X64_ARCHIVE="$(find_archive "$X64_TRIPLE")"

if [[ -z "$ARM_ARCHIVE" || -z "$X64_ARCHIVE" ]]; then
  echo "macOS per-arch archives not found; expected ${ARM_TRIPLE} and ${X64_TRIPLE}" >&2
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

STAGING="$WORK/enc-sync-${UNIVERSAL_MAC_TRIPLE}"
mkdir -p "$STAGING"
cp -a "$WORK/arm/enc-sync-${ARM_TRIPLE}/." "$STAGING/"
lipo -create -output "$STAGING/enc-sync" "$X64_BIN" "$ARM_BIN"
chmod +x "$STAGING/enc-sync"

UNIVERSAL_ARCHIVE="$DISTRIB/enc-sync-${UNIVERSAL_MAC_TRIPLE}${UNIX_ARCHIVE}"
case "$UNIX_ARCHIVE" in
  .tar.gz) tar czf "$UNIVERSAL_ARCHIVE" -C "$WORK" "enc-sync-${UNIVERSAL_MAC_TRIPLE}" ;;
  *)
    echo "unsupported unix-archive suffix for universal bundle: ${UNIX_ARCHIVE}" >&2
    exit 1
    ;;
esac

rm -f "$ARM_ARCHIVE" "${ARM_ARCHIVE}.sha256" "$X64_ARCHIVE" "${X64_ARCHIVE}.sha256"

echo "Created universal macOS archive: $UNIVERSAL_ARCHIVE"
lipo -info "$STAGING/enc-sync"

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  echo "universal_archive=${UNIVERSAL_ARCHIVE}" >>"$GITHUB_OUTPUT"
fi
