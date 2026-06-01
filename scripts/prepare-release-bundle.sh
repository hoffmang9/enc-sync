#!/usr/bin/env bash
# Rename dist artifacts, checksum, stage, and verify the release bundle.
#
# Contract: *-apple-darwin targets in dist.toml are omitted here; universal-apple-darwin
# must already exist from combine-macos-universal.sh (see dist-workspace.toml).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="${RELEASE_VERSION:?"RELEASE_VERSION is required"}"
DISTRIB="${DISTRIB_DIR:-target/distrib}"
STAGING="${STAGING_DIR:-release-staging}"
DIST_TOML="$ROOT/dist.toml"

read_dist_targets() {
  awk '
    BEGIN { in_targets = 0 }
    /^targets = \[/ { in_targets = 1; next }
    in_targets && /^\]/ { exit }
    in_targets {
      n = split($0, parts, "\"")
      for (i = 2; i <= n; i += 2) {
        if (parts[i] != "") {
          print parts[i]
        }
      }
    }
  ' "$DIST_TOML"
}

unix_archive="$(awk -F'"' '/^unix-archive/ { print $2; exit }' "$DIST_TOML")"
unix_archive="${unix_archive:-.tar.gz}"

release_suffixes=()
mac_targets=0
while IFS= read -r triple; do
  [[ -n "$triple" ]] || continue
  case "$triple" in
    *-apple-darwin)
      mac_targets=1
      ;;
    *-pc-windows-*)
      release_suffixes+=( "${triple}.zip" )
      ;;
    *)
      release_suffixes+=( "${triple}${unix_archive}" )
      ;;
  esac
done < <(read_dist_targets)

if (( mac_targets )); then
  release_suffixes+=( "universal-apple-darwin${unix_archive}" )
fi

if (( ${#release_suffixes[@]} == 0 )); then
  echo "no release targets found in ${DIST_TOML}" >&2
  exit 1
fi

mkdir -p "$DISTRIB"

checksum() {
  local file=$1
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" >"${file}.sha256"
  else
    shasum -a 256 "$file" >"${file}.sha256"
  fi
}

rename_dist_artifact() {
  local path=$1 base dest
  base="$(basename "$path")"

  [[ "$base" == enc-sync-${VERSION}-* ]] && return 0

  case "$base" in
    enc-sync-*.tar.* | enc-sync-*.zip)
      dest="$DISTRIB/enc-sync-${VERSION}-${base#enc-sync-}"
      mv "$path" "$dest"
      rm -f "${path}.sha256"
      checksum "$dest"
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
  [[ -f "${matches[0]}.sha256" ]] && cp "${matches[0]}.sha256" "$STAGING/"
done

echo "Release bundle ready in ${STAGING} (${#release_suffixes[@]} archives from dist.toml):"
ls -1 "$STAGING"
