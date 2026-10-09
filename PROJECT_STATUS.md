# Project Status

## Current Phase
Phase 16 — Files & Semantic Search (in progress). Phases 9–15 implemented; **manual stop gates 2–13 pending on the Mac**

## Completed
- **Phase 0 — Repository audit & architecture** (gate PASS, 2026-10-08)
  - Environment audited; architecture, decisions (D-001…D-016), plan documented
  - Monorepo per spec §42: pnpm + Cargo workspaces, SwiftPM host, contracts, docs, manifests, CI
- **Phase 1 — Application foundation** (gate PASS on automated evidence, 2026-10-08)
  - Swift host: AppKit lifecycle, main window (WKWebView, `wisp://` scheme), main menu, menu bar item
  - `wisp-core` Rust runtime supervised as a child process (restart with backoff on crash)
  - Typed IPC on all hops; contracts generated from Rust into Zod + Swift Codable, drift-checked
  - SQLite with 7 checksummed migrations covering every table in spec §8
  - Settings (general, appearance, privacy, onboarding) persisted and pushed as events
  - React 19 shell: sidebar, Home, Settings (theme, accent, privacy mode), honest "not available yet" states
  - Design tokens with independent light/dark palettes; original app icon and menu bar mark
  - Redacting logger; path confinement; secret scanning; architecture rule checks

## In Progress
- **15 Memory:** explicit and visible (Undo), on-device embeddings, calibrated hybrid recall, Memory page (gate PASS)
- **14 Skills:** versioned SKILL.md, used when relevant (never executed), import off by default, disk mirror,
  attachable to agents (gate PASS)
- **13 Agents:** saved agents + chat "Do it": plan → observe → one action via the approval pipeline → verify;
  limits, loop detection, pause/stop, live run card, Agents page
- **12 Teacher mode:** "teach me how to…" → observe, one highlighted and spoken step at a time, notice when the user
  did it, continue; never acts by itself
- **11 Computer control:** 19 typed tools, risk escalation, central policy (off by default, approvals with
  once/session/always/deny, high-risk always asks), host execution with verification, full audit, Activity page
- **10 Visual guidance:** answers mark targets, resolved to exact frames; on-screen highlight + label overlay; the
  buddy flies to point; replay from the chat
- **9 Screen understanding:** on-device OCR, budgeted context builder with condensed history, text-only fallback,
  screen reuse for follow-ups, quick actions, "Context" disclosure; screen text fenced as untrusted
- Phases 5–8 implemented and automatically verified (see PHASE_REPORT.md):
  - **5 Accessibility:** element under pointer, ancestors, focused element, region elements (bounded, off-main-thread,
    secure values never read), Rust relevance ranking, Permissions page + inspector
  - **6 AI providers:** Ollama + Apple Foundation Models + OpenAI-compatible behind one interface, privacy-safe router,
    Models page with downloads, tests, preferences; Keychain for keys. **Both Apple and Ollama answered "What is 2 + 2?"**
  - **7 First AI slice:** conversations with streaming, cancel, persistence, history, fallback; panel chat;
    **local qwen3-vl correctly described a dialog from a focused screenshot**
  - **8 Voice:** push-to-talk capture, VAD, whisper.cpp (Metal), Kokoro (ONNX + espeak-ng), sentence-streamed speech,
    barge-in; **end-to-end through the engine: spoken question → exact transcript → local answer → spoken reply**
- Phase 4 implemented and automatically tested; manual checklist pending (Phase 4 section):
  - Hold ⌥⌘ + drag (rectangle / lasso / circle) or click a point; panel buttons for area, point, window, screen
  - Per-display overlay: dims surroundings, "Focused" confirmation, guaranteed teardown, excluded from capture
  - ScreenCaptureKit one-shot capture at native scale, Wisp excluded; margin + clipping; PNG cache pruned to 12
  - FocusRegion with display, selection, crop, path, cursor, app, window; validated in Rust, kept in memory only
  - Panel focus card with thumbnail via `wisp://focus/…`; Screen Recording permission guidance
  - **Verified automatically on real hardware:** capture of a known on-screen target is exactly crop × scale pixels with no offset
- Phase 3 implemented and automatically tested; manual checklist pending (docs/release/manual-checklist.md → Phase 3):
  - Buddy beside the pointer on every display (spring follow, edge flipping, approach-hold so it can be clicked)
  - Click the buddy → assistant panel expands out of it; Esc / Close / click-away collapses it (D-019)
  - 12 assistant states with distinct ring treatments and a state label pill; `assistant.stateChanged` events
  - Themes (9 colors + custom), size, opacity, glow, animation level, label; Reduce Motion respected
  - Settings → Assistant cursor with live preview and "Preview on screen" on the real buddy
  - Hold shortcut → buddy becomes attentive; toggle / menu bar → panel expands
  - Buddy excluded from screen capture; 10 Hz idle polling
  - Fixed: Tailwind wasn't generating design-system classes (segmented controls/switch looked broken)
- Phase 2 implemented and automatically tested; manual gate checklist pending (docs/release/manual-checklist.md → Phase 2):
  - ⌥⌘ modifier-only chord via listen-only event tap with taint rule (⌘⌥I etc. never summon Wisp)
  - Key chords (e.g. ⌥Space) via Carbon hot keys — work without any permission
  - Hold / Toggle behaviors; Push to Talk stored but disabled until voice exists
  - Native shortcut recorder; conflict detection against live macOS symbolic hot keys, fixed system
    shortcuts, common app shortcuts and Wisp's own bindings
  - Mouse middle/back/forward button invocation; trackpad honestly reported unsupported (D-017)
  - Menu bar "Summon Wisp"; floating non-activating assistant panel shell (D-018)
  - Live channel status with permission guidance; auto-detects the Accessibility grant
  - invocation.started/ended recorded in the events table (metadata only)

## Blocked
- Nothing blocking. Environment caveats:
  - Command Line Tools manifest API is stale on this Mac → worked around by `scripts/development/swiftpm.sh` (D-014). Reinstalling the CLT or installing Xcode fixes it properly.
  - No code-signing identity yet → TCC permissions reset on every rebuild until `scripts/development/create-signing-identity.sh` is run (needed from Phase 2 onward).

## Failed Tests
- None. `pnpm validate` passes (Rust, 33 TS, 46 Swift tests incl. real Apple model, ScreenCaptureKit and Kokoro→Whisper round trip).
- Environment-dependent tests report skips rather than passing silently: AX test (runner lacks Accessibility);
  Apple generation when macOS refuses non-foreground callers (ModelManagerError 1013).

## Known Issues
- `qwen3-vl:8b` (thinking variant) takes 20–70 s to answer; `qwen3-vl:8b-instruct` is downloading and the router will
  prefer it automatically (D-023).
- Phase 1 manual UI checklist (docs/release/manual-checklist.md) not yet run by a person; launch and React→Swift→Rust IPC were verified from logs of a real app launch.
- Web bundle is 484 KB (unsplit); revisit in Phase 19.
- Model manifests are empty until Phase 8 (entries require verified checksums).

## Next Stop Gate
- **Gate 4 (manual):** primary + secondary display focus, Retina crop, no shift, overlay removal, no continuous capture. Checklist: Phase 4 section.
- **Gate 3 (manual):** slow/fast movement, screen edges, two displays, resolution change, theme change, Reduce Motion, click-to-expand. Checklist: docs/release/manual-checklist.md (Phase 3).
- **Gate 2 (manual, needs a person):** ⌘⌥ works globally; shortcut change survives restart; hold and toggle modes; menu bar invocation; conflicting shortcuts handled. Checklist: docs/release/manual-checklist.md (Phase 1 + Phase 2 sections).

## Last Verified
2026-10-09
