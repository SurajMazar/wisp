-- 0001_initial: settings, permissions, events, diagnostics.
-- Conventions: TEXT UUIDv7 primary keys, INTEGER unix-ms timestamps, JSON in TEXT columns
-- validated by the Rust repository layer.

CREATE TABLE settings (
    key         TEXT PRIMARY KEY NOT NULL,
    value       TEXT NOT NULL CHECK (json_valid(value)),
    updated_at  INTEGER NOT NULL
);

-- App-level permission state as last observed (TCC status is owned by macOS; this records
-- what Wisp saw and when, for the permission center and diagnostics).
CREATE TABLE permissions (
    kind        TEXT PRIMARY KEY NOT NULL,   -- microphone | screenRecording | accessibility | automation | notifications | files
    status      TEXT NOT NULL,               -- enabled | needsAttention | notRequested
    checked_at  INTEGER NOT NULL
);

-- Internal event log (metadata only; never content). Bounded by a retention job.
CREATE TABLE events (
    id          TEXT PRIMARY KEY NOT NULL,
    type        TEXT NOT NULL,
    payload     TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload)),
    created_at  INTEGER NOT NULL
);
CREATE INDEX idx_events_type_created ON events(type, created_at);
CREATE INDEX idx_events_created ON events(created_at);

-- Diagnostics: provider/model latency, token usage, errors. No prompt or response content.
CREATE TABLE diagnostics (
    id           TEXT PRIMARY KEY NOT NULL,
    category     TEXT NOT NULL,              -- model | tool | agent | audio | screen | system
    name         TEXT NOT NULL,
    level        TEXT NOT NULL DEFAULT 'info',
    provider     TEXT,
    model        TEXT,
    latency_ms   INTEGER,
    tokens_in    INTEGER,
    tokens_out   INTEGER,
    data         TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(data)),
    created_at   INTEGER NOT NULL
);
CREATE INDEX idx_diagnostics_category_created ON diagnostics(category, created_at);
