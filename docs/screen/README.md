# Assistant cursor & focus system

**Status:** assistant cursor (Phase 3, D-019) and focus system (Phase 4, D-020) implemented.

- The **assistant cursor** is a click-through, per-display overlay panel that follows the pointer at a
  small offset (never replacing the system cursor), springs smoothly, flips near screen edges, and
  shows state (listening, thinking, speaking, executing…). Themes and reduced motion respected.
- **Focus modes**: pointer, element (AX), draw (rectangle / lasso / circle / point), window, screen.
- Coordinates cross IPC in global top-left **points**; each display's backing scale converts to pixels
  for cropping. Only the focused region (plus margin), not the full screen, is sent to models.
- Screen capture happens only on explicit focus/ask, agent observation, or user-enabled continuous mode.
