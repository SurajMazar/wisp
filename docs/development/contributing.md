# Contributing

- Run `pnpm validate` before pushing; CI runs the same script.
- Never edit an applied migration — add `database/migrations/NNNN_name.sql`.
- Never edit generated contracts — change Rust types and run `pnpm contracts`.
- No `any`, no `unwrap()`/`expect()` outside tests, no suppressed lints without a comment saying why.
- Keep platform APIs in `apps/macos`, SQL in `crates/database`, policy in `crates/permissions`.
- Every user-visible feature needs loading, empty, error, success, permission and disabled states.
- Update PROJECT_STATUS.md when a phase's state changes.
