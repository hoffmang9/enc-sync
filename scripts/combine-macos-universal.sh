#!/usr/bin/env bash
# Combine per-arch macOS dist archives into one universal-apple-darwin tarball.
set -euo pipefail

MANIFEST="${1:-dist-manifest.json}"
DISTRIB="${DISTRIB_DIR:-target/distrib}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ ! -f "$MANIFEST" ]]; then
  echo "manifest not found: $MANIFEST" >&2
  exit 1
fi

ARM_ARCHIVE=""
X64_ARCHIVE=""
while IFS= read -r archive; do
  case "$archive" in
    *aarch64-apple-darwin*) ARM_ARCHIVE="$archive" ;;
    *x86_64-apple-darwin*) X64_ARCHIVE="$archive" ;;
  esac
done < <(find "$DISTRIB" -maxdepth 1 -type f \( \
  -name 'enc-sync-aarch64-apple-darwin.tar.*' -o \
  -name 'enc-sync-x86_64-apple-darwin.tar.*' \
\))

if [[ -z "$ARM_ARCHIVE" && -z "$X64_ARCHIVE" ]]; then
  echo "macOS per-arch archives not found; skipping universal merge"
  exit 0
fi

if [[ -z "$ARM_ARCHIVE" || -z "$X64_ARCHIVE" ]]; then
  echo "expected both aarch64 and x86_64 macOS archives" >&2
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

extract_archive() {
  local archive=$1 dest=$2
  mkdir -p "$dest"
  case "$archive" in
    *.tar.gz) tar xzf "$archive" -C "$dest" ;;
    *.tar.xz) tar xJf "$archive" -C "$dest" ;;
    *.tar.zst) tar --zstd -xf "$archive" -C "$dest" ;;
    *) echo "unsupported archive format: $archive" >&2; exit 1 ;;
  esac
}

extract_archive "$ARM_ARCHIVE" "$WORK/arm"
extract_archive "$X64_ARCHIVE" "$WORK/x64"

ARM_BIN="$(find "$WORK/arm" -type f -name enc-sync | head -1)"
X64_BIN="$(find "$WORK/x64" -type f -name enc-sync | head -1)"

if [[ -z "$ARM_BIN" || -z "$X64_BIN" ]]; then
  echo "failed to locate enc-sync binaries in macOS archives" >&2
  exit 1
fi

STAGING="$WORK/enc-sync-universal-apple-darwin"
mkdir -p "$STAGING"

# Static files live one directory below the tar root.
ARM_ROOT="$(dirname "$ARM_BIN")"
cp -a "$ARM_ROOT/." "$STAGING/"
lipo -create -output "$STAGING/enc-sync" "$X64_BIN" "$ARM_BIN"
chmod +x "$STAGING/enc-sync"

UNIVERSAL_ARCHIVE="$DISTRIB/enc-sync-universal-apple-darwin.tar.gz"
tar czf "$UNIVERSAL_ARCHIVE" -C "$WORK" enc-sync-universal-apple-darwin

CHECKSUM_FILE="${UNIVERSAL_ARCHIVE}.sha256"
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$UNIVERSAL_ARCHIVE" >"$CHECKSUM_FILE"
else
  shasum -a 256 "$UNIVERSAL_ARCHIVE" >"$CHECKSUM_FILE"
fi

rm -f "$ARM_ARCHIVE" "${ARM_ARCHIVE}.sha256" "$X64_ARCHIVE" "${X64_ARCHIVE}.sha256"

ABS_UNIVERSAL="$(cd "$(dirname "$UNIVERSAL_ARCHIVE")" && pwd)/$(basename "$UNIVERSAL_ARCHIVE")"
ABS_CHECKSUM="$(cd "$(dirname "$CHECKSUM_FILE")" && pwd)/$(basename "$CHECKSUM_FILE")"

tmp="$(mktemp)"
jq \
  --arg uni "$ABS_UNIVERSAL" \
  --arg chk "$ABS_CHECKSUM" \
  --arg uni_name "$(basename "$UNIVERSAL_ARCHIVE")" \
  '
  .artifacts |= (
    map(select((.target_triples[0] // "") | test("apple-darwin") | not))
    + [
      {
        "kind": "executable-zip",
        "target_triples": ["universal-apple-darwin"],
        "path": $uni,
        "checksum": ($uni_name + ".sha256")
      },
      {
        "kind": "checksum",
        "target_triples": ["universal-apple-darwin"],
        "path": $chk
      }
    ]
  )
  | .upload_files |= (
    map(select(test("apple-darwin") | not))
    + [$uni, $chk]
  )
  ' "$MANIFEST" >"$tmp"
mv "$tmp" "$MANIFEST"

echo "Created universal macOS archive: $UNIVERSAL_ARCHIVE"
lipo -info "$STAGING/enc-sync"
