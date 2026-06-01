#!/usr/bin/env bash
# Post a commit status for PR release builds (workflow_dispatch only).
#
# Context: release/enc-sync — add as a required check on PRs to block merge when
# Release fails while CI is green.
set -euo pipefail

if [[ "${GITHUB_EVENT_NAME:-}" != "workflow_dispatch" ]]; then
  exit 0
fi

post_status() {
  local state=$1
  local description=$2
  local context="${RELEASE_STATUS_CONTEXT:-release/enc-sync}"
  local sha="${RELEASE_STATUS_SHA:-${GITHUB_SHA:?}}"
  local target_url="${RELEASE_STATUS_TARGET_URL:-${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}}"

  gh api \
    -X POST \
    "repos/${GITHUB_REPOSITORY}/statuses/${sha}" \
    -f "state=${state}" \
    -f "context=${context}" \
    -f "description=${description}" \
    -f "target_url=${target_url}"
}

report_from_job_results() {
  local version="${RELEASE_VERSION:-unknown}"

  if [[ "${PLAN_RESULT:-}" == "success" && "${BUILD_LOCAL_RESULT:-}" == "success" && "${POSTBUILD_RESULT:-}" == "success" ]]; then
    post_status success "Release artifacts enc-sync-${version}"
  else
    post_status failure "Release build failed"
  fi
}

case "${1:-}" in
  pending | success | failure)
    post_status "$1" "${2:-Release build}"
    ;;
  report)
    report_from_job_results
    ;;
  *)
    echo "usage: $0 {pending|success|failure|report} [description]" >&2
    exit 1
    ;;
esac
