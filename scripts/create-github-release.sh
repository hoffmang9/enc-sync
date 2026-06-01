#!/usr/bin/env bash
# Generate dist release notes and create the GitHub release from staged artifacts.
#
# Requires TAG_FLAG in the environment (e.g. --tag=v0.1.0), set by the host job from
# needs.plan.outputs.tag-flag before invoking this script.
#
# artifacts/ is populated by prepare-release-bundle.sh with platform archives only.
set -euo pipefail

TAG="${1:?"release tag required"}"
ARTIFACTS_DIR="${2:-artifacts}"
MANIFEST="${3:-dist-manifest.json}"
RELEASE_COMMIT="${RELEASE_COMMIT:-${GITHUB_SHA:?}}"

dist host "${TAG_FLAG:?TAG_FLAG is required}" --steps=release --output-format=json >"$MANIFEST"
echo "release notes generated successfully"
cat "$MANIFEST"

shopt -s nullglob
files=( "$ARTIFACTS_DIR"/* )
if (( ${#files[@]} == 0 )); then
  echo "no release artifacts in ${ARTIFACTS_DIR}" >&2
  exit 1
fi

prerelease_flag=()
if [[ "$(jq -r '.announcement_is_prerelease // false' "$MANIFEST")" == "true" ]]; then
  prerelease_flag=(--prerelease)
fi

title="$(jq -r '.announcement_title' "$MANIFEST")"
body="$(jq -r '.announcement_github_body' "$MANIFEST")"

notes="$(mktemp)"
trap 'rm -f "$notes"' EXIT
printf '%s\n' "$body" >"$notes"

gh release create "$TAG" \
  --target "$RELEASE_COMMIT" \
  "${prerelease_flag[@]}" \
  --title "$title" \
  --notes-file "$notes" \
  "${files[@]}"
