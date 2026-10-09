#!/usr/bin/env bash
# Download a model listed in models/manifests/*.json into the Wisp data directory, verifying
# size and SHA-256 of every file. Files are written to a .partial and moved into place only
# after verification. Resumes interrupted downloads.
#   scripts/models/download-model.sh whisper-base.en
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
id="${1:?usage: download-model.sh <model-id>}"
data="${WISP_DATA_DIR:-$HOME/Library/Application Support/Wisp}"
dest="$data/models/$id"

entry="$(python3 -I - "$root/models/manifests" "$id" <<'PY'
import json, sys, pathlib
for m in pathlib.Path(sys.argv[1]).glob("*.json"):
    for e in json.loads(m.read_text())["models"]:
        if e["id"] == sys.argv[2]:
            for f in e["files"]:
                assert f["url"].startswith("https://huggingface.co/"), "untrusted source"
                print(f["path"], f["url"], f["sha256"], f["sizeBytes"])
            sys.exit(0)
sys.exit(f"unknown model {sys.argv[2]}")
PY
)"

mkdir -p "$dest"
while read -r path url sha size; do
  out="$dest/$path"
  mkdir -p "$(dirname "$out")"
  if [[ -f "$out" ]] && [[ "$(shasum -a 256 "$out" | cut -d' ' -f1)" == "$sha" ]]; then
    echo "✓ $path (already verified)"; continue
  fi
  free=$(df -k "$data" | awk 'NR==2 {print $4 * 1024}')
  (( free > size * 2 )) || { echo "not enough disk space for $path" >&2; exit 1; }
  echo "↓ $path ($((size / 1000000)) MB)"
  curl -fL --retry 3 -C - -o "$out.partial" "$url"
  [[ "$(stat -f%z "$out.partial")" == "$size" ]] || { echo "size mismatch for $path" >&2; exit 1; }
  [[ "$(shasum -a 256 "$out.partial" | cut -d' ' -f1)" == "$sha" ]] || { echo "checksum mismatch for $path" >&2; rm -f "$out.partial"; exit 1; }
  mv "$out.partial" "$out"
  echo "✓ $path"
done <<< "$entry"
echo "✓ $id → $dest"
