-- 0006_files: imported/indexed files and their chunks (with embeddings).

CREATE TABLE files (
    id           TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    origin_path  TEXT,                       -- original location (informational; may be NULL for drops)
    stored_path  TEXT NOT NULL,              -- relative to the data dir
    mime_type    TEXT NOT NULL,
    size_bytes   INTEGER NOT NULL,
    sha256       TEXT NOT NULL,
    status       TEXT NOT NULL,              -- pending | indexing | indexed | failed
    error        TEXT,
    project_id   TEXT REFERENCES projects(id) ON DELETE SET NULL,
    created_at   INTEGER NOT NULL,
    indexed_at   INTEGER
);
CREATE INDEX idx_files_sha ON files(sha256);

CREATE TABLE file_chunks (
    id          TEXT PRIMARY KEY NOT NULL,
    file_id     TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    content     TEXT NOT NULL,
    locator     TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(locator)), -- page / line range for citations
    model       TEXT,
    dims        INTEGER,
    vector      BLOB,
    created_at  INTEGER NOT NULL,
    UNIQUE (file_id, seq)
);
