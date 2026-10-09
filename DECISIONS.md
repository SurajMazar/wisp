# Decisions

Architecture Decision Records. Newest at the bottom. Each records context, the decision, and what we gave up.

---

## D-001 — Product name: Wisp
**Date** 2026-10-08 · **Status** accepted
Original branding required (spec §3). Chosen by the project owner. Bundle id `com.wisp.assistant`; data dir `~/Library/Application Support/Wisp`.

## D-002 — Two processes: Swift host + Rust `wisp-core` child process
**Status** accepted
**Context.** Spec wants Swift for platform, Rust for runtime, and crash resilience for model failures. Options: (a) Rust as a static library linked via C ABI; (b) Rust as a child process over pipes; (c) Tauri (Rust hosts the app).
**Decision.** (b). Host spawns `Contents/MacOS/wisp-core`, talks newline-delimited JSON-RPC over stdio, supervises and restarts it.
**Why.** whisper.cpp / ONNX Runtime faults can't take down the UI; Rust core is independently runnable and testable (integration tests drive it over the same protocol the host uses); no xcframework packaging (which needs Xcode). Tauri was rejected because overlays, event taps, AX and ScreenCaptureKit are first-class in AppKit and awkward from a Rust-hosted webview.
**Cost.** Serialization overhead on hot paths (PCM, images). Mitigated: images are cropped/compressed in Swift first; PCM is 16 kHz mono i16 (~43 KB/s base64). Revisit with a binary side-channel if profiling demands it (Phase 19).

## D-003 — Contracts: Rust types are canonical, generated into Zod and Swift
**Status** accepted
**Context.** Spec forbids duplicating shared types across languages and requires Zod at runtime.
**Decision.** IPC types live in `crates/ipc` with `serde` + `schemars`. A generator binary emits JSON Schema; a TypeScript script emits Zod schemas (`packages/contracts/src/generated`) and Swift Codable types (`apps/macos/Sources/WispIPC/Generated`). Generated files are committed and drift-checked by `pnpm validate`.
**Cost.** We own a small code generator covering the schema subset we use (objects, string enums, internally-tagged unions, arrays, optionals, maps, primitives). Kept deliberately narrow.

## D-004 — React UI hosted in WKWebView via `wisp://` scheme
**Status** accepted
Main window and floating assistant are WKWebViews in native windows (NSWindow / non-activating NSPanel). Assets served by a `WKURLSchemeHandler` from `Resources/web` — no file:// quirks, no local HTTP server. In development the host loads `WISP_DEV_URL` (Vite). Cursor, focus and annotation overlays are native (Core Animation) because they must be click-through, per-display, and sub-frame responsive.

## D-005 — Build without Xcode
**Status** accepted (environment constraint)
The dev machine has Command Line Tools only. The host is a SwiftPM executable; `scripts/build/build-macos.sh` assembles `Wisp.app` (Info.plist, entitlements, binaries, web assets) and code-signs it. XCUITest and notarization need Xcode and are deferred to Phase 21 with a documented path.
**Consequence — TCC.** Ad-hoc signatures change every build, so macOS forgets Screen Recording / Accessibility grants after each rebuild. `scripts/development/create-signing-identity.sh` creates a local self-signed code-signing identity (run by the developer, it touches the login keychain) so grants persist across builds. The build script uses it when present.

## D-006 — Modifier-only default shortcut (⌘ + ⌥) via CGEventTap
**Status** accepted
Carbon `RegisterEventHotKey` cannot register modifier-only chords. ⌘+⌥ is detected with a listen-only `CGEventTap` on `flagsChanged`, which requires the Accessibility (or Input Monitoring) permission. Chords with a key (e.g. ⌥Space) use Carbon hot keys and need no permission. Until Accessibility is granted, the menu bar item and a Carbon fallback chord remain available, and onboarding explains why. Hold vs. toggle is decided by press duration and the configured behavior. Chord "taint" rule: if any other key is pressed while ⌘⌥ is held, the invocation is cancelled so ⌘⌥-based app shortcuts keep working.

## D-007 — SQLite via rusqlite, embedded migrations, brute-force vectors
**Status** accepted
`rusqlite` with the `bundled` feature (no system SQLite drift). Migrations embedded at compile time and checksummed. Vector search: cosine similarity over `f32` BLOBs behind a `VectorStore` trait. sqlite-vec was considered; brute force is simpler, dependency-free and fast enough at desktop scale. Swap later behind the trait if needed.

## D-008 — STT: whisper.cpp (via `whisper-rs`), not faster-whisper
**Status** accepted
faster-whisper needs a Python runtime + CTranslate2 — poor for app distribution. whisper.cpp is a C/C++ library with Metal acceleration on Apple Silicon and GGML models on Hugging Face (`ggerganov/whisper.cpp`, MIT). Presets: Fast = `base.en`/`base`, Balanced = `small`, High Quality = `large-v3-turbo` (q5). Checksums pinned in `models/manifests/stt.json`.

## D-009 — TTS: Kokoro-82M via ONNX Runtime (`ort`); VAD: Silero via `ort`
**Status** accepted, with open risk
Kokoro-82M (Apache-2.0) ONNX export runs on ONNX Runtime with CoreML/CPU. Silero VAD (MIT) uses the same runtime, so one native dependency covers both.
**Open risk — phonemization.** Kokoro consumes phonemes. The reference G2P (misaki) falls back to espeak-ng, which is GPL-3.0 and must not be linked into Wisp. Plan: dictionary-based English G2P (CMUdict → Kokoro phoneme set) plus rule-based fallback for OOV words, implemented in Rust. If quality is insufficient, ship espeak-ng as a separately-installed optional component. Decision to be finalized at Phase 8 and recorded as D-0xx.

## D-010 — Apple Foundation Models live in the Swift host
**Status** accepted
`FoundationModels` is a Swift-only framework. The Rust `apple` provider proxies `generate` / `generateStructured` to `host.apple.*`, streaming chunks back as events. Availability is read from `SystemLanguageModel.default.availability` — never assumed. Verified available on the dev machine (2026-10-08).

## D-011 — Cloud providers over HTTPS from Rust with keys fetched from Keychain per request
**Status** accepted
Rust requests a secret from the host (`host.keychain.get`) when it needs it and holds it only for the request's lifetime. Keys are never written to SQLite, config files or logs.

## D-012 — Package manager and toolchains
**Status** accepted
pnpm workspaces (`apps/web`, `packages/*`); Rust stable pinned in `rust-toolchain.toml`; Swift 6 language mode with strict concurrency.

## D-013 — Hardened runtime, no App Sandbox
**Status** accepted
Accessibility-based control, `CGEventTap` and cross-app automation don't work inside the App Sandbox. Wisp ships with the hardened runtime and only the entitlements it needs (`device.audio-input`, `automation.apple-events`); distribution is Developer ID + notarization (Phase 21). Least privilege is enforced by Wisp's own policy engine and macOS TCC prompts.

## D-014 — Work around the stale Command Line Tools manifest API
**Status** accepted (environment)
The dev machine's CLT has 2024-era `PackageDescription.private.swiftinterface` files that shadow the current manifest API, so every `Package.swift` fails to compile. `scripts/development/swiftpm.sh` detects this and points SwiftPM at a cleaned user-space copy (`SWIFTPM_CUSTOM_LIBS_DIR`), and adds Swift Testing's framework/rpath flags that CLT omits. On a healthy toolchain it is a pass-through. The real fix — reinstalling the CLT or installing Xcode — is the developer's call.

## D-015 — TypeScript 6.0, not 7.0
**Status** accepted
TypeScript 7 (native port) is current, but `typescript-eslint` supports `<6.1`. Pinned to 6.0.3 until lint tooling supports 7.

## D-016 — Generated contracts are committed
**Status** accepted
Generated Zod and Swift files are checked in so the Swift and web builds don't require Rust; `pnpm validate` fails on drift.

## D-017 — Trackpad invocation is unsupported; mouse side buttons are supported
**Status** accepted (platform limitation, spec §17/§128)
macOS offers no public API to observe trackpad gestures system-wide: `NSEvent` global monitors don't
deliver gesture events reliably and gesture types aren't available to `CGEventTap`. Wisp shows the
trackpad channel as "Not supported by macOS" and suggests a shortcut or mouse button instead. Auxiliary
mouse buttons (middle/back/forward) arrive as `otherMouseDown` in the same listen-only event tap used
for ⌘⌥, so they're supported under the same Accessibility/Input Monitoring permission.

## D-018 — Floating assistant is a non-activating panel
**Status** accepted
The assistant must appear over the user's work without stealing focus (hold mode keeps typing in the
current app). It's an `NSPanel` with `.nonactivatingPanel`, joins all Spaces and full-screen apps, and
becomes key only in toggle mode or when the user clicks into it. Content is the React `/assistant`
route over native vibrancy (transparent WKWebView via the `drawsBackground` KVC key — no public API).

## D-019 — The buddy is always present; the panel expands out of it
**Status** accepted (product direction from the owner, 2026-10-09)
The assistant cursor is the product's front door, as in HeyClicky: by default it's always beside the
pointer and clicking it expands the floating assistant from that spot (Esc / click-away collapses it).
Holding the invocation shortcut makes the buddy attentive instead of opening a window. "When summoned"
visibility remains available.
Mechanics: the buddy is a borderless non-activating panel at status-bar level, positioned every frame
from `NSEvent.mouseLocation` (no permission needed), with a spring follow and an *approach hold* — it
holds still when the pointer reaches for it slowly (<320 pt/s) so it can be clicked, but never for fast
movement. It drops to 10 Hz polling when the pointer rests. It is excluded from screen capture
(`sharingType = .none`) so Wisp never sees itself in screenshots; `WISP_BUDDY_CAPTURABLE=1` overrides
this for verification and demos.
