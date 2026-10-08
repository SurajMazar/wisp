# Manual verification checklist

Automated tests can't grant TCC permissions, plug in a second display, or listen to a speaker.
Every item here is verified by a person on real hardware before its phase gate passes.
Record the date, macOS version, and hardware next to each run in PHASE_REPORT.md.

## Phase 1 — Foundation
- [ ] `scripts/build/build-macos.sh --debug && open build/debug/Wisp.app` launches; Dock icon shows the Wisp mark.
- [ ] Main window opens on the Home screen; sidebar lists all sections; footer reads "Engine running".
- [ ] Settings → Appearance → Theme → Dark switches the UI immediately.
- [ ] Settings → Appearance → choose an accent color; switches and toggles pick it up.
- [ ] Settings → Privacy → Local Preferred; the explanation below updates.
- [ ] Quit Wisp (⌘Q), relaunch: theme, accent and privacy mode are unchanged.
- [ ] Menu bar item is present; "Open Wisp" and "Settings…" work; "Engine: running" is shown.
- [ ] Settings → General → turn off "Show menu bar icon": the item disappears; turn on: it returns.
- [ ] Close the main window: Wisp keeps running in the menu bar; clicking the Dock icon reopens it.
- [ ] `pkill -9 wisp-core` while Wisp is open: menu shows "restarting", then "running" again; settings still load.
- [ ] ⌘C / ⌘V / ⌘A work in any text field (future phases add fields; skip if none).

## Phase 2 — Invocation
Prerequisite: run `scripts/development/create-signing-identity.sh` once, then rebuild, so the Accessibility grant survives rebuilds.

- [ ] Settings → Invocation shows **⌥⌘** with "Needs permission" and an **Open System Settings** button.
- [ ] The button opens Privacy & Security → Accessibility. Enable Wisp. Within ~2 s the status turns **Active** (no relaunch).
- [ ] **Hold** (default): in another app (e.g. Safari), hold ⌘⌥ ~½ s → the floating panel appears *without* taking focus (keep typing works after release). Release → it disappears.
- [ ] Tap ⌘⌥ quickly in hold mode → nothing appears.
- [ ] Press ⌘⌥I in Safari (Web Inspector) → Wisp does **not** appear and Safari's shortcut works.
- [ ] Hold ⌘⌥⇧ → nothing appears.
- [ ] Switch Behavior to **Toggle**: tap ⌘⌥ → panel appears and takes focus; tap again → closes. Esc closes it too.
- [ ] Menu bar → **Summon Wisp** opens the panel; again closes it. The item shows the current shortcut.
- [ ] **Record New Shortcut** → press ⌥Space → saved; ⌥Space now summons Wisp (works even without Accessibility). Restart Wisp → still ⌥Space.
- [ ] Record ⌘Space → "Conflicts with macOS: Spotlight", not saved.
- [ ] Record ⌘C → saved with a warning ("Also used for Copy (most apps)"). Re-record ⌥⌘ afterwards.
- [ ] Esc during recording cancels without changes; waiting 10 s shows "No shortcut was pressed."
- [ ] Mouse button → Back side button (if your mouse has one) → pressing it toggles the panel.
- [ ] Trackpad row reads "Not supported by macOS".
- [ ] Panel appears on the display containing the pointer; can be dragged; position is remembered next time.
- [ ] Panel appears over a full-screen app and on every Space.

## Later phases
Added as each phase lands (invocation, cursor, multi-monitor, Retina, microphone, Whisper, Kokoro, Ollama, Apple Foundation Models, Accessibility, Screen Recording).
