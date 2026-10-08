#!/usr/bin/env bash
# Development loop: Vite dev server + debug app bundle pointed at it.
# The React UI hot-reloads; restart this script after Swift or Rust changes.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

scripts/build/build-macos.sh --debug --no-web

pnpm --filter @wisp/web dev &
vite_pid=$!
trap 'kill $vite_pid 2>/dev/null || true; pkill -x Wisp 2>/dev/null || true' EXIT

# Wait for Vite.
for _ in $(seq 1 100); do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 0.1; done

open -n -W build/debug/Wisp.app --env WISP_DEV_URL=http://127.0.0.1:5173 --env WISP_LOG="${WISP_LOG:-debug}" \
  ${WISP_DATA_DIR:+--env WISP_DATA_DIR="$WISP_DATA_DIR"}
