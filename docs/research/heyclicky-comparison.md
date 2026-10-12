# HeyClicky vs Wisp — feature comparison and plan

_Researched 2026-10-12 from public sources (listed at the end) and the MIT-licensed open-source Clicky code.
HeyClicky's current app is closed source, so its behaviour is described from its site, the founder's posts and
third-party write-ups; treat details marked "reported" as unverified. Per the spec we copy capabilities, never
branding._

## Summary

HeyClicky's core loop is: **hold ⌃⌥ → it screenshots your screen → you talk → it answers out loud and points or
draws on screen.** On top of that it has **agents** ("heyclicky agent …") that run tasks in the background and tap
you when done, **~15 integrations** (Google Workspace, Notion, Dropbox, Apple Notes/Calendar/Reminders, Spotify),
**dictation**, and a continuous **talk** mode. It's cloud-only (Claude, GPT models, a custom router), with
subscription quotas.

Wisp already matches or beats it on: pointing and on-screen annotation, step-by-step teaching, long-term memory,
on-screen computer control with approvals, privacy (everything can run locally), model choice, and user drawing.

The gaps that matter most, in order:

1. **Wisp doesn't look at the screen unless you draw.** A spoken question without a drawing goes to the model with
   no screenshot. HeyClicky always captures on the hotkey. This is the single biggest quality gap.
2. **Background agents + integrations.** Wisp's agents act through the mouse and keyboard in the foreground; HeyClicky's
   run in the background through APIs and apps, and notify you when finished.
3. **Speed of the voice loop.** HeyClicky streams speech-to-text and uses a fast first-layer model; Wisp transcribes
   after you let go and its default local model is slow.
4. **Dictation** and a hands-free **talk** mode.
5. **Zero-setup first run** (no model downloads, no keys).

## Feature comparison

| Area | HeyClicky | Wisp today | Gap |
|---|---|---|---|
| Invocation | Hold ⌃⌥ | Hold ⌃⌥ (configurable), menu bar, mouse button | — |
| Screen context | Screenshot on every hotkey, all displays | Only the area you draw (or a recent one, reused) | **Big** |
| Pointing | Cursor flies to the element, multi-monitor | Buddy flies to element resolved via Accessibility + OCR | Multi-monitor needs full-screen context |
| Drawing on screen (by the AI) | "Draws right on your screen to point the way" | Arrow, circle, highlight, spotlight guidance | Parity |
| Drawing by the user | Not documented | Pen while holding the shortcut; marks sent with the question | Wisp ahead |
| Voice in | Streaming STT (AssemblyAI), words appear live | Local Whisper, transcribed after release | Latency; no live words |
| Voice out | ElevenLabs | Local Kokoro, sentence streaming | Optional premium voice |
| Talk mode | Continuous conversation, interruptible; demo of hands-free control (reported) | Push-to-talk only; a new hold interrupts speech | **Medium** |
| Dictation | Included on all plans | None | **Medium** |
| Teaching | "Teaches you any tool" | Teacher mode with lessons and step checks | Wisp ahead |
| Agents | "heyclicky agent …" by voice; background tasks (research, files, apps, email); notifies when done; Codex bundled for building (reported) | Agents act on screen with approvals; started from the UI; no voice trigger; no completion notification | **Big** |
| Integrations | ~15 services: Google Workspace, Notion, Dropbox, Apple Notes/Calendar/Reminders, Spotify (reported) | None (only on-screen control) | **Big** |
| Proactivity | Offers help after time in one app (reported) | None | Small (opt-in only) |
| Memory | Last ~10 exchanges + text summaries | 40-message history with summarising, long-term memory with embeddings, files with citations | Wisp ahead |
| Routines | Not documented | Scheduled routines | Wisp ahead |
| Models | Router: fast realtime model, Claude for vision, GPT for agents (reported) | Local (Ollama, Apple FM), cloud keys (OpenAI, Anthropic, Gemini), latency-aware routing; one general + one vision slot | Per-task slots (fast / vision / agent) |
| Privacy | Screenshots on hotkey only, not stored; cloud processing | Local-only by default; cloud only if you allow; captures are kept in a local cache | Wisp ahead |
| Status UI | Floating overlay with waveform and response text | Cursor buddy + notch island with caption | Parity |
| Setup | Download, grant permissions, talk. No keys, no models | Needs Ollama + a model download, or keys | **Medium** |
| Price / accounts | Free 25/25, Pro $20, Max $100 per month | Free, no account | Wisp ahead |
| Platforms | Mac (Windows waitlist) | Mac | — |

## Plan

Effort estimates are for one engineer working with the existing codebase. Each step ships on its own.

### Step 1 — See the screen on every question, and point/draw on it (≈1 week) ✅ done 2026-10-12

Answers can already point: the model marks `[[point: Save]]`, the buddy flies there and a highlight box is drawn.
But this only works when there's a screenshot, which today means only when the user drew something. This step makes
it work on every answer, and extends it:

- **Richer marks:** the model can ask for a circle, arrow, underline or spotlight, not just a highlight box. The
  overlay can already render these.
- **Agents show what they're doing:** before each click or type, the buddy flies to the target, outlines it and
  pauses briefly. The approval card points at the same spot.

- On every hold, capture the screen behind the pointer when the user didn't draw:
  - Capture the window under the pointer by default, or the whole display (a setting).
  - Run OCR and the Accessibility summary on it, as focus captures already do.
- If the user drew, keep using the drawn area, plus a small downscaled full-display image for orientation.
- Multi-display: capture the display under the pointer. Optionally capture all displays, labelled `screen1…N`.
  Pointing resolves the element on the right display.
- Capture starts when the hold begins, so the screenshot is ready by the time the user lets go. Nothing is captured
  without the hotkey.
- Privacy:
  - Skip windows the user has excluded (password managers by default).
  - Never read secure fields (already enforced).
- Acceptance: "what's this button do?" with no drawing answers about the window under the pointer, and points at the
  right control.

### Step 2 — A faster voice loop (≈1 week) ✅ done 2026-10-12 (see D-041 for measurements)

- **Live transcription:**
  - Transcribe 100 ms chunks while the user talks (Whisper with a sliding window, or the Apple Speech framework on
    device).
  - Show the words in the island as they come.
  - Finalise on release, so the question is sent within about 300 ms.
- **Warm the model when the hold starts:**
  - Ollama: send a keep-alive request.
  - Cloud: open the connection ahead of time.
- **Fast-first routing:** add model slots for **Fast** (chat), **Vision** (screen questions) and **Agent**:
  - Default Fast to a small instruct model; route to Vision only when an image is attached.
- **Barge-in:** speaking or holding during an answer stops it immediately (verify end to end).
- Acceptance:
  - Median time from release to first spoken word under 2 s with a local instruct model.
  - Under 1.5 s with a cloud model.

### Step 3 — Voice-triggered background agents (≈2 weeks)

- **Trigger:**
  - Saying "agent, …" or "Wisp agent, …" at the start of a spoken question, or holding ⌃⌥⇧, starts an agent run
    instead of a Q&A.
  - The island shows "Agent started" and a running count.
- **Background tools** that don't take over the mouse:
  - Files in folders the user grants: read, organise, rename, move. Moves and deletes go to the Trash and need
    approval.
  - Web research: search and fetch. Only when cloud or network use is allowed; results are cited.
  - Apple apps via native frameworks:
    - Reminders and Calendar through EventKit.
    - Notes through AppleScript.
    - Music and Spotify playback through AppleScript.
  - The existing on-screen tools stay available, and a run uses them only when no background tool fits.
- **Completion:**
  - A notification plus an island "Done" chip; clicking it opens the run.
  - Spoken summary if the user is idle.
- **Safety:**
  - The same permission policy as computer control: reads are allowed; writes ask; sending and deleting are always
    High risk.
  - Per-run limits (steps, minutes, tool calls) already exist.
- Acceptance: "agent, remind me tomorrow at 9 to call Sam" creates the reminder without touching the screen, then
  says it's done.

### Step 4 — Integrations through MCP (≈2 weeks)

- Add an MCP client to the core:
  - Local stdio servers.
  - Remote servers with OAuth handled by the user in their browser.
- Expose MCP tools to agents and Q&A.
- Settings → Connections:
  - Add a server, see its tools, allow or deny each one.
  - Every tool call goes through the approval policy.
- Starter catalogue (documented, not bundled): Google Workspace, Notion, Dropbox, GitHub, Slack.
- Local Only mode only allows local MCP servers.
- Acceptance: with a Notion MCP server connected, "agent, add these notes to my Project page" works with one
  approval.

### Step 5 — Dictation (≈3 days)

- A separate hold chord (default ⌃⌥ + Space, configurable) records speech, transcribes it locally, and inserts the
  text at the caret:
  - Use Accessibility value-set where supported.
  - Otherwise paste and restore the previous clipboard.
- Optional "clean up" pass (punctuation, filler words) with the Fast model.
- Never into secure fields.
- Acceptance: dictating into Notes, Slack and a browser text box inserts the text in under 1 s after release.

### Step 6 — Talk mode (≈1 week)

- Tap the shortcut twice to start a conversation:
  - Wisp listens and uses voice activity detection to end each turn.
  - It answers and listens again.
- The island shows a "Talking" pill with a stop button; Esc or silence for 20 s ends it.
- Each turn includes a fresh screenshot (Step 1).
- Always-on listening without a key press stays out of scope by default (privacy). It could be an explicit,
  clearly indicated option later.

### Step 7 — First run and polish (≈1 week)

- **Zero-setup first run:**
  - If no model is available, use Apple's on-device model immediately.
  - Offer a one-click download of the recommended instruct model in the background.
  - Offer cloud keys as an alternative, not a requirement.
- **Optional premium voices** (cloud TTS with the user's key), off in Local Only.
- **Opt-in proactive tips:** after N minutes in an app, the island offers "Want a quick tour of {app}?".
  - Uses only the frontmost app's name, never a screenshot.
  - Off by default.
- **Island "Done"/agent chips and history.** Settings to hide the island or the cursor buddy.

## What we deliberately won't copy

- Accounts, quotas and subscriptions. Wisp stays free and local-first.
- Always-on microphone by default. HeyClicky's hands-free demo is impressive, but listening without a key press
  breaks Wisp's privacy promise. If we do it, it's opt-in and clearly indicated.
- Bundling a cloud coding agent. Building software is out of Wisp's scope; MCP lets users connect one.
- HeyClicky's name, mascot, colours or copy.

## Where Wisp is already ahead (keep and show it)

- Runs fully on device, with Local Only enforced end to end.
- User drawing while talking, burned into the capture.
- Teacher mode with checked steps.
- Long-term memory, files with citations, routines.
- Approval-bound computer control with audit log.
- Model choice: local, Apple, OpenAI, Anthropic, Gemini.

## Open questions

- Default screen context for Step 1: the window under the pointer (cheaper, more private) or the whole display
  (more context)? Recommendation: window by default, with a setting.
- Agent trigger: a spoken phrase, a separate chord, or both? Recommendation: both.
- Which Apple apps to support first in Step 3? Recommendation: Reminders, Calendar, Notes, Music.

## Sources

- [heyclicky.com](https://www.heyclicky.com/) — features, plans, privacy statements
- [farzaa/clicky (MIT)](https://github.com/farzaa/clicky) and its CLAUDE.md — architecture of the open-source version
- [Farza on LinkedIn — Introducing Clicky Agents](https://www.linkedin.com/posts/farza-majeed-76685612a_introducing-clicky-agents-this-is-the-simplest-activity-7454552863227285504-vQrQ)
- [TBPN digest — Hey Clicky built in 8 weeks](https://www.tbpndigest.com/story/2026-06-10/hey-clicky-founder-farza-majeed-built-a-voice-controlled-ai-desktop-agent-in-8-weeks-now-using-claude-4-by-default)
- [AI by Aakash — Clicky](https://www.aibyaakash.com/p/clicky)
- [VoiceOS — VoiceOS vs Clicky](https://www.voiceos.com/compare/clicky)
- [explainx — HeyClicky hands-free demo](https://explainx.ai/blog/heyclicky-voice-control-mac-gpt-realtime-2-demo-2026)
- [Product Hunt — Clicky](https://www.producthunt.com/products/clicky-2)
- [EveryDev — Hey Clicky](https://www.everydev.ai/tools/hey-clicky)
