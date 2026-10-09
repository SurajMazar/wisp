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

## D-020 — Focus interaction and capture policy
**Status** accepted
* **Hold the shortcut, then drag** draws the configured shape (rectangle default, lasso, circle); a
  **click** focuses a point (a square of `pointSize` around it, shifted to stay on screen). Releasing the
  shortcut **without drawing captures nothing** — summoning alone never takes a screenshot.
* The assistant panel offers Area / Point / Window / Screen buttons, so focus works without the
  Accessibility permission the global shortcut needs.
* One capture per completed selection via `SCScreenshotManager` at the display's native scale, with
  Wisp's own windows excluded. The crop is the selection plus a configurable margin, snapped to whole
  points and clipped to the display, so pixel size is exactly `crop × scale`.
* Captures are PNGs in `<data>/cache/focus`, pruned to the 12 most recent, served to the UI only via
  `wisp://focus/<uuid>.png` (PNG-only, confined). The core keeps the latest region **in memory** and logs
  metadata only (kind, display, size) — never pixels.
* The overlay is one non-activating panel per display above the menu bar, excluded from capture, and
  torn down on every exit path (finish, Esc, display change, 60 s timeout, 2 s post-finish safety).

## D-021 — Kokoro phonemes via espeak-ng as a separate process (resolves D-009's open risk)
**Status** accepted
Kokoro needs IPA phonemes. Wisp invokes `espeak-ng` (GPL-3.0) as an external process found on the
system (Homebrew) — never linked or bundled — and maps its IPA to Kokoro's vocabulary (port of Kokoro's
reference normalization). If espeak-ng is missing, Voice settings say so with the exact install command;
recognition and everything else still work. Verified: Kokoro output transcribed back by Whisper as
"Click the red delete button to remove the file."

## D-022 — Hold the shortcut = point *and* talk
**Status** accepted
Holding ⌥⌘ listens (microphone only while held) and arms the focus overlay. On release, the recording is
VAD-trimmed and transcribed with whisper.cpp; if there was speech, it's asked as a question — with the
area drawn during that hold attached — and the answer is spoken sentence by sentence while it streams.
Silence asks nothing. Holding again interrupts speech. "Push to Talk" behavior is the same without the
overlay. Speech recognition and Kokoro load at startup so the first question is instant.

## D-023 — Provider fallback and "thinking" models
**Status** accepted
If the routed model fails before producing text, the next eligible model answers (privacy rules are
re-applied on every attempt) and the failed provider cools down for two minutes (spec §99). Ollama's
`qwen3-vl:8b` tag is the *thinking* variant, which deliberates silently for tens of seconds and ignores
`think:false`; the router prefers models that answer immediately and Wisp recommends the `-instruct`
variants. Apple Foundation Models can refuse non-foreground callers (ModelManagerError 1013); fallback
covers it.

## D-024 — The buddy is a second cursor, never a control (supersedes the click part of D-019)
**Status** accepted (owner feedback, 2026-10-09)
Like HeyClicky, the assistant cursor looks like an actual pointer (accent-colored arrow with a soft glow)
trailing the real cursor's tip diagonally below-right. It is fully click-through and never hovers, holds
still or dodges — anything else distracts from moving the mouse. The panel opens with a **quick tap of the
shortcut** (< 350 ms, no other key), the menu bar, or automatically when there's an answer; **holding**
remains talk-and-point, and a hold that turns out to be a tap discards its recording unheard.

## D-025 — Buddy shapes and gradient glow
**Status** accepted (owner direction, 2026-10-09)
Default shape is Wisp's flat arrowhead (rounded triangle with a deep V-notch, pointing up-left toward
the real cursor); users can pick Pointer or Dot. Flat fill in the cursor color with a hairline edge — no
3D extrusion or gloss (explicitly rejected). The glow is either the cursor color or a two-color gradient
(presets Aurora, Sunset, Ocean, Mint, Ember, or custom colors), rendered as the gradient-filled shape
blurred with Core Image, because layer shadows can only be a single color.

## D-026 — Screen understanding: on-device OCR, text fallback, fenced screen content
**Status** accepted (2026-10-09)
Every capture is also read with Apple Vision text recognition on device (lines with global-point frames, reading
order). The context builder (`wisp-core::context`) sizes the prompt from the model's context length: newest turns
verbatim, older turns condensed into a summary appended to the system prompt (no extra LLM call), and the screen
described compactly by `wisp-screen::describe` (app/window, ranked UI elements, selected text, recognized text
within a budget). When no allowed model can see images — or the user picked a text-only model — and the region is
readable, the screenshot is not sent and the model answers from the text (so Apple's on-device model can answer
about the screen). Follow-ups within six messages reuse the cached description instead of recapturing. Screen
content is untrusted: it is fenced in `<screen>` / `"""` with control characters stripped, and the system prompt
says it is information, never instructions. Each answer records a `ContextInfo` shown in a "Context" disclosure.

## D-027 — Guidance targets are named, then resolved locally
**Status** accepted (2026-10-09)
Small local models are poor at predicting pixel coordinates but good at naming what's on screen. When a question
is about the screen, the system prompt asks the model to mark targets as `[[point: label]]` (the label as it appears)
— or `[[point: x,y]]` in 0–1000 image coordinates, which Qwen-VL produces natively. `wisp-screen::guide` resolves
labels against Accessibility elements (exact frames, actionable elements preferred, smallest container wins) and
recognized text (narrowed to the matched words), and maps image coordinates into the capture. Targets are shown as
soon as their marker streams in; markers are stripped from stored text, the UI and speech. The host draws on a
click-through, non-activating, capture-excluded overlay per display and the buddy flies to the last target.
Guidance is persisted on the message so it can be replayed.

## D-028 — Computer control: typed tools, central policy, host execution, verification
**Status** accepted (2026-10-09)
Tools are a closed, typed set (`ToolCall`, adjacently tagged) — 19 from spec §55; **no shell tool** (spec §59 makes it
optional and it is the highest-risk capability; revisit only with a sandbox). Pipeline in the core: structural
validation (`wisp-computer::validate`: ranges, http(s)-only URLs, app names without paths, no control characters) →
risk (`assess`: tool default, raised by destructive/financial/irreversible target labels as whole words, risky
shortcuts, line breaks in typed text, insecure URLs) → policy (`wisp-permissions::evaluate`): off by default; Safe runs
without asking; Low/Medium ask unless granted (Allow once / session / always, or never), High/Critical always ask and
only offer "Allow once"; answers outside the offer are denials; unanswered requests deny after 2 minutes. The host
executes with Accessibility press when the labeled control is under the point, synthetic events otherwise, refuses
secure input, password fields and Wisp's own windows, and verifies by comparing frontmost app, window title, focus and
the element at the point before/after (verified / failed / unknown — never assumed). Every call is audited in
`tool_calls` with arguments redacted (typed text becomes a length); unfinished calls are marked failed at startup.
First integration: "Click “…” for me" on what an answer pointed at; the model-driven loop arrives with agents (Phase 13).

## D-029 — Teacher mode reuses the answer engine; progress is detected, never assumed
**Status** accepted (2026-10-09)
A lesson is a conversation: the goal is the user message and each step is an assistant message produced by the
same engine as answers (streaming, stages, guidance markers resolved against the screen, speech), with a teacher
system prompt that allows exactly one short instruction, `DONE:` or `STUCK:`. Each step observes the frontmost
non-Wisp window (`host.screen.observe`: capture + OCR + Accessibility, no selection UI). Progress is detected with a
cheap Accessibility fingerprint (app, window title, focused element, selection) polled every 1.5 s — no repeated
captures — or the user's "I did it". Lessons stop at 15 steps or after 15 minutes waiting; three identical
instructions mark the lesson stuck (loop detection, spec §61). Lessons never act: "Do it for me" goes through the
Phase 11 tool pipeline and its approvals. Intent detection starts a lesson for "teach me how to…", "show me how to…",
"walk/guide me through…", or the composer's Teach me mode.

## D-030 — Agents speak a tiny action language; every action goes through the tool pipeline
**Status** accepted (2026-10-09)
Local models are unreliable at JSON tool calling, so agents reply with `THOUGHT:` / `STEP:` / `ACTION:` lines in a small,
documented command language (`click "Save"`, `type "…"`, `hotkey cmd+s`, `done "…"`, `ask "…"`; full list in
`wisp-agents::ACTION_HELP`), parsed strictly with one correction retry. Targets are labels resolved against the
observed window (the Phase 10 resolver), or 0–1000 image coordinates for vision models. A run plans once (2–6 items,
shown as a checklist), then loops observe → decide → execute → observe. Every action is a `tool.run` — validation,
risk, policy, approval, verification, audit — and an agent may only use the tools its definition lists (spec §104:
no automatic new permissions). A denial ends the run (never routed around). Hard limits per agent: steps, minutes,
actions and an estimated token budget; the same action three times in a row pauses the run as stuck (spec §61);
users can pause, resume, continue, answer questions and stop. Runs and steps are persisted; interrupted runs are
marked cancelled at startup. The chat's "Do it" mode starts a one-off run with default limits.

## D-031 — Skills are versioned SKILL.md text, offered by relevance, never executed
**Status** accepted (2026-10-09)
A skill is a SKILL.md: front matter (`name`, `description`, `activation: auto|always|manual`, `triggers`, `tools`)
and a markdown body (instructions, workflows, reference, examples), at most 50 KB of text. The database is the
source of truth (every edit is a new version with its SHA-256; restore makes an old version current as a new one);
files are mirrored to `<data>/skills/<slug>/SKILL.md` and folders added there are picked up at startup. Activation:
enabled skills only — always-on, attached to the running agent, or `auto` with triggers/name matching the request;
at most two (plus attached) within a 6 000-character budget, added to the system prompt and named in the answer's
Context. Imported skills (file, paste or folder) start disabled (spec §103, §118); skills are never run as code.
A built-in "Mac keyboard shortcuts" skill is seeded as the sample.

## D-032 — Memory: explicit, visible, local; Apple sentence embeddings with calibrated hybrid ranking
**Status** accepted (2026-10-09)
Wisp remembers only what the user clearly states ("remember that…", "my name is…", "I prefer…", "call me…",
"from now on…", "my favorite…"), never questions, and each save is shown under the answer with Undo; "forget that…"
deletes matching memories. No hidden summarization of chats (spec §67: don't silently store unlimited information);
a hard cap of 500, temporary memories expire after a day and session ones at restart. Retrieval embeds the question
and memories with Apple's on-device NaturalLanguage sentence embeddings via the host (no download, nothing leaves
the Mac), cached per model in `memory_embeddings`, and ranks with an even blend of semantic similarity and keyword
overlap — calibrated on measured cosines (related ≈ 0.25–0.40, unrelated ≈ 0–0.29, so 0.20–0.45 maps to 0–1) — plus
boosts for profile facts and standing instructions; up to four above a threshold join the system prompt and are
listed in Context. Without embeddings ranking falls back to keywords. Agents can `remember "…"` (agent-scoped).

## D-033 — Files: copied in, read on device, chunked with locators, hybrid search, cited answers
**Status** accepted (2026-10-09)
Files enter only through user intent — the system open panel (`host.files.pick`) or drag-and-drop (≤ 20 MB through
IPC) — and are copied into `<data>/files/<id>-<sanitized name>`; the originals are never read in place. An extension
allow-list (text/Markdown/CSV/JSON/code, PDF, Word/RTF/HTML/ODT, images) gates import; everything else is refused;
50 MB cap; identical content (SHA-256) is stored once; all paths are confined to `files/` on both sides (core
`confine`, host prefix + symlink resolution). Text formats are decoded in the core (binary rejected); PDFs are read by
PDFKit page by page (citations name the page), documents by macOS's own readers, images by on-device Vision. Text is
chunked (~900 chars, 150 overlap, line/page locators), embedded with the same on-device sentence embeddings as
memory, and searched with the same calibrated blend of similarity and keywords. Up to three passages above a
threshold join the prompt in a fenced `<documents>` block (information, never instructions) and the answer carries
numbered citations that open the file. Generation still follows the privacy router, so Local Only never sends a
document anywhere. Unfinished indexing resumes when the host connects.

