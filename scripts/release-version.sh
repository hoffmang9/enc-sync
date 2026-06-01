#!/usr/bin/env bash
# Print the artifact version string for this workflow run.
#
# Modes (set by the Release plan job):
#   tag — GITHUB_REF_TYPE=tag
#   pr  — RELEASE_PR_NUMBER set (workflow_dispatch from CI)
#   dev — fallback for manual runs
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

short_sha="${GITHUB_SHA:0:7}"

if [[ "${GITHUB_REF_TYPE:-}" == "tag" ]]; then
  # Git tags must match Cargo.toml semver exactly (e.g. 1.0.0-rc1, not v1.0.0-rc1).
  printf '%s\n' "${GITHUB_REF_NAME}"
elif [[ -n "${RELEASE_PR_NUMBER:-}" ]]; then
  printf '%s\n' "${cargo_version}-pr.${RELEASE_PR_NUMBER}.${short_sha}"
else
  printf '%s\n' "${cargo_version}-dev.${short_sha}"
fi
