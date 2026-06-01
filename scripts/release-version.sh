#!/usr/bin/env bash
# Print the artifact version string for this workflow run.
set -euo pipefail

cargo_version="$(
  awk '
    /^\[package\]/ { in_pkg = 1; next }
    /^\[/ { in_pkg = 0 }
    in_pkg && /^version = / {
      gsub(/"/, "", $3)
      print $3
      exit
    }
  ' Cargo.toml
)"

if [[ -z "$cargo_version" ]]; then
  echo "failed to read enc-sync version from Cargo.toml [package] section" >&2
  exit 1
fi

pr_number() {
  if [[ -n "${GITHUB_EVENT_NUMBER:-}" ]]; then
    printf '%s' "$GITHUB_EVENT_NUMBER"
    return 0
  fi

  if [[ -n "${GITHUB_EVENT_PATH:-}" && -f "$GITHUB_EVENT_PATH" ]]; then
    jq -r '.number // empty' "$GITHUB_EVENT_PATH"
    return 0
  fi

  echo "missing pull request number (set GITHUB_EVENT_NUMBER or GITHUB_EVENT_PATH)" >&2
  exit 1
}

short_sha="${GITHUB_SHA:0:7}"

if [[ "${GITHUB_REF_TYPE:-}" == "tag" ]]; then
  printf '%s\n' "${GITHUB_REF_NAME#v}"
elif [[ "${GITHUB_EVENT_NAME:-}" == "pull_request" ]]; then
  number="$(pr_number)"
  if [[ -z "$number" ]]; then
    echo "missing pull request number in GitHub event payload" >&2
    exit 1
  fi
  printf '%s\n' "${cargo_version}-pr.${number}.${short_sha}"
else
  printf '%s\n' "${cargo_version}-dev.${short_sha}"
fi
