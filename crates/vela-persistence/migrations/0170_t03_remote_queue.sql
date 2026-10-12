-- T03: updates to the remote tracker that must survive an outage and never be applied twice.
CREATE TABLE remote_updates (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id           TEXT,
    ticket_id        TEXT NOT NULL,
    kind             TEXT NOT NULL CHECK (kind IN ('status', 'comment', 'close')),
    payload          TEXT NOT NULL,
    idempotency_key  TEXT NOT NULL UNIQUE,
    state            TEXT NOT NULL CHECK (state IN ('PENDING', 'DONE', 'FAILED_PERMANENT')),
    attempts         INTEGER NOT NULL DEFAULT 0,
    next_attempt_ms  INTEGER NOT NULL,
    last_error       TEXT NOT NULL DEFAULT '',
    created_ms       INTEGER NOT NULL,
    updated_ms       INTEGER NOT NULL
);
CREATE INDEX remote_updates_due ON remote_updates(state, next_attempt_ms);
