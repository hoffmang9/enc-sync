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

if [[ "${GITHUB_REF_TYPE:-}" == "tag" ]]; then
  printf '%s\n' "${GITHUB_REF_NAME#v}"
elif [[ "${GITHUB_EVENT_NAME:-}" == "pull_request" ]]; then
  printf '%s\n' "${cargo_version}-pr.${GITHUB_EVENT_NUMBER}.${GITHUB_SHA:0:7}"
else
  printf '%s\n' "${cargo_version}-dev.${GITHUB_SHA:0:7}"
fi
