# Testing

| Layer | Where | Runs |
|---|---|---|
| Rust unit | `#[cfg(test)]` in each crate | `cargo test --workspace` |
| Rust ↔ protocol | `crates/core/tests/stdio.rs` — spawns the real binary | `cargo test -p wisp-core` |
| TS contracts | `packages/contracts/tests` | `pnpm --filter @wisp/contracts test` |
| React (mock core) | `apps/web/tests/*.test.tsx` with `MockTransport` | `pnpm --filter @wisp/web test` |
| React ↔ real core | `apps/web/tests/settings.core.test.tsx` | same; needs `cargo build -p wisp-core` |
| Swift unit + real core | `apps/macos/Tests/WispTests` (Swift Testing) | `scripts/development/swiftpm.sh test` |
| Cross-layer | `tests/e2e/run.sh` | `pnpm e2e` |
| Manual (TCC, hardware) | `docs/release/manual-checklist.md` | a person |

Rules: tests use temporary data directories (`WISP_DATA_DIR` / `--data-dir`), never the real profile.
Mock providers exist only in tests (spec §127). Failing tests are never skipped to make a gate pass.
