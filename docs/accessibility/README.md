# Accessibility integration (design — Phase 5)

The host reads AX trees (`AXUIElement`) for the frontmost app/window, the element under the pointer and
the focused element, normalizes nodes (role, title, value, bounds, actions), and sends a compact,
relevance-filtered subset to the core. Raw trees are never logged or sent whole.

UI accessibility of Wisp itself (VoiceOver labels, keyboard navigation, reduced motion, contrast) is a
requirement of every screen, verified in Phase 20.
