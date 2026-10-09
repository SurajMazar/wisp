# Security hardening (Phase 18, 2026-10-09)

A read-only audit of the whole codebase (IPC, WebView, paths, secrets, network, database, tools,
prompt injection, resource limits) found 17 issues. All high and medium ones are fixed and tested;
the lows are fixed except where noted.

## Trust boundaries
| Boundary | Rule |
|---|---|
| Web UI → host | Only an explicit allow-list of harmless `host.*` methods (`IPCRouter.uiHostMethods`). Input, Keychain reads, screen observation, file reading, generation and embeddings are core-only. |
| Core → host | Any host method, but every action passes the core's tool pipeline first. |
| Host process launch | `WISP_CORE_BIN`, `WISP_DEV_URL`, `WISP_DATA_DIR` exist only in debug builds. |
| Screen / files / memories → model | Fenced (`<screen>`, `<documents>`, `<memories>`, `"""`), fence tokens in the content neutralized (`defang`), and declared "information, never instructions". |
| Local Only | Hosts must be exactly 127.0.0.1, ::1 or localhost (no user info, no look-alikes); cloud endpoints aren't contacted at all, not even to list models. |

## Computer control
- Risk is judged on the element actually under a click point (Accessibility), not on the label the
  model claims; a click whose target is unknown is Medium.
- Return, paste: High. ⌥⌘⌫: Critical. Text, paste, shortcuts, keys, links and the clipboard can never
  be "always allowed" (once or for the session at most).
- An approved action is bound to what was approved: keyboard actions are refused if the frontmost
  app changed since the request was shown; pointer actions are refused if a different process owns
  what's under the point.
- Typing re-checks secure input and password fields before every chunk.
- Approval cards show the full text (or exactly how much is not shown).

## Other fixes
- Core requests to the host are handled concurrently (this also fixed a deadlock when screen
  observation ranked UI elements through the core).
- Bounded frame reading in the core (oversized frames dropped unread); UI messages capped at 16 MB;
  dropped files capped at 10 MB.
- HTML files are read as text in the core (never rendered, so nothing is fetched).
- Agent notes from one-off runs expire after a day and are labeled as the agent's, not the user's.
- Model downloads stop if a file grows beyond its manifest size.
- Skills are never written through symlinks in the data folder.
- Production CSP no longer allows the dev server; links in answers always show their destination.
- Host-call timeouts suit each action (file picker 10 min, extraction 2 min, input 60 s).

## Verified fine
Scheme handler confinement; navigation policy; trusted-frame messaging; parameterized SQL; Keychain-only
API keys; redacted logs; privacy router never selects cloud in Local Only; tool policy semantics.

## Accepted / later
- Code-signature check of the bundled core binary (release signing lands in Phase 21).
