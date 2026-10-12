-- S02: project settings (stored in Vela, never in the repository), the immutable per-run settings snapshot, and consent records.
CREATE TABLE project_settings (
    project_id  TEXT NOT NULL REFERENCES projects(id),
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,
    updated_ms  INTEGER NOT NULL,
    PRIMARY KEY (project_id, key)
);

CREATE TABLE run_settings_snapshot (
    run_id      TEXT PRIMARY KEY REFERENCES build_runs(id),
    snapshot    TEXT NOT NULL,
    created_ms  INTEGER NOT NULL
);

CREATE TABLE consents (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    key           TEXT NOT NULL,
    text_version  TEXT NOT NULL,
    granted       INTEGER NOT NULL CHECK (granted IN (0, 1)),
    scope         TEXT NOT NULL DEFAULT 'user',
    recorded_ms   INTEGER NOT NULL
);
CREATE INDEX consents_key ON consents(key, id);
