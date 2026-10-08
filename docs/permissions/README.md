# Permissions

Two layers:

1. **macOS (TCC)** — Microphone, Screen Recording, Accessibility, Automation, Notifications. Owned by
   macOS; the host reads status and guides the user through granting it (Phase 4–5 onboarding).
   Features degrade gracefully: without Accessibility, chat/voice/screen still work and computer
   control shows as unavailable (spec §25).
2. **Wisp policy** — every tool call is checked by `crates/permissions` against its declared risk
   (`safe` … `critical`) and stored grants (`allowOnce`, `allowSession`, `alwaysAllow`, `deny`), scoped
   to user / agent / tool / session. Grants are revocable; decisions are audited in `tool_calls`.

Status: design. TCC status tracking lands with onboarding; the policy engine with computer control (Phase 11).
