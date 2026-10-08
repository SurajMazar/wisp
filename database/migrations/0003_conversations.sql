-- 0003_conversations: projects, folders, conversations, messages, attachments.

CREATE TABLE projects (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE conversations (
    id           TEXT PRIMARY KEY NOT NULL,
    project_id   TEXT REFERENCES projects(id) ON DELETE SET NULL,
    title        TEXT NOT NULL DEFAULT '',
    folder       TEXT,
    pinned       INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    archived     INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
    summary      TEXT,                       -- rolling summary for context-window management
    mode         TEXT NOT NULL DEFAULT 'talk', -- talk | teacher | agent
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);
CREATE INDEX idx_conversations_updated ON conversations(archived, pinned, updated_at);
CREATE INDEX idx_conversations_project ON conversations(project_id);

CREATE TABLE messages (
    id              TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    parent_id       TEXT REFERENCES messages(id) ON DELETE SET NULL, -- regeneration / edit branches
    role            TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system', 'tool')),
    content         TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL DEFAULT 'complete', -- streaming | complete | cancelled | error
    provider_id     TEXT,
    model_key       TEXT,
    locality        TEXT CHECK (locality IS NULL OR locality IN ('local', 'cloud')),
    context         TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(context)), -- focus/app/window metadata (no pixels)
    error           TEXT,
    tokens_in       INTEGER,
    tokens_out      INTEGER,
    latency_ms      INTEGER,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);
CREATE INDEX idx_messages_conversation ON messages(conversation_id, created_at);

CREATE TABLE attachments (
    id          TEXT PRIMARY KEY NOT NULL,
    message_id  TEXT REFERENCES messages(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,               -- screenshot | focusRegion | file | audio
    file_name   TEXT NOT NULL,
    mime_type   TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    path        TEXT NOT NULL,               -- relative to the data dir; validated on read
    sha256      TEXT NOT NULL,
    metadata    TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    created_at  INTEGER NOT NULL
);
CREATE INDEX idx_attachments_message ON attachments(message_id);
