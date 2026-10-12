-- F06: the core tables. Later tickets add tables in their reserved blocks (SHARED_SURFACE_PROTOCOL.md section 3).

CREATE TABLE projects (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    repo_path       TEXT NOT NULL UNIQUE,
    created_ms      INTEGER NOT NULL,
    last_opened_ms  INTEGER,
    last_event_seq  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE build_runs (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL REFERENCES projects(id),
    state           TEXT NOT NULL,
    created_ms      INTEGER NOT NULL,
    updated_ms      INTEGER NOT NULL,
    last_event_seq  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX build_runs_project ON build_runs(project_id);

-- Append-only history. `seq` is assigned here, never by the producer.
CREATE TABLE event_journal (
    seq             INTEGER PRIMARY KEY AUTOINCREMENT,
    ts              INTEGER NOT NULL,
    run_id          TEXT,
    ticket_id       TEXT,
    worker_id       TEXT,
    kind            TEXT NOT NULL,
    payload         TEXT NOT NULL,
    schema_version  INTEGER NOT NULL
);
CREATE INDEX event_journal_run ON event_journal(run_id, seq);

CREATE TRIGGER event_journal_no_update BEFORE UPDATE ON event_journal
BEGIN SELECT RAISE(ABORT, 'event_journal is append-only'); END;
CREATE TRIGGER event_journal_no_delete BEFORE DELETE ON event_journal
BEGIN SELECT RAISE(ABORT, 'event_journal is append-only'); END;

CREATE TABLE operations (
    id              TEXT PRIMARY KEY,
    run_id          TEXT,
    kind            TEXT NOT NULL,
    state           TEXT NOT NULL,
    detail          TEXT NOT NULL DEFAULT '',
    created_ms      INTEGER NOT NULL,
    updated_ms      INTEGER NOT NULL
);

CREATE TABLE settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_ms      INTEGER NOT NULL
);

CREATE TABLE state_store_backups (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    path            TEXT NOT NULL,
    schema_version  INTEGER NOT NULL,
    created_ms      INTEGER NOT NULL
);
