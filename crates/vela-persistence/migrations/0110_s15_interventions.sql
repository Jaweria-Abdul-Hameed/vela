-- S15: anything that needs a human becomes an intervention record that preserves the work and names the way back.
CREATE TABLE human_interventions (
    id                TEXT PRIMARY KEY,
    run_id            TEXT NOT NULL REFERENCES build_runs(id),
    worker_id         TEXT REFERENCES workers(id),
    kind              TEXT NOT NULL,
    state             TEXT NOT NULL CHECK (state IN ('OPEN', 'RESOLVED')),
    resume_state      TEXT NOT NULL,
    summary           TEXT NOT NULL,
    evidence          TEXT NOT NULL DEFAULT '[]',
    preserved_work    TEXT NOT NULL DEFAULT '',
    last_safe_commit  TEXT,
    actions           TEXT NOT NULL DEFAULT '[]',
    consequences      TEXT NOT NULL DEFAULT '',
    created_ms        INTEGER NOT NULL,
    resolved_ms       INTEGER,
    resolution        TEXT
);
CREATE INDEX human_interventions_open ON human_interventions(run_id, state);
