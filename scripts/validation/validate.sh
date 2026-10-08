#!/usr/bin/env bash
# Everything CI runs. `pnpm validate`.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
step() { printf '\n\033[1m▸ %s\033[0m\n' "$*"; }

step "Architecture rules";   scripts/validation/check-architecture.sh
step "Contracts drift";      pnpm -s contracts --check
step "Rust format";          cargo fmt --all --check
step "Rust lint";            cargo clippy --workspace --all-targets -- -D warnings
step "Rust tests";           cargo test --workspace --quiet
step "TS format";            pnpm -s exec prettier --check .
step "TS lint";              pnpm -s exec eslint .
step "TS typecheck";         pnpm -s typecheck
step "TS tests";             pnpm -s test:js
step "Swift tests";          scripts/development/swiftpm.sh test --quiet
echo; echo "✓ all validation passed"
