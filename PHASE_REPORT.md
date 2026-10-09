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
