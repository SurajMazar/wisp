# Agents, skills and memory (design — Phases 13–15)

- **Agents**: observe → plan → permission → act → observe → verify, with limits on steps, runtime,
  tool calls and tokens; repeated-action detection pauses with "The assistant may be stuck".
- **Skills**: `SKILL.md` folders; versioned in `skill_versions`; imported skills are never executed automatically.
- **Memory**: user-inspectable and deletable; categories × levels (temporary/session/project/persistent);
  retrieved by relevance, never blindly injected into every prompt.
