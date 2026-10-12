-- S04: preflight reports, plus the adapter capability and skill installation records the Antigravity checks persist here.
CREATE TABLE preflight_reports (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id),
    verdict     TEXT NOT NULL CHECK (verdict IN ('PASS', 'WARN', 'BLOCK')),
    report      TEXT NOT NULL,
    created_ms  INTEGER NOT NULL
);
CREATE INDEX preflight_reports_project ON preflight_reports(project_id, created_ms);

CREATE TABLE adapter_capability_snapshots (
    id          TEXT PRIMARY KEY,
    adapter     TEXT NOT NULL,
    version     TEXT NOT NULL,
    snapshot    TEXT NOT NULL,
    created_ms  INTEGER NOT NULL
);
CREATE INDEX adapter_capability_adapter ON adapter_capability_snapshots(adapter, created_ms);

CREATE TABLE skill_installations (
    id            TEXT PRIMARY KEY,
    adapter       TEXT NOT NULL,
    scope         TEXT NOT NULL,
    skill         TEXT NOT NULL,
    content_hash  TEXT NOT NULL,
    installed_ms  INTEGER NOT NULL
);
CREATE INDEX skill_installations_adapter ON skill_installations(adapter, scope, skill);
