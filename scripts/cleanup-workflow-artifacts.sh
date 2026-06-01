#!/usr/bin/env bash
# Delete workflow artifacts for the current run.
#
# Modes:
#   DELETE_ALL=true     - remove every artifact (tag releases after host publishes)
#   KEEP_ARTIFACT_NAME  - remove all except one named artifact (PR preview bundle)
set -euo pipefail

RUN_ID="${GITHUB_RUN_ID:?GITHUB_RUN_ID is required}"
REPO="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
DELETE_ALL="${DELETE_ALL:-false}"
KEEP_NAME="${KEEP_ARTIFACT_NAME:-}"

artifact_json() {
  if [[ -n "${ARTIFACTS_JSON:-}" ]]; then
    cat "$ARTIFACTS_JSON"
  else
    gh api "repos/${REPO}/actions/runs/${RUN_ID}/artifacts" --paginate
  fi
}

artifact_count() {
  artifact_json | jq '.artifacts | length'
}

artifact_names() {
  artifact_json | jq -r '.artifacts[].name' | sort
}

artifact_ids_to_delete() {
  if [[ "$DELETE_ALL" == "true" ]]; then
    artifact_json | jq -r '.artifacts[].id'
    return 0
  fi

  if [[ -z "$KEEP_NAME" ]]; then
    echo "KEEP_ARTIFACT_NAME is required when DELETE_ALL is not true" >&2
    return 1
  fi

  artifact_json | jq -r --arg keep "$KEEP_NAME" '.artifacts[] | select(.name != $keep) | .id'
}

if [[ "${1:-}" == "--print-delete-ids" ]]; then
  artifact_ids_to_delete
  exit 0
fi

deleted=0
while IFS= read -r id; do
  [[ -n "$id" ]] || continue
  echo "Deleting artifact ${id}"
  gh api -X DELETE "repos/${REPO}/actions/artifacts/${id}"
  deleted=$((deleted + 1))
done < <(artifact_ids_to_delete)
echo "Deleted ${deleted} workflow artifact(s)"

if [[ "$DELETE_ALL" == "true" ]]; then
  remaining="$(artifact_count)"
  if (( remaining != 0 )); then
    echo "expected 0 artifacts after DELETE_ALL, found ${remaining}:" >&2
    artifact_names >&2
    exit 1
  fi
  echo "All workflow artifacts removed"
else
  remaining="$(artifact_count)"
  if (( remaining != 1 )); then
    echo "expected 1 artifact (${KEEP_NAME}), found ${remaining}:" >&2
    artifact_names >&2
    exit 1
  fi

  actual="$(artifact_names)"
  if [[ "$actual" != "$KEEP_NAME" ]]; then
    echo "expected artifact ${KEEP_NAME}, found ${actual}" >&2
    exit 1
  fi

  echo "Kept ${KEEP_NAME}"
fi
