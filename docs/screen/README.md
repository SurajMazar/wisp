# Assistant cursor & focus system

**Status:** assistant cursor implemented (Phase 3, see DECISIONS.md D-019); focus system is Phase 4.

- The **assistant cursor** is a click-through, per-display overlay panel that follows the pointer at a
  small offset (never replacing the system cursor), springs smoothly, flips near screen edges, and
  shows state (listening, thinking, speaking, executing…). Themes and reduced motion respected.
- **Focus modes**: pointer, element (AX), draw (rectangle / lasso / circle / point), window, screen.
- Coordinates cross IPC in global top-left **points**; each display's backing scale converts to pixels
  for cropping. Only the focused region (plus margin), not the full screen, is sent to models.
- Screen capture happens only on explicit focus/ask, agent observation, or user-enabled continuous mode.
