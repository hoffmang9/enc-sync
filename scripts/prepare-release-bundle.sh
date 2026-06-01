#!/usr/bin/env bash
# Rename dist artifacts and stage platform archives for download.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
# shellcheck source=release-targets.sh
source "$ROOT/scripts/release-targets.sh"

VERSION="${RELEASE_VERSION:?"RELEASE_VERSION is required"}"
DISTRIB="${DISTRIB_DIR:-target/distrib}"
STAGING="${STAGING_DIR:-release-staging}"

release_suffixes=()
while IFS= read -r suffix; do
  [[ -n "$suffix" ]] && release_suffixes+=( "$suffix" )
done < <(list_release_suffixes)
if (( ${#release_suffixes[@]} == 0 )); then
  echo "no release targets found in ${RELEASE_TARGETS_DIST_TOML}" >&2
  exit 1
fi

mkdir -p "$DISTRIB"

rename_dist_artifact() {
  local path=$1 base dest
  base="$(basename "$path")"

  [[ "$base" == enc-sync-${VERSION}-* ]] && return 0

  case "$base" in
    enc-sync-*.tar.* | enc-sync-*.zip)
      dest="$DISTRIB/enc-sync-${VERSION}-${base#enc-sync-}"
      mv "$path" "$dest"
      rm -f "${path}.sha256"
      ;;
  esac
}

shopt -s nullglob
for artifact in "$DISTRIB"/enc-sync-*; do
  [[ -f "$artifact" ]] || continue
  rename_dist_artifact "$artifact"
done

rm -rf "$STAGING"
mkdir -p "$STAGING"

for suffix in "${release_suffixes[@]}"; do
  shopt -s nullglob
  matches=( "$DISTRIB"/enc-sync-"${VERSION}-${suffix}" )
  if (( ${#matches[@]} != 1 )); then
    echo "expected exactly one enc-sync-${VERSION}-${suffix}, found ${#matches[@]}" >&2
    exit 1
  fi
  cp "${matches[0]}" "$STAGING/"
done

echo "Release bundle ready in ${STAGING} (${#release_suffixes[@]} archives):"
ls -1 "$STAGING"
