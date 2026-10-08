-- 0005_memory: user-inspectable memory and its embeddings.

CREATE TABLE memories (
    id               TEXT PRIMARY KEY NOT NULL,
    category         TEXT NOT NULL,          -- userProfile | preference | project | fact | conversation | agent | skill
    level            TEXT NOT NULL,          -- temporary | session | project | persistent
    content          TEXT NOT NULL,
    source           TEXT NOT NULL DEFAULT 'user', -- user | extracted | agent
    project_id       TEXT REFERENCES projects(id) ON DELETE CASCADE,
    agent_id         TEXT REFERENCES agents(id) ON DELETE CASCADE,
    conversation_id  TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    importance       REAL NOT NULL DEFAULT 0.5,
    access_count     INTEGER NOT NULL DEFAULT 0,
    last_accessed_at INTEGER,
    expires_at       INTEGER,
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
);
CREATE INDEX idx_memories_category ON memories(category, level);
CREATE INDEX idx_memories_scope ON memories(project_id, agent_id);

CREATE TABLE memory_embeddings (
    memory_id   TEXT PRIMARY KEY NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    model       TEXT NOT NULL,
    dims        INTEGER NOT NULL,
    vector      BLOB NOT NULL,               -- little-endian f32 × dims
    created_at  INTEGER NOT NULL
);
