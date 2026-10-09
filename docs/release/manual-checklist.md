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

## Phase 3 — Assistant cursor ("buddy")
- [ ] Launch Wisp: a small glowing buddy sits just below-right of the pointer. The system cursor is unchanged.
- [ ] Move slowly, then quickly: it follows smoothly and stays close; it never lags far behind.
- [ ] Move to each screen edge and corner: the buddy flips to the inside and never leaves the screen.
- [ ] With two displays: move across — the buddy jumps with the pointer and stays on its display. Repeat with different resolutions/scales.
- [ ] Reach for the buddy slowly: it holds still; hovering grows it and shows "Open Wisp".
- [ ] Click it: the assistant panel grows out of the buddy, next to it, fully on screen. Esc, Close, or clicking another app collapses it back into the buddy.
- [ ] Hold ⌥⌘ (Accessibility granted): the buddy brightens with a ring ("summoned"); release → back to resting. No window opens.
- [ ] Toggle behavior: tap ⌥⌘ → panel expands from the buddy; tap again → collapses.
- [ ] Menu bar → Summon Wisp: expands; again: collapses.
- [ ] Settings → Assistant cursor: change color, size, opacity, glow — the on-screen buddy updates immediately, and the preview matches.
- [ ] "Preview on screen" plays Summoned → Listening → Thinking → Speaking → Working → Waiting → Done on the real buddy, each with a label pill and distinct ring.
- [ ] Visible: "When summoned" → buddy disappears; holding ⌥⌘ shows it. Switch back to Always.
- [ ] Turn on System Settings → Accessibility → Display → Reduce motion: the buddy snaps instead of gliding and rings stop animating.
- [ ] Turn on "Show label" with a custom name: the pill appears beside the buddy.
- [ ] Screenshot (⌘⇧4) near the buddy: the buddy is not in the image.
- [ ] Activity Monitor: Wisp CPU stays low (≈0–1%) while the pointer rests.

## Phase 4 — Screen capture & focus
Prerequisite: Screen Recording for Wisp (Settings → Screen & focus → Allow…, or the panel's "Allow Screen Access").

- [ ] First focus without permission: the panel explains and "Allow Screen Access" opens the right System Settings pane.
- [ ] Hold ⌥⌘, drag a box around something on the **primary** display, release the mouse: the rest of the screen dims while dragging, a "Focused" label flashes, and the panel opens showing a thumbnail of exactly that area (plus a little margin).
- [ ] Same on a **secondary** display (if available), including one positioned left of / above the primary.
- [ ] Thumbnail pixel size equals the point size × 2 on Retina (shown under the thumbnail).
- [ ] Hold ⌥⌘ and release without dragging: no capture, overlay disappears immediately.
- [ ] Hold ⌥⌘ and click once: a point focus (square area around the click).
- [ ] Settings → Screen & focus → Lasso: draw a loop; Circle: drag an ellipse. Both capture their bounding area.
- [ ] Panel → Window: captures the window under the pointer (not Wisp's own). Panel → Screen: whole display.
- [ ] Esc during selection cancels; no dimming or outline remains anywhere, on any display.
- [ ] Unplug/rearrange a display during selection: the overlay disappears.
- [ ] The captured image never contains the buddy, the overlay, or the panel.
- [ ] Clear removes the card; the PNG is gone from `~/Library/Application Support/Wisp/cache/focus`.
- [ ] No screen-recording indicator stays on: capture happens once per selection, never continuously.

## Phase 5 — Accessibility
- [ ] Permissions → Accessibility → Allow: System Settings opens on the right pane; after enabling, the status turns Enabled without restarting.
- [ ] Inspector → Inspect Under Pointer, then hover a button in **Safari**, **Finder**, **VS Code** (or another editor) and **System Settings**: the right app, window, element role and title appear with plausible bounds.
- [ ] Draw a focus box around a form in Safari: the focus card shows "Element: … · N controls understood".
- [ ] Hover a password field: its value never appears anywhere (inspector, logs, answers).
- [ ] Revoke Accessibility: chat, voice and screen focus still work; the capability list marks control features unavailable.

## Phase 6 — AI providers
- [ ] Models page: Apple Foundation Model and Ollama show their real status (stop Ollama → "Not running" + Start Ollama works).
- [ ] Test on Apple's model and on an Ollama model both answer "4" with timings.
- [ ] Download a model with progress; Pause, then Resume continues; Delete removes it.
- [ ] Settings → Privacy → Local Only, add a cloud endpoint: questions never go to it ("Who answers" stays local).
- [ ] Pull the network cable / turn off Wi-Fi: local models still answer.

## Phase 7 — Ask and answer
- [ ] Click the buddy, type a question: the answer streams in with "model · on this Mac · N s".
- [ ] Stop during an answer keeps the partial text and says "Stopped".
- [ ] Quit Ollama mid-answer: a friendly error with Try Again and technical details; Wisp doesn't crash.
- [ ] Draw around an error message, ask "What's wrong here?": the answer is about that error.
- [ ] Ten conversations in a row without a crash; quit and relaunch: all are in Conversations, searchable, pinnable, renamable, deletable.

## Phase 8 — Voice
Prerequisite: Microphone permission; `brew install espeak-ng` (already installed on the dev Mac).
- [ ] Voice → Test Microphone shows what you said.
- [ ] Voice → Test Voice speaks with the chosen Kokoro voice; Speed and Volume change it.
- [ ] Hold ⌥⌘ and ask "What am I looking at?" while drawing around something: the buddy shows Listening, then Thinking; the panel opens with your words; the answer streams and is spoken.
- [ ] Start holding ⌥⌘ again while it's speaking: speech stops immediately.
- [ ] Hold ⌥⌘ and say nothing: nothing is asked.
- [ ] Local Only on, Wi-Fi off: the whole voice loop still works.
- [ ] No microphone indicator in the menu bar except while holding the shortcut.

## Phase 9 — Screen understanding
- [ ] Select an area containing text: the composer shows quick actions that fit it (an error → Explain error / Fix error; code → Explain code / Find bugs; selected text → Explain / Summarize / Rewrite / Translate).
- [ ] Ask about it; under the answer, Context lists the screenshot size, lines of screen text, UI elements.
- [ ] Pick a text-only model (e.g. Apple on-device) in the composer and ask about a selection: it answers from the on-screen text; Context says why no screenshot was sent.
- [ ] Ask a follow-up without selecting again: Context says the earlier screen was reused.
- [ ] Select a web page that says "ignore previous instructions…": Wisp explains it rather than obeying it.

## Phase 10 — Visual guidance
- [ ] Select a window with buttons and ask "Where do I click to save?": while the answer streams, the button gets a glowing outline with its name, and the buddy flies over to point at it.
- [ ] The highlight goes away on Esc, any click, holding the shortcut again, or after ~10 s; the buddy returns to the pointer.
- [ ] The answer text never shows "[[point: …]]" and it is never read aloud.
- [ ] Under the answer, "Pointed at: Save" — clicking it shows the highlight again.
- [ ] On two displays, the highlight appears on the display where the target is.
- [ ] The overlay never steals focus and clicks pass straight through it; Reduce Motion turns off the animations.

## Phase 11 — Computer control
Prerequisite: Accessibility permission for Wisp.
- [ ] Settings → Computer control is off by default; with it off, "Click “Save” for me" says it's off and nothing happens.
- [ ] Turn it on. Ask where to save in TextEdit; click "Click “Save” for me": an approval card appears in the panel (Allow once / for this session / Always / Deny) with a 2-minute countdown.
- [ ] Allow once: the Save dialog opens; the result says what changed; Activity lists it with "Done".
- [ ] Allow for this session, then repeat: no card the second time. Settings lists the session permission; Revoke removes it.
- [ ] Point at a "Delete" or "Move to Trash" button and ask Wisp to click it: the card is marked High risk, explains why, and only offers Allow once / Deny — even if click is always allowed.
- [ ] Let a card expire: the action is denied and Activity says so.
- [ ] Ask Wisp to type into a password field: it refuses.
- [ ] Turn on "Ask before every action": every action asks, whatever was allowed before.

## Phase 12 — Teacher mode
Prerequisite: Screen Recording (to watch), Accessibility (to notice steps by itself — optional).
- [ ] In TextEdit, ask "Teach me how to make this text bold" (or turn on Teach me in the composer and type the goal).
- [ ] A lesson card appears: Wisp looks at the window, says the first step aloud, highlights the control and the buddy points at it.
- [ ] Do the step yourself: within a couple of seconds the step gets a ✓ ("Wisp saw the screen change") and the next step appears.
- [ ] "Show me again" re-highlights and repeats the step; "I did it" advances; "Stop lesson" ends it and clears the highlight.
- [ ] "Do it for me" asks for approval through Computer control (and is refused if it's off).
- [ ] When the goal is reached, the card says Done with a short note. Wisp never clicks or types by itself during a lesson.
- [ ] Without Accessibility, the card says to press "I did it" after each step.

## Phase 13 — Agents
Prerequisite: Computer control on (Settings), Accessibility, Screen Recording.
- [ ] Agents → start from the Note Taker template; Run with "Make a note called Groceries with milk and eggs".
- [ ] The run card shows the plan (○ / ● / ✓) and each action live; every risky action asks in an approval card first.
- [ ] Pause stops it between steps; Resume continues; Stop ends it and clears any highlight.
- [ ] Deny an action: the run stops and says why — it never tries another way around.
- [ ] Give an agent only read-only tools and ask it to click something: it stops and says it needs permission for “click”.
- [ ] In the chat, turn on "Do it" and ask for a small task: the answer is the run's summary when it's done.
- [ ] Watch for loops: an agent repeating itself pauses with "The assistant may be stuck…" and Continue / Stop.
- [ ] Activity lists every action an agent took.

## Phase 14 — Skills
- [ ] Skills lists "Mac keyboard shortcuts" (Built in, on). Ask "is there a shortcut to copy?": the answer uses it and Context lists the skill.
- [ ] Turn it off and ask again: the skill isn't used.
- [ ] Edit it (add a line), save: version 2; restore v1 from the editor.
- [ ] Import a SKILL.md file: it appears as Imported and off; it's only used after you turn it on.
- [ ] Export copies the SKILL.md to the clipboard.
- [ ] Quit and reopen Wisp: skills and their on/off state are unchanged.
- [ ] Drop a folder with a SKILL.md into ~/Library/Application Support/Wisp/skills and reopen: it appears, off.
- [ ] Attach a skill to an agent: its runs list "Using skills: …".

## Later phases
Added as each phase lands (invocation, cursor, multi-monitor, Retina, microphone, Whisper, Kokoro, Ollama, Apple Foundation Models, Accessibility, Screen Recording).
