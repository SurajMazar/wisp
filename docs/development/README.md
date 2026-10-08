# Development

## Setup

```bash
pnpm install                 # JS workspace
cargo build -p wisp-core     # Rust core (toolchain installs from rust-toolchain.toml)
pnpm dev                     # Vite on :5173 + build/debug/Wisp.app loading it
```

`pnpm dev` builds a debug bundle and launches it with `WISP_DEV_URL`, so React hot-reloads. Rerun
after Swift or Rust changes.

### Environment variables

| Variable | Effect |
|---|---|
| `WISP_DEV_URL` | Load the UI from a local dev server (127.0.0.1/localhost only) |
| `WISP_DATA_DIR` | Use another data directory (tests, clean-profile runs) |
| `WISP_CORE_BIN` | Use a specific `wisp-core` binary |
| `WISP_LOG` | `error`/`warn`/`info`/`debug`/`trace` for wisp-core logs |
| `WISP_SIGN_IDENTITY` | Code-signing identity for `build-macos.sh` |

### Logs

- Core: `~/Library/Application Support/Wisp/logs/wisp-core.log.<date>` (redacted)
- Host: `log stream --predicate 'subsystem == "com.wisp.assistant"'`

### Swift without Xcode

Use `scripts/development/swiftpm.sh build|test` instead of calling `swift` directly. It works around
a stale Command Line Tools manifest API and adds Swift Testing's paths (DECISIONS.md D-014).

### Permissions during development

Ad-hoc signed builds lose Screen Recording / Accessibility / Microphone grants on every rebuild.
Run `scripts/development/create-signing-identity.sh` once; subsequent builds are signed with
"Wisp Development" and grants persist.

## Commands

| Command | Does |
|---|---|
| `pnpm validate` | Everything CI runs |
| `pnpm test` | JS, Rust and Swift tests |
| `pnpm e2e` | Cross-layer tests against the real core |
| `pnpm contracts` | Regenerate TS/Swift contracts from Rust |
| `pnpm build` | Validate + release `build/Wisp.app` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Rust lint |
