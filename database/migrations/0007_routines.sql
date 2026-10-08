-- 0007_routines: scheduled routines and their run history.

CREATE TABLE routines (
    id            TEXT PRIMARY KEY NOT NULL,
    name          TEXT NOT NULL,
    schedule      TEXT NOT NULL,             -- cron-like expression, validated in Rust
    timezone      TEXT NOT NULL,             -- IANA zone
    agent_id      TEXT REFERENCES agents(id) ON DELETE SET NULL,
    prompt        TEXT NOT NULL,
    enabled       INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    retry_policy  TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(retry_policy)),
    notify        TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(notify)),
    next_run_at   INTEGER,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);
CREATE INDEX idx_routines_next ON routines(enabled, next_run_at);

CREATE TABLE routine_runs (
    id            TEXT PRIMARY KEY NOT NULL,
    routine_id    TEXT NOT NULL REFERENCES routines(id) ON DELETE CASCADE,
    attempt       INTEGER NOT NULL DEFAULT 1,
    status        TEXT NOT NULL,             -- running | succeeded | failed
    agent_run_id  TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    error         TEXT,
    scheduled_for INTEGER NOT NULL,
    started_at    INTEGER NOT NULL,
    finished_at   INTEGER
);
CREATE INDEX idx_routine_runs_routine ON routine_runs(routine_id, started_at);
