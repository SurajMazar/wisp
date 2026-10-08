#!/usr/bin/env bash
# Full release build: validation, then the signed app bundle.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
"$root/scripts/validation/validate.sh"
"$root/scripts/build/build-macos.sh"
