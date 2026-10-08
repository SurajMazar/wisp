-- 0004_agents: agents, runs, steps, skills, tool permissions and tool calls.

CREATE TABLE skills (
    id          TEXT PRIMARY KEY NOT NULL,
    slug        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    source      TEXT NOT NULL,               -- builtin | user | imported
    enabled     INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    current_version_id TEXT,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE skill_versions (
    id          TEXT PRIMARY KEY NOT NULL,
    skill_id    TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    version     INTEGER NOT NULL,
    content     TEXT NOT NULL,               -- SKILL.md body
    sha256      TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    UNIQUE (skill_id, version)
);

CREATE TABLE agents (
    id            TEXT PRIMARY KEY NOT NULL,
    name          TEXT NOT NULL,
    description   TEXT NOT NULL DEFAULT '',
    avatar        TEXT,
    instructions  TEXT NOT NULL DEFAULT '',
    model_pref    TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(model_pref)),
    tools         TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(tools)),
    memory_scope  TEXT NOT NULL DEFAULT 'agent',
    limits        TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(limits)),
    workspace     TEXT,
    enabled       INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);

CREATE TABLE agent_skills (
    agent_id  TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    skill_id  TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    PRIMARY KEY (agent_id, skill_id)
);

CREATE TABLE agent_runs (
    id              TEXT PRIMARY KEY NOT NULL,
    agent_id        TEXT REFERENCES agents(id) ON DELETE SET NULL,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    trigger         TEXT NOT NULL,           -- user | routine
    goal            TEXT NOT NULL,
    status          TEXT NOT NULL,           -- planning | running | paused | waitingPermission | completed | failed | cancelled | stuck
    plan            TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(plan)),
    error           TEXT,
    steps_used      INTEGER NOT NULL DEFAULT 0,
    tool_calls_used INTEGER NOT NULL DEFAULT 0,
    tokens_used     INTEGER NOT NULL DEFAULT 0,
    started_at      INTEGER NOT NULL,
    finished_at     INTEGER
);
CREATE INDEX idx_agent_runs_agent ON agent_runs(agent_id, started_at);

CREATE TABLE agent_steps (
    id          TEXT PRIMARY KEY NOT NULL,
    run_id      TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    kind        TEXT NOT NULL,               -- observe | plan | act | verify | message
    title       TEXT NOT NULL,
    status      TEXT NOT NULL,               -- pending | running | succeeded | failed | skipped
    detail      TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(detail)),
    started_at  INTEGER,
    finished_at INTEGER,
    UNIQUE (run_id, seq)
);

-- Persistent tool grants. Scope columns are nullable: NULL means "any".
CREATE TABLE tool_permissions (
    id          TEXT PRIMARY KEY NOT NULL,
    tool        TEXT NOT NULL,
    agent_id    TEXT REFERENCES agents(id) ON DELETE CASCADE,
    decision    TEXT NOT NULL CHECK (decision IN ('alwaysAllow', 'deny')),
    constraints TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(constraints)),
    created_at  INTEGER NOT NULL,
    revoked_at  INTEGER
);
CREATE INDEX idx_tool_permissions_tool ON tool_permissions(tool, agent_id);

-- Audit trail of every tool request and its outcome.
CREATE TABLE tool_calls (
    id            TEXT PRIMARY KEY NOT NULL,
    tool          TEXT NOT NULL,
    risk          TEXT NOT NULL CHECK (risk IN ('safe', 'low', 'medium', 'high', 'critical')),
    arguments     TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(arguments)), -- redacted
    decision      TEXT NOT NULL,             -- allowOnce | allowSession | alwaysAllow | deny | policyDeny | autoAllow
    status        TEXT NOT NULL,             -- requested | running | succeeded | failed | denied
    result        TEXT,
    error         TEXT,
    agent_run_id  TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    message_id    TEXT REFERENCES messages(id) ON DELETE SET NULL,
    created_at    INTEGER NOT NULL,
    finished_at   INTEGER
);
CREATE INDEX idx_tool_calls_created ON tool_calls(created_at);
CREATE INDEX idx_tool_calls_run ON tool_calls(agent_run_id);
