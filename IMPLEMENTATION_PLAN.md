# Implementation Plan

Phases follow spec §32. A phase is complete only when its stop gate passes with evidence recorded in [PHASE_REPORT.md](PHASE_REPORT.md). Gates marked **(M)** contain manual checks that need a person at the Mac.

| # | Phase | Key deliverables | Gate type |
|---|---|---|---|
| 0 | Repository audit & architecture | Docs, decisions, monorepo skeleton, toolchain | Automated |
| 1 | Application foundation | Host app, WKWebView UI, `wisp-core`, IPC, SQLite + migrations, settings persist, menu bar | Automated + **(M)** launch |
| 2 | Invocation system | ⌘⌥ via event tap, recorder, hold/toggle/PTT, menu bar invoke, conflicts | **(M)** |
| 3 | Assistant shell & cursor | Floating panel, companion cursor, multi-display, themes, reduced motion | **(M)** |
| 4 | Screen capture & focus | ScreenCaptureKit, pointer/rect/lasso/circle focus, Retina + multi-display coords, overlay | Automated geometry + **(M)** |
| 5 | Accessibility | AX onboarding, app/window/element detection, relevance filtering | **(M)** |
| 6 | AI providers | Provider trait, Ollama, Apple FM, OpenAI-compatible, router, privacy modes, streaming, cancel | Automated (2 real providers) |
| 7 | First AI vertical slice | Summon → ask → stream → persist → history | Automated + **(M)** |
| 8 | Voice pipeline | Mic → VAD → Whisper → LLM → Kokoro → speaker, interruption | **(M)** |
| 9 | Screen understanding | Context builder, vision routing, compression, caching | **(M)** |
| 10 | Visual guidance | Annotation overlay, coordinate mapping, dismissal | **(M)** |
| 11 | Computer control | Tool registry, schemas, risk, approvals UI, execution, audit, verification | **(M)** |
| 12 | Teacher mode | Step loop with highlight/speak/wait/observe/verify | **(M)** |
| 13 | Agents | Definitions, plan/act/observe/verify, limits, loop detection, activity UI | **(M)** |
| 14 | Skills | SKILL.md parser, registry, versions, import/export | Automated |
| 15 | Memory | Extraction, retrieval, ranking, inspection, deletion | Automated |
| 16 | Files & semantic search | Import, parse, chunk, embed, search, citations | Automated |
| 17 | Routines | Scheduler, timezone, retries, notifications, history | Automated + **(M)** |
| 18 | Security hardening | Audit across secrets/IPC/DB/files/tools/shell/network/logs | Automated |
| 19 | Performance | Measure + optimize latency, memory, CPU | Automated + **(M)** |
| 20 | Polish & UX | Every state on every screen, onboarding, a11y | **(M)** |
| 21 | Release | Release build, signing/notarization config, updates, backups | **(M)** |

## Per-phase loop

```
IMPLEMENT → COMPILE → TEST → FIX → INTEGRATE → POLISH (only after gate)
```

Every phase updates PROJECT_STATUS.md and appends to PHASE_REPORT.md.

## Manual verification

`docs/release/manual-checklist.md` collects every **(M)** item (TCC permissions, Retina, multi-monitor, mic, audio out, Ollama, Apple FM, Whisper, Kokoro). Automated tests never stand in for these.
