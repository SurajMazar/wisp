# Wisp — Architecture

Wisp is a local-first AI layer for macOS. You summon it, point at or draw around part of the screen, ask a question (by voice or text), and it understands, explains, highlights, and — only when authorized — acts.

This document is the map. Decisions and their trade-offs live in [DECISIONS.md](DECISIONS.md); the build order lives in [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md); current truth lives in [PROJECT_STATUS.md](PROJECT_STATUS.md).

## 1. Process model

One application bundle, two processes, one database:

```
Wisp.app/Contents/
├── MacOS/
│   ├── Wisp          ← Swift host (AppKit + SwiftUI). Owns every macOS API.
│   └── wisp-core     ← Rust runtime. Owns logic, data, models, policy.
└── Resources/
    └── web/          ← Built React UI, served to WKWebView via wisp:// scheme
```

```
┌──────────────────────────── Wisp (Swift host) ─────────────────────────────┐
│                                                                            │
│  Native surfaces                     Web surfaces (WKWebView)              │
│  ─────────────────                   ─────────────────────────             │
│  Menu bar item                       Main window   (React, route /*)       │
│  Assistant cursor  (NSPanel, CA)     Floating assistant (NSPanel, /assist) │
│  Focus overlay     (per-display)                                           │
│  Annotation overlay(per-display)            ▲  window.webkit.messageHandlers│
│                                             │  + evaluateJavaScript events  │
│  Platform services                          ▼                              │
│  Invocation (CGEventTap / Carbon)    ┌──────────────┐                      │
│  ScreenCaptureKit                    │  IPC Router  │◄── validates, routes │
│  Accessibility (AXUIElement)         └──────┬───────┘                      │
│  Audio I/O (AVAudioEngine)                  │ host.* methods handled here  │
│  Keychain, Notifications, TCC               │ everything else → core       │
│  Apple Foundation Models                    │                              │
└─────────────────────────────────────────────┼──────────────────────────────┘
                                              │ stdin/stdout, framed JSON-RPC
                                              │ (versioned, schema-validated)
┌─────────────────────────────────────────────▼──────────────────────────────┐
│ wisp-core (Rust)                                                           │
│                                                                            │
│  ipc ──► core (command dispatch, event bus, lifecycle, config)             │
│            │                                                               │
│            ├── ai ─────────── router · context builder · orchestrator      │
│            │     └── providers: ollama · openai-compat · anthropic ·       │
│            │                    gemini · apple (proxied to host) · mock    │
│            ├── audio ──────── VAD · STT (whisper.cpp) · TTS (Kokoro) ·     │
│            │                  sentence streamer                            │
│            ├── screen ─────── focus regions · coordinates · crop/compress  │
│            ├── accessibility─ AX node normalization · relevance filtering  │
│            ├── computer ───── tool definitions for mouse/keyboard/apps     │
│            ├── agents ─────── loop · planner · verifier · limits           │
│            ├── memory ─────── extraction · retrieval · ranking             │
│            ├── files ──────── parse · chunk · embed · vector search        │
│            ├── permissions ── policy engine · risk · approvals · audit     │
│            ├── security ───── redaction · path/URL/shell validation        │
│            └── database ───── SQLite · migrations · repositories           │
└────────────────────────────────────────────────────────────────────────────┘
                     │
                     ▼
     ~/Library/Application Support/Wisp/   (database, models, files, logs …)
     macOS Keychain                         (API keys — never in SQLite)
```

### Why two processes

Local inference stacks (whisper.cpp, ONNX Runtime) are C/C++. A segfault in one of them must not take the menu bar, overlays, or the user's in-flight UI down with it (spec §106: *"One failed provider must not crash the application"*). The Swift host supervises `wisp-core`, restarts it with backoff, and surfaces a calm error state while it does. TCC attributes the child's activity to `Wisp.app`, so permissions are still granted once, to Wisp.

This is a child process with a pipe, not a service: no ports, no network listener in production.

## 2. Ownership (spec §42.29)

| Feature | Owner | Notes |
|---|---|---|
| Conversation / settings / agent / model UI | React | Talks only to typed IPC client |
| Floating assistant panel | Swift window + React content | NSPanel for native behavior, React for rendering |
| Assistant cursor, focus overlay, annotation overlay | Swift | Core Animation, per-display windows, click-through |
| Global invocation | Swift | CGEventTap for modifier-only chords, Carbon hot keys otherwise |
| Screen capture | Swift | ScreenCaptureKit; pixels cropped/compressed before leaving the host |
| Accessibility tree, element under pointer | Swift | Raw AX → normalized nodes; filtering in Rust |
| Mouse/keyboard/app actions | Swift executes, Rust authorizes | Rust policy engine must approve every call |
| Audio devices, capture, playback | Swift | PCM frames streamed to/from core |
| Apple Foundation Models | Swift executes | Exposed to Rust as the `apple` provider |
| AI orchestration, routing, providers | Rust | |
| STT / TTS orchestration and inference | Rust | whisper.cpp, Kokoro via ONNX Runtime |
| Database, memory, files, agents, skills, routines | Rust | |
| Permissions policy | Rust (+ Swift for TCC status) | |
| Secrets | Swift (Keychain) | Rust requests a key by reference; never persisted by Rust |

## 3. IPC

Three hops, one contract:

```
React ──postMessage──► Swift IPC Router ──frame──► wisp-core
  ▲                        │    ▲                     │
  └────── events ──────────┘    └──── host.* calls ───┘
```

* **Envelope** (`packages/contracts`, `crates/ipc`, `apps/macos/Sources/WispIPC`):
  `{ v: 1, kind: "request"|"response"|"event", id?, method?, params?, result?, error?, event?, payload? }`
* **Methods are namespaced**: `settings.get`, `conversation.send`, `models.list`, … are core methods. `host.*` methods (`host.screen.capture`, `host.ax.elementAt`, `host.apple.generate`, `host.input.click` …) are implemented by Swift and may be called by core.
* **Single source of truth**: message types are Rust structs/enums deriving `serde` + `schemars`. `cargo run -p wisp-ipc --bin contracts-gen` emits `packages/contracts/schema/wisp.schema.json`; `scripts/validation/gen-contracts.ts` turns that schema into Zod (TypeScript) and Codable (Swift). `pnpm validate` fails if generated code drifts.
* **Validation everywhere**: React validates responses/events with Zod; Swift decodes with Codable and rejects unknown methods; Rust validates all params (serde + explicit semantic validation of paths, URLs, tool names, model ids).
* **Framing** (Swift ↔ Rust): newline-delimited UTF-8 JSON, one message per line, 16 MiB cap. Large binary payloads (PCM, PNG) are base64 inside typed fields; screenshots are cropped and compressed in Swift first.

## 4. Data

* SQLite via `rusqlite` (bundled), WAL mode, foreign keys on.
* Migrations: `database/migrations/NNNN_name.sql`, embedded into `wisp-core` at compile time, applied in order inside a transaction, recorded in `schema_migrations` with checksum. A changed checksum on an applied migration is a hard error.
* Repositories in `crates/database` are the only code that writes SQL.
* Vectors: `f32` BLOBs in `memory_embeddings` / `file_chunks`, brute-force cosine in Rust behind a `VectorStore` trait (adequate to ~10⁵ chunks; swappable).
* User data root: `~/Library/Application Support/Wisp/{database,models,files,embeddings,skills,logs,cache,recordings,exports}`. Overridable with `WISP_DATA_DIR` for tests.

## 5. Privacy enforcement

Privacy is enforced in one place — `ai::router` consults `permissions::PrivacyPolicy` before any provider call:

* **Local Only** — router filters out every provider with `locality = Cloud`. No fallback to cloud, ever. Tested.
* **Local Preferred** — cloud candidates require an approval event (`privacy.cloudApprovalRequired`) resolved by the user, per request.
* **Cloud Allowed** — configured cloud providers are eligible.

Every outbound payload is classified (`screenshot`, `audio`, `file`, `clipboard`, `accessibility`, `memory`) and recorded in the activity log (metadata only, never content).

## 6. Security model (summary — full detail in docs/security)

* No arbitrary shell. Shell tool parses commands, classifies risk, blocks destructive patterns by default, requires approval, runs with a restricted environment and working directory.
* Every tool declares a JSON schema, risk level and permission requirement; arguments are validated before policy evaluation; policy decisions are audited.
* Agents never gain permissions implicitly; grants are scoped (user / agent / tool / session).
* Paths are canonicalized and checked against allowed roots; imported filenames sanitized; imported skills never executed.
* Secrets live in Keychain, referenced by id; the logger redacts known secret shapes.

## 7. Repository layout

See spec §42.1; implemented as:

```
apps/macos      Swift host (SwiftPM)          crates/*       Rust workspace
apps/web        React 19 + Vite UI            packages/*     contracts, ui, config, icons, test-utils
database/       migrations, seeds, fixtures   models/        manifests (no binaries)
skills/         builtin + examples            scripts/       build, dev, release, models, validation
tests/          e2e, integration, fixtures    docs/          per-subsystem docs
```

## 8. Build

* `pnpm build` → builds contracts, web UI, `wisp-core` (release), the Swift host, then assembles and signs `build/Wisp.app` (`scripts/build/build-macos.sh`).
* No Xcode required for development: the host is a SwiftPM executable wrapped into a bundle by script. Xcode is needed later only for XCUITest and notarization tooling.
* `pnpm dev` → Vite dev server + debug `wisp-core`; the host loads `http://localhost:5173` when `WISP_DEV_URL` is set.
