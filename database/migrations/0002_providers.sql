-- 0002_providers: AI providers and the models they expose.
-- API keys are NEVER stored here; `secret_ref` names a Keychain item.

CREATE TABLE providers (
    id          TEXT PRIMARY KEY NOT NULL,
    kind        TEXT NOT NULL,               -- apple | ollama | openai | anthropic | gemini | openaiCompatible
    name        TEXT NOT NULL,
    locality    TEXT NOT NULL CHECK (locality IN ('local', 'cloud')),
    base_url    TEXT,
    secret_ref  TEXT,
    enabled     INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    config      TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(config)),
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE models (
    id              TEXT PRIMARY KEY NOT NULL,
    provider_id     TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    model_key       TEXT NOT NULL,           -- provider-native identifier, e.g. "qwen3:8b"
    display_name    TEXT NOT NULL,
    kind            TEXT NOT NULL,           -- llm | vision | stt | tts | embedding
    capabilities    TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(capabilities)),
    context_length  INTEGER,
    size_bytes      INTEGER,
    status          TEXT NOT NULL,           -- available | installed | running | unavailable | needsDownload | needsPermission
    path            TEXT,
    checksum        TEXT,
    license         TEXT,
    source          TEXT,
    metadata        TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    UNIQUE (provider_id, model_key)
);
CREATE INDEX idx_models_kind ON models(kind);
