#!/usr/bin/env bash
# Fail unless the CI workflow completed successfully on the given commit.
set -euo pipefail

commit="${1:-${GITHUB_SHA:?GITHUB_SHA or commit argument required}}"
workflow="${CI_WORKFLOW_NAME:-CI}"
wait_seconds="${WAIT_FOR_CI_SECONDS:-0}"
poll_seconds="${WAIT_FOR_CI_POLL_SECONDS:-15}"

ci_state() {
  gh run list \
    --repo "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY required}" \
    --commit "$commit" \
    --workflow "$workflow" \
    --limit 20 \
    --json conclusion,status \
    --jq '.[0] | [.status // "", .conclusion // ""] | @tsv'
}

IFS=$'\t' read -r status conclusion < <(ci_state)

if [[ "$status" != "completed" && "$wait_seconds" != "0" ]]; then
  deadline=$((SECONDS + wait_seconds))
  echo "waiting up to ${wait_seconds}s for ${workflow} on commit ${commit}"
  while [[ "$status" != "completed" && $SECONDS -lt $deadline ]]; do
    sleep "$poll_seconds"
    IFS=$'\t' read -r status conclusion < <(ci_state)
  done
fi

if [[ "$status" != "completed" ]]; then
  echo "no completed ${workflow} run found for commit ${commit}" >&2
  echo "merge to main and wait for CI before tagging this commit" >&2
  exit 1
fi

if [[ "$conclusion" != "success" ]]; then
  echo "${workflow} on commit ${commit} concluded: ${conclusion}" >&2
  exit 1
fi

echo "${workflow} succeeded on commit ${commit}"
