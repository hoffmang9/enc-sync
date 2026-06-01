#!/usr/bin/env bash
# Canonical release target model derived from dist.toml.
#
# Source this file from other release scripts:
#   source "$(dirname "${BASH_SOURCE[0]}")/release-targets.sh"
#
# cargo-dist does not build universal macOS binaries (see axodotdev/cargo-dist#77).
# Per-arch *-apple-darwin targets are built by dist; combine-macos-universal.sh merges
# them into universal-apple-darwin before prepare-release-bundle.sh runs.
set -euo pipefail

RELEASE_TARGETS_DIST_TOML="${RELEASE_TARGETS_DIST_TOML:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/dist.toml}"
UNIVERSAL_MAC_TRIPLE="universal-apple-darwin"

_read_dist_targets() {
  _query_dist_toml targets
}

unix_archive_suffix() {
  _query_dist_toml unix-archive
}

_query_dist_toml() {
  local field=$1
  python3 - "$RELEASE_TARGETS_DIST_TOML" "$field" <<'PY'
import sys

try:
    import tomllib
except ModuleNotFoundError:
    tomllib = None


def fallback_dist_table(path):
    dist = {}
    current_table = None
    collecting_targets = False
    targets = []

    with open(path, encoding="utf-8") as config:
        for raw_line in config:
            line = raw_line.split("#", 1)[0].strip()
            if not line:
                continue
            if line.startswith("[") and line.endswith("]"):
                current_table = line.strip("[]")
                collecting_targets = False
                continue
            if current_table != "dist":
                continue
            if collecting_targets:
                if line.startswith("]"):
                    dist["targets"] = targets
                    collecting_targets = False
                    continue
                targets.extend(piece for piece in line.split('"')[1::2] if piece)
                continue
            if line.startswith("targets"):
                collecting_targets = True
                targets.extend(piece for piece in line.split('"')[1::2] if piece)
                if "]" in line:
                    dist["targets"] = targets
                    collecting_targets = False
                continue
            if "=" in line:
                key, value = line.split("=", 1)
                dist[key.strip()] = value.strip().strip('"')

    return dist


path, field = sys.argv[1], sys.argv[2]
if tomllib:
    with open(path, "rb") as config:
        dist = tomllib.load(config).get("dist", {})
else:
    dist = fallback_dist_table(path)

if field == "targets":
    for target in dist.get("targets", []):
        print(target)
elif field == "unix-archive":
    print(dist.get("unix-archive", ".tar.gz"), end="")
else:
    raise SystemExit(f"unsupported dist.toml field: {field}")
PY
}


# Print artifact suffixes shipped to users (one per line), e.g.
# x86_64-unknown-linux-musl.tar.gz or universal-apple-darwin.tar.gz
list_release_suffixes() {
  local unix_archive mac_targets=0 triple
  unix_archive="$(unix_archive_suffix)"

  while IFS= read -r triple; do
    [[ -n "$triple" ]] || continue
    case "$triple" in
      *-apple-darwin)
        mac_targets=1
        ;;
      *-pc-windows-*)
        printf '%s\n' "${triple}.zip"
        ;;
      *)
        printf '%s\n' "${triple}${unix_archive}"
        ;;
    esac
  done < <(_read_dist_targets)

  if (( mac_targets )); then
    printf '%s\n' "${UNIVERSAL_MAC_TRIPLE}${unix_archive}"
  fi
}

# Print macOS per-arch triples from dist.toml (exactly two required for lipo).
mac_per_arch_triples() {
  local triples=() triple
  while IFS= read -r triple; do
    [[ -n "$triple" ]] || continue
    case "$triple" in
      *-apple-darwin) triples+=( "$triple" ) ;;
    esac
  done < <(_read_dist_targets)

  if (( ${#triples[@]} != 2 )); then
    return 1
  fi

  printf '%s\n' "${triples[@]}"
}
