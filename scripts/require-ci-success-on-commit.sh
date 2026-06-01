#!/usr/bin/env bash
# Fail unless the CI workflow completed successfully on the given commit.
set -euo pipefail

commit="${1:-${GITHUB_SHA:?GITHUB_SHA or commit argument required}}"
workflow="${CI_WORKFLOW_NAME:-CI}"

conclusion="$(
  gh run list \
    --repo "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY required}" \
    --commit "$commit" \
    --workflow "$workflow" \
    --limit 20 \
    --json conclusion,status \
    --jq '[.[] | select(.status == "completed")][0].conclusion // empty'
)"

if [[ -z "$conclusion" ]]; then
  echo "no completed ${workflow} run found for commit ${commit}" >&2
  echo "merge to main and wait for CI before tagging this commit" >&2
  exit 1
fi

if [[ "$conclusion" != "success" ]]; then
  echo "${workflow} on commit ${commit} concluded: ${conclusion}" >&2
  exit 1
fi

echo "${workflow} succeeded on commit ${commit}"
