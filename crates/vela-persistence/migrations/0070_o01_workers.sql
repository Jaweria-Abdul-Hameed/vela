-- O01: workers. `state` holds the serialized worker state (phase, or paused/needs-human with its resume state).
CREATE TABLE workers (
    id               TEXT PRIMARY KEY,
    run_id           TEXT NOT NULL REFERENCES build_runs(id),
    ticket_id        TEXT NOT NULL,
    state            TEXT NOT NULL,
    waiting_approval INTEGER NOT NULL DEFAULT 0 CHECK (waiting_approval IN (0, 1)),
    conversation_id  TEXT,
    branch           TEXT,
    worktree_path    TEXT,
    base_sha         TEXT,
    last_event_seq   INTEGER NOT NULL DEFAULT 0,
    created_ms       INTEGER NOT NULL,
    updated_ms       INTEGER NOT NULL
);
CREATE INDEX workers_run ON workers(run_id);
