-- S10: every validation command a worker ran, with its result, so evidence survives a restart.
CREATE TABLE test_runs (
    id           TEXT PRIMARY KEY,
    run_id       TEXT NOT NULL REFERENCES build_runs(id),
    worker_id    TEXT NOT NULL REFERENCES workers(id),
    ticket_id    TEXT NOT NULL,
    command      TEXT NOT NULL,
    exit_code    INTEGER,
    passed       INTEGER NOT NULL CHECK (passed IN (0, 1)),
    timed_out    INTEGER NOT NULL CHECK (timed_out IN (0, 1)),
    duration_ms  INTEGER NOT NULL,
    output_tail  TEXT NOT NULL DEFAULT '',
    created_ms   INTEGER NOT NULL
);
CREATE INDEX test_runs_worker ON test_runs(worker_id, created_ms);
