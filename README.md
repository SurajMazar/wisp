# Wisp

A local-first AI layer for macOS. Summon it, point at or draw around anything on screen, ask
by voice or text — Wisp understands that exact context, explains, highlights where to click,
and acts only when you allow it. Private by default: in **Local Only** mode nothing leaves your Mac.

> **Status:** early development — see [PROJECT_STATUS.md](PROJECT_STATUS.md) for what works today.

## Architecture in one picture

```
Wisp.app ── Swift host (AppKit/SwiftUI, WKWebView, ScreenCaptureKit, AX, audio, Keychain)
   │  stdio, typed JSON-RPC (contracts generated from Rust)
   └── wisp-core ── Rust runtime (AI orchestration, providers, policy, SQLite, memory, agents)
React 19 UI ── runs inside the host's web views, talks only through typed IPC
```

Details: [ARCHITECTURE.md](ARCHITECTURE.md) · decisions: [DECISIONS.md](DECISIONS.md) · plan: [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md)

## Install a build

Download the latest `.dmg` from [Releases](https://github.com/SurajMazar/wisp/releases), drag Wisp to
Applications, then (builds aren't notarized yet) run `xattr -dr com.apple.quarantine /Applications/Wisp.app`
or use **System Settings → Privacy & Security → Open Anyway** on first launch.

Releases are built by `.github/workflows/release.yml` when a `v*` tag is pushed.

## Requirements

- macOS 15+ on Apple Silicon (macOS 26 for Apple Foundation Models)
- Command Line Tools or Xcode (Swift 6)
- Rust (pinned by `rust-toolchain.toml`; rustup installs it automatically)
- Node 22+ and pnpm 10
- Optional: [Ollama](https://ollama.com) for local models

## Quick start

```bash
pnpm install
pnpm dev          # Vite + debug app pointed at it (hot-reloading UI)
```

```bash
pnpm build        # validate everything, then build/Wisp.app
pnpm validate     # what CI runs: architecture rules, lint, format, typecheck, all tests
```

To keep macOS privacy permissions across rebuilds, create a local signing identity once:

```bash
scripts/development/create-signing-identity.sh
```

More in [docs/development/README.md](docs/development/README.md).

## Repository

| Path | What lives there |
|---|---|
| `apps/macos` | Swift host — the only code that touches macOS APIs |
| `apps/web` | React UI |
| `crates/*` | Rust runtime (`wisp-core` binary in `crates/core`) |
| `packages/contracts` | Generated, validated IPC contracts (Zod) + typed client |
| `packages/ui`, `icons`, `config`, `test-utils` | Design system and shared tooling |
| `database/migrations` | Forward-only SQL migrations embedded into `wisp-core` |
| `models/manifests` | Model metadata + checksums (never binaries) |
| `docs/` | Subsystem documentation |

## License

Not yet licensed — see [LICENSE](LICENSE).
