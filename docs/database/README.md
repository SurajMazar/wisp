# Database

SQLite at `<data dir>/database/wisp.sqlite3`, opened by `wisp-core` only (React never touches it).

- WAL journal, `foreign_keys=ON`, `synchronous=NORMAL`, 5 s busy timeout.
- Primary keys: UUIDv7 text (time-ordered). Timestamps: integer Unix milliseconds.
- JSON columns are `TEXT CHECK (json_valid(...))`; their shapes are validated in Rust.
- API keys are never stored; `providers.secret_ref` names a Keychain item.

## Migrations

`database/migrations/NNNN_name.sql`, embedded at compile time (`crates/database/build.rs`), applied in
order in a transaction, recorded in `schema_migrations` with a SHA-256. If an applied migration's
checksum changes, startup fails loudly. A database newer than the build is refused.

| # | Tables |
|---|---|
| 0001 | settings, permissions, events, diagnostics |
| 0002 | providers, models |
| 0003 | projects, conversations, messages, attachments |
| 0004 | skills, skill_versions, agents, agent_skills, agent_runs, agent_steps, tool_permissions, tool_calls |
| 0005 | memories, memory_embeddings |
| 0006 | files, file_chunks |
| 0007 | routines, routine_runs |

## Settings

One row per section (`general`, `appearance`, `privacy`, `onboarding`). A corrupt section falls back
to defaults without discarding the others.
