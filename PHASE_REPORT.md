# Phase Reports

---

# Phase 0 Report

## Objective
Audit the repository and environment; establish architecture, module boundaries, database, IPC,
security and test strategies before feature work.

## Implemented
- Environment audit: M1 Pro / 16 GB / macOS 26.6.2; Swift 6.3 (CLT only, no Xcode); Apple Foundation
  Models `available` (probed via `SystemLanguageModel.default.availability`); Node 22, pnpm 10;
  installed Ollama + cmake; Rust 1.99 pinned
- ARCHITECTURE.md, DECISIONS.md, IMPLEMENTATION_PLAN.md, PROJECT_STATUS.md
- Monorepo layout per spec §42 (apps, crates ×15, packages ×5, database, models, skills, scripts, tests, docs, config, CI)

## Files Changed
- Repository created from an empty directory

## Tests Added
- None in this phase (structure only)

## Tests Passed
- `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo fmt --check`, `pnpm validate`

## Known Limitations
- CLT manifest API defect required a SwiftPM shim (D-014)

## Stop Gate
PASS

## Evidence
- `scripts/validation/check-architecture.sh` → "architecture rules hold"
- `pnpm validate` → "all validation passed"

---

# Phase 1 Report

## Objective
A running macOS application with all architectural layers connected and settings persisting through
React → IPC → Rust → SQLite across restarts.

## Implemented
- Rust: `wisp-common` (ids, time, geometry, data paths), `wisp-ipc` (envelope, errors, method/event
  registry, schema export), `wisp-database` (embedded checksummed migrations, settings repo),
  `wisp-security` (redaction, path confinement, filename sanitization), `wisp-core` (dispatch, event bus,
  stdio transport with host-call support, redacting file logs)
- Contracts generator (`pnpm contracts`) → Zod + Swift Codable; `--check` drift mode
- TS: `@wisp/contracts` typed/validated IPC client + WebKit transport; `@wisp/ui` tokens & components;
  `@wisp/icons`; `@wisp/test-utils` (mock transport, real-core spawner); `@wisp/web` shell and Settings
- Swift: `CoreProcess` supervisor, `IPCRouter`, `WebSurface` (origin-checked bridge, navigation lockdown),
  `WispSchemeHandler` (confined asset serving), main window, main menu, menu bar, app delegate
- Packaging: `scripts/build/build-macos.sh` assembles and signs `Wisp.app`; app icon generator

## Files Changed
- `crates/{common,ipc,database,security,core}`, `database/migrations/0001–0007`, `packages/*`,
  `apps/web/*`, `apps/macos/*`, `scripts/*`, `docs/*`

## Tests Added
- Rust (22): envelope round-trip/rejection, method registry uniqueness, settings defaults/validation,
  migrations (order, idempotency, tamper detection, spec tables, FKs), settings repo (round trip,
  reopen, corrupt-section fallback), redaction, path confinement incl. symlink escape, geometry,
  **stdio integration** (restart persistence, bad input resilience, events)
- TS (9): IPC client validation/errors/timeouts/events; Settings page with mock core; **Settings page
  with the real core across a core restart**
- Swift (12): envelope strictness, line framing, JSON integer precision, generated union codable,
  scheme-handler confinement, router rejects unknown methods, **CoreProcess against the real core:
  restart persistence, error surfacing, crash → automatic restart, missing binary**

## Tests Passed
- All (see `pnpm validate`, `pnpm e2e`)

## Known Limitations
- The UI-click → quit → relaunch path inside the real app window is covered by automated equivalents
  on each hop but awaits a manual run (docs/release/manual-checklist.md, Phase 1 section).

## Stop Gate
PASS

| Gate item | Evidence |
|---|---|
| Builds | `scripts/build/build-macos.sh --debug` → signed `build/debug/Wisp.app` |
| Launches | `open build/debug/Wisp.app` → `Wisp` and `wisp-core` processes running |
| Working IPC | core log in the launched app: `handled method="app.info"`, `"settings.get"` issued by React via WKWebView → Swift router → core |
| Persists settings | `settings_persist_across_restart` (Rust), `settings.core.test.tsx` (React↔core), `settingsPersistAcrossCoreRestart` (Swift↔core) |
| Runs migrations | launch log: migrations 1–7 applied; tamper test |
| No critical console errors | no error entries under `com.wisp.assistant`; clean core log |
| Basic automated tests | 43 tests across Rust, TS and Swift |

## Evidence
- `pnpm validate` → ✓ all validation passed
- `pnpm e2e` → Rust 3/3, React↔core 1/1, Swift↔core 5/5
- Quitting the host leaves no orphaned `wisp-core` (stdin EOF shutdown)

---

# Phase 2 Report

## Objective
Complete, configurable assistant activation: default ⌘⌥, shortcut recorder, hold/toggle/push-to-talk,
mouse and menu bar invocation, conflict detection, persistent configuration.

## Implemented
- Contracts: `InvocationSettings` (shortcut, behavior, mouse button/action, hold delay) as a settings
  section; `host.invocation.status`, `host.shortcut.record`, `host.shortcut.check`,
  `host.system.openSettingsPane`, `host.assistant.setVisible`; events `invocation.started|ended|statusChanged`
- Swift `WispPlatform`: `ChordStateMachine` (pure, tested), `EventTapMonitor` (listen-only, self-re-enabling),
  `CarbonHotKey` (press/release, detects shortcuts taken by other apps), `ShortcutConflicts`
  (reads `com.apple.symbolichotkeys` overrides + built-in defaults), `ShortcutRecorder`,
  `InvocationController` (channels, permission polling, Accessibility change notifications), `SystemSettings`
- Swift UI: `FloatingAssistantPanel` (non-activating, all Spaces/full screen, remembers position), menu bar
  "Summon Wisp" with shortcut hint
- Core: host invocation events validated against the contract and recorded in `events` (bounded)
- React: `/assistant` panel route, Invocation settings group (recorder, conflicts, permission guidance,
  behavior, mouse, trackpad limitation, menu bar), `invocationStore`, `assistantStore`
- Generator: handles schemars' mixed `enum`/`const` string unions

## Files Changed
- `crates/ipc/src/types/invocation.rs`, `crates/ipc/src/{methods.rs,types/settings.rs}`,
  `crates/database/src/repos/{events.rs,settings.rs}`, `crates/core/src/{dispatch.rs,transport.rs}`
- `apps/macos/Sources/Platform/**`, `apps/macos/Sources/UI/{FloatingAssistant,MenuBar,Web}/**`, `apps/macos/App/**`
- `apps/web/src/{features/assistant,features/settings,stores,lib}/**`, `packages/ui/src/styles/index.css`

## Tests Added
- Rust: invocation defaults and shortcut validation; event recording incl. rejection of malformed/unknown events (stdio)
- Swift: 8 chord state machine cases (hold delay, taint by app shortcut, cancel on key while active,
  early release, superset chords, toggle taps, long-hold/tainted taps, dismissal), 6 conflict/display cases
  (Spotlight default, disabled override, remapped override, common-app warning, clean defaults, ordering)
- React: invocation group — default display, permission guidance, save clean recording, refuse blocking
  conflict, quiet cancel; settings fixture updated

## Tests Passed
- `pnpm validate` → all passed (26 Rust, 15 TS, 26 Swift)

## Known Limitations
- Trackpad gestures: unsupported by public macOS APIs (D-017).
- Push to Talk is selectable only after the voice pipeline exists (Phase 8).
- Global input cannot be simulated in automated tests without granting this terminal Accessibility;
  the event tap path is verified manually.
- With ad-hoc signing, the Accessibility grant must be redone after every rebuild until the local
  signing identity is created.

## Stop Gate
PENDING — automated portion passes; manual checklist (Phase 2) must be run on the Mac.

## Evidence
- `pnpm validate` → ✓ all validation passed
- Launched debug app with no Accessibility grant: core requests served; keyboard channel reports
  `needsPermission` (no false "active" state)

---

# Phase 3 Report

## Objective
The distinctive interaction: an assistant cursor that lives beside the pointer, communicates state,
is themable, and opens the floating assistant.

## Implemented
- Contracts: `CursorSettings` section (enabled, visibility, theme/custom color, size, opacity, glow,
  animation, label), `AssistantState` (12 states), `assistant.stateChanged`, `host.assistant.state`,
  `host.cursor.preview`
- Swift: `BuddyMotion` (pure follow/edge/approach-hold model), `BuddyView` (Core Animation orb, glow,
  per-state rings, hover), `BuddyController` (per-frame placement across displays, adaptive 60/10 Hz,
  label pill via SwiftUI, Reduce Motion, screen-change handling, capture exclusion), panel
  expand-from/collapse-into buddy with on-screen placement, `AssistantCoordinator` owning buddy + panel + state
- React: Assistant cursor settings with live preview and on-screen preview, `Slider` component,
  panel shows live assistant state, fixtures derived from contract defaults
- Fix: `@source` for the design-system package so its Tailwind classes are generated

## Tests Added
- Swift: anchor placement, right/bottom edge flips, every-corner containment, negative-origin secondary
  display, convergence while following, approach-hold, fast sweep isn't a reach, Reduce Motion snap,
  panel placement on screen and not covering the buddy
- Rust: cursor settings defaults and validation
- React: defaults, color/visibility save, local + on-screen preview, label length

## Tests Passed
- `pnpm validate` → all passed (28 Rust, 19 TS, 34 Swift)

## Known Limitations
- Panel position always follows the buddy; a dragged panel isn't remembered across openings.
- States beyond idle/summoned are driven by later phases (voice, screen, AI); today they're reachable via preview.
- The buddy can't avoid covering UI under it until Accessibility element detection (Phase 5).

## Stop Gate
PENDING — automated portion passes; manual checklist (Phase 3) must be run on the Mac.

## Evidence
- `pnpm validate` → ✓ all validation passed
- Launched debug app: screenshot shows the buddy rendered 38 pt below-right of the pointer and following
  it live while the Mac was in normal use (captured with `WISP_BUDDY_CAPTURABLE=1`).

---

# Phase 4 Report

## Objective
Let the user tell the AI exactly what to look at, with trustworthy coordinates on any display.

## Implemented
- Contracts: `FocusSettings` section, `FocusRegion` (+ display, app, window, image), focus events
  (`started/completed/ended/cleared`), host methods (`focus.start/cancel/clear`, screen permission), core `focus.latest`
- Rust `wisp-screen`: region validation (inside display, crop ⊇ selection, plausible scale, image pixels =
  crop × scale ±2, URL shape, path presence); core keeps the latest valid region in memory, logs metadata only
- Swift: `FocusGeometry` (AppKit↔CG conversion, crop/margin, point region, local rects, pixel sizes),
  `Displays`, `ScreenPermission`, `ScreenCapturer` (SCScreenshotManager, self-exclusion, PNG save + prune),
  `WindowLocator`, `FocusOverlayView/Controller` (per-display panels, rect/lasso/circle/point, dimming,
  "Focused" label, teardown on every path), `FocusCoordinator` (hold flow, panel flow, permission, region
  assembly), `wisp://focus` scheme root (PNG-only)
- React: focus store, panel `FocusCard` + `FocusActions`, permission banner, Settings → Screen & focus

## Tests Added
- Rust: region validation accept/reject cases; stdio: valid region kept, inconsistent region rejected, clear, metadata-only log
- Swift: coordinate round-trips incl. negative-y displays, drag rects, margin clipping, point region shifting,
  local rect + Retina pixels, click vs drag, focus scheme confinement, **real ScreenCaptureKit capture of a
  known target (exact pixel size, center/edge/inset colors prove no offset)**
- React: panel starts each focus kind, card shows thumbnail/size/display/window/pixels, clear, permission guidance

## Tests Passed
- `pnpm validate` → all passed (31 Rust, 23 TS, 42 Swift)

## Known Limitations
- Secondary-display capture is covered by geometry tests; real-hardware verification needs a second display (manual).
- Window titles require Screen Recording permission (macOS policy); without it only the app name is known.
- Lasso and circle capture their bounding box (the exact shape is kept in `path` for later masking).

## Stop Gate
PENDING — automated portion passes, including real capture on the primary Retina display; manual checklist (Phase 4) remains.

## Evidence
- `scripts/development/swiftpm.sh test --filter ScreenCaptureIntegrationTests` → passed (not skipped) on the M1 Pro Retina display
- `pnpm validate` → ✓ all validation passed

---

# Phases 5–8 Report

## Objective
Understand real UI elements (5), connect local AI providers (6), deliver the first end-to-end AI experience (7),
and make it voice-driven (8).

## Implemented
- **5:** `AccessibilityReader` (AX under point, ancestors, focused element, selected text, region traversal
  bounded by nodes/depth/250 ms, off the main thread, secure values never read); `wisp-accessibility` ranking +
  compact summaries; focus regions carry AX context; Permissions page with live TCC status, capability list,
  inspector; microphone permission
- **6:** `wisp-ai` provider trait + pure privacy-safe router; providers: Ollama (status, capabilities via
  /api/show, streaming chat with images, pull/cancel/delete, alias filtering), OpenAI-compatible (locality by
  address, Keychain keys), Apple Foundation Models via host (streamed deltas, cancel), mock; host Keychain and
  Start Ollama; Models page; host-call timeouts
- **7:** conversation engine (system prompt for spoken answers, recent turns, focus JPEG + AX summary, streaming,
  progressive persistence, cancel, startup recovery, diagnostics), provider fallback with cooldown, conversations
  repo, panel chat, Conversations page, Home ask
- **8:** `wisp-audio` (energy VAD with adaptive floor, sentence splitter, speakable text, whisper.cpp STT on Metal,
  Kokoro TTS on ONNX Runtime with espeak-ng phonemes, verified model downloads with resume); core voice service
  (sessions, transcription, ask, sentence-streamed synthesis worker, stop speaking, model management, warm-up);
  host `AudioCapture` (16 kHz Int16, levels), `SpeechPlayer` (ordered, cache-confined, barge-in),
  `VoiceCoordinator` (hold = point + talk); Voice page; panel mic button

## Tests Added
- Rust: AX ranking (4), router privacy/fallback/thinking preference (8), Ollama/OpenAI/Apple parsing (7),
  conversations repo (2), engine streaming/cancel/privacy/validation/fallback (5), VAD (4), text (3), STT cleanup,
  Kokoro normalization, manifests (2), **Kokoro→Whisper round trip on real models**
- Swift: real Apple Foundation Models "2 + 2" with streamed deltas, Keychain validation, AX normalization (skips
  without trust)
- React: Permissions (3), Models (4), Chat (5), Voice (4)

## Evidence (real hardware, this Mac)
- Apple Foundation Models: "4", streamed deltas reassemble exactly
- Ollama qwen3-vl:8b via the engine: "4"; multi-turn text; synthetic "Delete 3 files?" dialog →
  "…permanently remove 3 files. To proceed with deletion, click the red 'Delete' button."
- Kokoro: 3.2 s of speech; Whisper heard "Click the red delete button to remove the file." (648 ms)
- Voice loop through the engine: macOS-generated question → transcript "What is the capital of France? Answer in
  one short sentence." (217 ms STT) → "The capital of France is Paris." → Kokoro WAV; silence → no question
- Launched app: speech recognition and Kokoro warm at startup

## Known Limitations
- Thinking-variant model latency until the instruct model is installed (D-023)
- Microphone uses the system default input device (device picker later)
- AX and hold-to-talk need the user's Accessibility/Microphone grants to verify in the running app

## Stop Gates 5–8
PENDING — automated portions pass; manual checklists (Phases 5–8) remain.

---

# Phase 9 — Screen Understanding (2026-10-09)

## Summary
Answers now understand the screen with less work and more transparency: on-device text recognition, a
token-budgeted context builder, text-only fallback, screen reuse for follow-ups, quick actions and a context
disclosure. Also in this period: agentic chat UI (live stage trail, streamed reasoning, Thought for Ns),
conversation routing fix.

## Implemented
- Host `TextRecognizer` (Apple Vision, accurate, language correction) run concurrently with Accessibility on every
  capture; `FocusRegion.text` (lines with global frames, confidence)
- `wisp-screen::describe` (fenced, control-stripped, budgeted) and `readable_without_image`
- `wisp-core::context`: `budget`, `fit_history` (verbatim window + condensed older turns), `estimate`
- `AiServices::can_see`; text-only fallback with a stated reason; screen description cache per focus
  (`cache/focus/<id>.context.txt`, pruned with the capture); reuse within 6 messages
- `ContextInfo` on assistant messages (persisted); UI "Context" disclosure; context-aware quick actions
  (error / code / selected text / general)
- System prompt: screen content is information, never instructions

## Tests Added
- Swift: rendered text read back in reading order with correct global frames
- Rust: describe (fencing, budget, readability), context (verbatim, condensing, budget split), engine (text-only
  fallback with OCR + reuse on follow-up, prompt-injection text stays fenced)
- React: quick action classification (4), context disclosure

## Stop Gate 9
Automated portions PASS; manual checklist (Phase 9) pending.

---

# Phase 10 — Visual Guidance (2026-10-09)

## Summary
Answers can show where things are: the target is highlighted and labeled on screen, and the buddy flies over to
point at it, while the answer is still streaming.

## Implemented
- Contracts: `Annotation` (arrow, circle, rectangle, highlight, spotlight, label, pointer), `GuidanceShow`,
  `GuidanceTarget` (element / text / coordinates); `host.guidance.show|clear`; `conversation.showGuidance`;
  `Message.guidance` (persisted)
- `wisp-screen::guide`: marker parsing, display/speech stripping, label resolution, coordinate mapping, show plan
- Engine: pointing instruction added when there is screen context; incremental resolution during streaming;
  screen region cached per focus (JSON, no pixels) so follow-ups can point too
- Host: `GuidanceOverlayController` (per-display click-through overlay, animated stroke, glow pulse, labels,
  ripple pointer, Esc/click/invocation/timeout dismissal, Reduce Motion), `BuddyController.point(at:)` flight
- UI: markers hidden while streaming; "Pointed at" chips replay guidance

## Tests Added
- Rust: marker parsing/stripping, label resolution (exact element over sentence, unknown → none), text narrowing,
  coordinate mapping (4); engine: answer points at Save → element frame, marker stripped, persisted; speech
  never reads markers
- Swift: engine JSON decodes and measures; all annotation kinds render (visually reviewed)
- React: marker stripping while streaming; chips replay guidance

## Stop Gate 10
Automated portions PASS; manual checklist (Phase 10) pending.

---

# Phase 11 — Computer Control (2026-10-09)

## Summary
Wisp can act on the Mac — click, type, press keys and shortcuts, scroll, drag, open/switch/quit apps, open links, use
the clipboard — only through validation, risk analysis, a central policy and the user's approval, with every action
verified and audited.

## Implemented
- Contracts: `ToolCall` (19 tools), `RiskLevel`, `ToolDefinition`, `ApprovalRequest/Response/Resolved`,
  `ToolExecution`, `Verification`, `ToolCallRecord`, grants; methods `tool.list|run|respond|activity|grants|revoke`,
  `host.input.perform`; events `tool.approvalRequested|approvalResolved|callUpdated`; `computer` settings section
- `wisp-computer` (definitions, validation, risk escalation, titles, redaction), `wisp-permissions` (policy)
- Database: tool audit + standing grants repository; startup recovery of unfinished calls
- Core `ToolService`: pipeline, approvals with timeout, session grants, read-only tools from Accessibility
- Host `InputController`: AX press / CGEvent input, typing with secure-input and password-field refusal, app control,
  clipboard, own-window refusal, before/after verification; panel opens for approvals
- UI: approval cards (panel + main window), "Click “…” for me" on guidance, Computer control settings with revocable
  permissions, Activity page

## Tests Added
- Rust: validation (URLs, apps, keys, text), risk escalation (whole words, critical phrases, shortcuts), titles and
  redaction (4); policy (4); audit + grants repo (2); engine pipeline with fake host — off by default, read-only
  allowed, approvals, session grants, High never covered by grants, forged choices denied, audit order, invalid calls
  never reach policy (3)
- Swift: key mapping, clipboard round trip (restores the user's clipboard), non-web URL refusal, observation
- React: approval choices and response, critical card, Do-it flow, Activity list (4)

## Stop Gate 11
Automated portions PASS; real input needs Accessibility for the app — manual checklist (Phase 11) pending.

---

# Phase 12 — Teacher Mode (2026-10-09)

## Summary
"Teach me how to …" starts a guided lesson: Wisp looks at the window you're working in, says and highlights one step,
waits while you do it, notices when you have, and continues until the goal is reached — without ever acting for you.

## Implemented
- Contracts: `Lesson`, `LessonStep`, `LessonStatus`, `LessonControl`; `teach.control|current`; `teach.updated`;
  `host.screen.observe`; `SendParams.teach`
- Core `teacher`: intent detection, lesson loop (observe → step via the answer engine → guidance + speech → wait →
  confirm), fingerprint watching, manual next/repeat/stop, step and time limits, loop detection, guidance cleared at
  the end; answer engine generalized (`lesson` mode, returns the final message)
- Host: `WindowLocator.frontmostWindow` (skips Wisp), `FocusCoordinator.observeFrontWindow`
- Contracts generator escapes all Swift keywords (`repeat`)
- UI: lesson card (checklist, status, watching hint, I did it / Show me again / Do it for me / Stop), composer
  Teach me mode, DONE/STUCK prefixes hidden in the chat

## Tests Added
- Rust: intent detection; lesson end to end with a fake host — first step resolves "Save" to its frame, screen change
  is noticed, DONE ends it, guidance shown, nothing performed, conversation holds goal/step/wrap-up; manual
  next and stop
- React: lesson checklist and controls, conversation scoping and closing, Teach mode send

## Stop Gate 12
Automated portions PASS; manual checklist (Phase 12) pending.

---

# Phase 13 — Agents (2026-10-09)

## Summary
Saved agents (and the chat's "Do it" mode) carry out multi-step tasks on the Mac: plan, observe, act through the
approval pipeline, verify, repeat — visibly, within hard limits, with pause/stop and loop detection.

## Implemented
- Contracts: `AgentDefinition`, `AgentLimits`, `AgentRun` (plan, steps, counters), `RunControl`; `agent.list|save|
  delete|run|control|runs`; `agent.runUpdated`; `SendParams.act`
- `wisp-agents`: action language parser, decision/plan parsing, label/coordinate resolution to tool calls, loop
  detector (pure)
- Database: agents (+ skills links), runs and steps; startup recovery
- Core runtime: validation of definitions, plan + act loop, tool scope per agent, limits, stuck/needs-input holds,
  denial stops, chat integration (answer = run summary)
- UI: Agents page (templates, editor with tools by risk and limits, run with a goal, recent runs), live run card
  (plan checklist, actions, controls, reply box) in the panel and conversations, composer "Do it" mode

## Tests Added
- Rust: action language (11 forms + rejects), decisions and plans, resolution to tool calls, loop detection (4);
  repository (2); runtime end to end — plan → click resolved to the Save button through approval → done, persisted
  history; repetition → stuck → stop; denial ends the run with nothing performed; validation; tool scope (3)
- React: template → run with goal → live card; plan/actions with pause/stop; stuck continue and question reply;
  Do it mode (4)

## Stop Gate 13
Automated portions PASS; manual checklist (Phase 13) pending.

---

# Phase 14 — Skills (2026-10-09)

## Summary
Reusable SKILL.md expertise that Wisp follows when relevant: create, edit (versioned), enable/disable, delete,
import (off by default), export; mirrored to disk; attachable to agents.

## Implemented
- `wisp-agents::skills`: SKILL.md parser (front matter, both list styles, strict limits and reasons), renderer,
  relevance, sample skill
- Contracts: `SkillSummary`, `Skill`, versions, export; `skill.list|get|save|setEnabled|delete|import|export|versions|
  restore`; `ContextInfo.skills`
- Database: skills + versions (SHA-256, dedupe unchanged edits, unique slugs)
- Core: startup seeding and folder scan (no symlinks, size-checked), disk mirror with safe slugs, activation into
  answers, lessons and agent runs
- UI: Skills page (list with source/activation/triggers/version, toggle, editor with version restore, import from
  file, export to clipboard), agent editor attaches skills, Context names skills used; MockTransport passes engine
  errors through

## Tests Added
- Rust: parser (sample, list styles, 6 invalid files with reasons, size), relevance (auto/always/manual), render
  round trip (4); repository (versions, slugs, toggles) (1); stop gate end to end — sample loads and is mirrored,
  instructions reach the model only when relevant, disabled skills unused, edit/restore versions, import disabled,
  invalid import explained, export name, survives restart, disk folder found disabled, no duplicates (1)
- React: list + toggle, edit with save error, Context names skills (3)

## Stop Gate 14
PASS on automated evidence (skill loads, affects behavior, can be disabled, edited, survives restart).

---

# Phase 15 — Memory (2026-10-09)

## Summary
Controlled persistent memory: explicit statements are saved visibly (with Undo), recalled when relevant using
on-device embeddings, and fully inspectable, editable and deletable.

## Implemented
- Contracts: `MemoryItem` (category, level, source, scope, usage), `memory.list|save|delete|clear`, `host.embed`,
  `MemorySettings`, `Message.savedMemories`, `ContextInfo.memories`
- `wisp-memory`: extraction (with third-person rewriting), forget requests, lexical + cosine ranking calibrated on
  measured Apple embeddings, vector encoding
- Database: memories (scoped links checked), embeddings per model, prune expired/session, usage tracking
- Core: memory service (dedupe, cap, embedding cache, recall with agent scope), extraction in `send`, recall into
  answers/lessons and agent runs, agent `remember` action
- Host: `SentenceEmbedder` (NLEmbedding sentence, English)
- UI: Memory page (grouped, search, add/edit/delete, Forget everything with confirmation, settings), "Remembered … Undo"
  under answers, Context count

## Tests Added
- Rust: extraction (8 cases + non-statements), forget, ranking, calibration on 6 measured pairs, vectors (5);
  repository (1); stop gate end to end with and without embeddings — preference saved visibly, related question
  recalls it, unrelated doesn't, forget removes it; manual add/edit/temporary/validation/filter/delete, learning
  off, memory off, clear (2); agent `remember` parsing
- Swift: on-device embeddings rank a related sentence above an unrelated one
- React: grouped list + forget, add + Forget everything confirmation, Remembered/Undo chip (3)

## Evidence
Measured on this Mac (Apple NL sentence embeddings): coffee preference ↔ "Which coffee should I buy?" 0.394;
dentist ↔ coffee 0.290; editor preference ↔ "Book a flight" 0.017 — used to calibrate ranking.

## Stop Gate 15
PASS on automated evidence (stated preference → memory → later conversation retrieves it; inspect; delete).

---

# Phase 16 — Files & Semantic Search (2026-10-09)

## Summary
Local knowledge: import documents (picker or drop), read them on this Mac, search them semantically, and get answers
grounded in them with citations — never uploaded in Local Only.

## Implemented
- `wisp-files`: supported kinds, name sanitizing, text decoding, chunking with locators, citations, lexical score
- Contracts: `FileItem`, search hits, `Citation`, staged files, extracted text; `file.list|import|importData|delete|
  search|open`, `host.files.pick|extract|open`, `file.updated`; `Message.citations`, `ContextInfo.files`
- Database: files + chunks (vectors), search rows, resume list, cascade delete
- Core: import (dedupe, confinement, caps), indexing (text here, rich formats via host, embeddings in batches),
  hybrid ranking, grounding into answers with citations, open/delete, resume on host attach
- Host: `FileReader` (NSOpenPanel copy-in, PDFKit pages, document readers, Vision OCR for images, open; strict path
  resolution)
- UI: Files page (import, drop zone, status, search with citations, open, remove), Sources chips under answers,
  Context lines

## Tests Added
- Rust: kinds, sanitizing (traversal, hidden, shell characters, length), decoding/binary, chunking (overlap,
  coverage, page locators, long lines), lexical (5); repository (1); stop gate end to end — correct document,
  relevant passage, grounded prompt with citation, unrelated questions ungrounded, dedupe, refusal, delete from disk
  and search; Local Only with a cloud-only model never calls it (2)
- Swift: real PDF pages and RTF read; paths outside files/ refused; names sanitized (2)
- React: list/search/open, picker and drop import, Sources chips (3)

## Stop Gate 16
PASS on automated evidence (correct document, relevant chunk, grounded answer, Local Only stays local).

---

# Phase 17 — Routines (2026-10-09)

## Summary
Scheduled automation: prompts or agents on a cron schedule in the user's timezone, with retries, notifications and
run history.

## Implemented
- `wisp-agents::schedule`: cron parser (lists, ranges, steps, Sunday 0/7, classic day-field OR), next-run search on
  the local wall clock with DST handling, plain descriptions
- Contracts: `Routine`, retry/notify policies, `RoutineRun`; `routine.list|save|delete|runNow|runs`, `host.notify`,
  `routine.runUpdated`; migration 0008 (run → conversation)
- Database: routines (agent links checked), due query, runs + history, startup recovery
- Core: validation with explanations, scheduler (tick, reschedule-before-run, catch-up once), prompt routines into
  "Routine: …" conversations, agent routines with limits, retries with backoff, notifications
- Host: `Notifier` (UserNotifications, asks once)
- UI: Routines page (presets + cron, timezone default, run as question or agent, tries, notify options, enable switch,
  Run now, history with links to results)

## Tests Added
- Rust: every 5 minutes, weekdays at 9 in Asia/Kathmandu, DST night is 25 h, month/day fields, invalid expressions (5);
  repository (1); stop gate — every-5-minutes routine triggers at its time, records the run, answer in its
  conversation, notification, reschedules forward, never fires twice; retries then one failure notification;
  schedule/timezone errors explained (2)
- React: list/history/run now, create from preset in local timezone (2)

## Stop Gate 17
PASS on automated evidence (scheduler triggers, run recorded).

---

# Phase 18 — Security Hardening (2026-10-09)

## Summary
Full read-only audit (17 findings: 3 high, 7 medium, 7 low); all high/medium fixed with tests, lows fixed except
core-binary signature checking (Phase 21). Write-up: docs/security/hardening-2026-10.md.

## Fixed
- HIGH: UI could call any host method (input, Keychain, screen, files) → explicit UI allow-list in the router
- HIGH: release builds honored env overrides for core binary / dev URL / data dir → debug-only
- HIGH: click risk judged on model-supplied label → judged on the real Accessibility element at the point;
  unknown targets Medium
- MEDIUM: newline escalation bypass (Return/paste) → High; content-driven tools never "always" grantable
- MEDIUM: approvals not bound to target → frontmost-app / target-pid binding; per-chunk secure-input checks
- MEDIUM: approval text truncated silently → full text or explicit "+N more characters"
- MEDIUM: loopback look-alikes classed Local → strict host parsing (`wisp_common::net`)
- MEDIUM: cloud endpoints probed in Local Only → not contacted
- MEDIUM: fence breakout in screen/documents → `defang`; memories fenced
- MEDIUM: agent `remember` persisted injected text as user memory → one-off runs temporary, labeled as agent notes
- LOW: HTML import via WebKit (network) → text extraction in Rust; ⌥⌘⌫ Critical; bounded frame reads, UI cap, drop
  cap; dev CSP stripped in production; link destinations shown; download size cap; skill symlinks refused
- Functional: host handled core requests serially → concurrent (fixed screen-observation deadlock); host timeouts
  per action (file picker no longer times out)

## Tests Added
- Rust: strict loopback parsing (13 cases), never-always tools, Return/paste/⌥⌘⌫ risk, unknown-target clicks,
  real-element risk end to end ("Account" over "Delete Account" → Critical, true label shown, app named), approval
  binding passed to the host, fence neutralization, HTML to text without scripts
- Swift: UI cannot call `host.input.perform` / `host.keychain.get`; allowed host methods still work

## Stop Gate 18
PASS on automated evidence; manual checks listed (Phase 18).

---

# Phase 19 — Performance (2026-10-09)

## Measured (this Mac, M1 Pro; release build)
| Measurement | Before | After |
|---|---|---|
| Engine startup (fresh data folder: migrations + seed) | 6.6 ms | 6.6 ms |
| IPC dispatch round trip (median) | < 0.01 ms | < 0.01 ms |
| `settings.get` / `memory.list` (500 memories) | 0.02 / 0.25 ms | same |
| Import + index 40 documents (10 720 passages) | 0.55 s | 0.55 s |
| **File search over 10 720 passages** | **139 ms** | **9.4 ms** |
| `conversation.list` with 1 000 conversations | 0.27 ms | same |
| Main UI bundle | 664 KB | **378 KB** (pages 4–24 KB, loaded on first visit) |
| Engine memory with speech models warm / after 15 min idle | 489 MB | 489 MB / models released |
| App (host) memory | 100 MB | — |
| First token, Apple on-device (10 answers, from diagnostics) | 8.0 s avg (1.8 s best) | routed away from when slow |
| First token, Ollama qwen3-vl:8b-instruct (warm) | ~0.25 s | preferred |
| Whisper load (cached) / speech recognition | 191 ms / 217 ms | same |

## Implemented
- Perf harness writing `target/perf.json`
- In-memory search index; question-embedding cache
- Lazy-loaded secondary pages
- Speech models: re-warm on hold, release after idle
- Latency-aware routing (per-model first-token EWMA, seeded from diagnostics, slow > 4 s)

## Tests Added
- Router: measured slowness moves answers to a faster local model (still local)
- Idle unload no-op; perf harness (manual)

## Stop Gate 19
PASS on automated evidence; app-level timings above (manual feel checks in the checklist).

