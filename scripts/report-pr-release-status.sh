#!/usr/bin/env bash
# Post a commit status for PR release builds (workflow_dispatch only).
#
# Context: release/enc-sync — add as a required check on PRs to block merge when
# Release fails while CI is green.
set -euo pipefail

if [[ "${GITHUB_EVENT_NAME:-}" != "workflow_dispatch" ]]; then
  exit 0
fi

state="${1:?state required: pending, success, or failure}"
description="${2:-Release build}"
context="${RELEASE_STATUS_CONTEXT:-release/enc-sync}"
sha="${RELEASE_STATUS_SHA:-${GITHUB_SHA:?}}"
target_url="${RELEASE_STATUS_TARGET_URL:-${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}}"

gh api \
  -X POST \
  "repos/${GITHUB_REPOSITORY}/statuses/${sha}" \
  -f "state=${state}" \
  -f "context=${context}" \
  -f "description=${description}" \
  -f "target_url=${target_url}"
