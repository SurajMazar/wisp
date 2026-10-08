#!/usr/bin/env bash
# Cross-layer tests that run without a GUI session:
#   Rust:  host protocol → wisp-core → SQLite (restart persistence, bad input)
#   React: UI → IPC client → real wisp-core → SQLite (restart persistence)
#   Swift: CoreProcess supervisor → real wisp-core (restart, crash recovery)
# Flows needing TCC permissions or a person at the Mac are in docs/release/manual-checklist.md.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q -p wisp-core
cargo test -q -p wisp-core --test stdio
pnpm --filter @wisp/web exec vitest run tests/settings.core.test.tsx
scripts/development/swiftpm.sh test --filter CoreProcessTests
