#!/usr/bin/env bash
# Enforces the dependency rules in spec §42.26 / ARCHITECTURE.md. Fails on any violation.
set -euo pipefail
cd "$(dirname "$0")/../.."
fail=0
violation() { echo "✗ $*" >&2; fail=1; }

# React must not reach platform APIs, the database or secrets directly.
if grep -rEn "from ['\"](node:)?(fs|child_process|sqlite|better-sqlite3)['\"]|window\.webkit" apps/web/src \
  | grep -v "^apps/web/src/.*\.test\." ; then
  violation "apps/web/src must not use Node APIs, SQLite or the raw WebKit bridge (use @wisp/contracts)"
fi

# SQL lives only in crates/database.
if grep -rEn "\b(SELECT|INSERT INTO|UPDATE [a-z_]+ SET|DELETE FROM|CREATE TABLE)\b" crates --include=*.rs \
  | grep -v "^crates/database/" ; then
  violation "SQL found outside crates/database"
fi

# Rust crates must not depend on UI or the composition root.
for c in crates/*/Cargo.toml; do
  [[ "$c" == "crates/core/Cargo.toml" ]] && continue
  if grep -q "wisp-core" "$c"; then violation "$c depends on wisp-core (composition root)"; fi
done
for c in crates/database crates/ai crates/providers; do
  if grep -rqE "react|webkit|swift" "$c/Cargo.toml"; then violation "$c must not depend on UI"; fi
done

# Providers must not depend on agents, and the agent runtime must not know specific providers.
grep -q "wisp-agents" crates/providers/Cargo.toml && violation "providers must not depend on agents"
grep -q "wisp-providers" crates/agents/Cargo.toml && violation "agents must not depend on concrete providers"

# No API keys or secrets committed.
secrets="$(git ls-files -z --cached --others --exclude-standard 2>/dev/null \
  | xargs -0 grep -lE "sk-ant-[A-Za-z0-9]{16,}|sk-proj-[A-Za-z0-9]{16,}|AIza[0-9A-Za-z_-]{30,}" 2>/dev/null || true)"
if [[ -n "$secrets" ]]; then
  echo "$secrets" >&2
  violation "possible secret in repository files"
fi

# Model binaries must not be committed.
if find models -type f \( -name "*.bin" -o -name "*.gguf" -o -name "*.onnx" -o -name "*.safetensors" \) | grep -q .; then
  violation "model binaries found under models/ (they belong in Application Support)"
fi

[[ $fail == 0 ]] && echo "✓ architecture rules hold"
exit $fail
