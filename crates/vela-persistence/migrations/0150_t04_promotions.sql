-- T04: how a finished run was handed to the user. One row per run.
CREATE TABLE promotions (
    run_id       TEXT PRIMARY KEY REFERENCES build_runs(id),
    mode         TEXT NOT NULL CHECK (mode IN ('PULL_REQUEST', 'LOCAL_ONLY', 'AUTO_MERGE')),
    state        TEXT NOT NULL CHECK (state IN ('NONE', 'DRAFT', 'READY', 'LOCAL_READY', 'MERGED')),
    branch       TEXT NOT NULL,
    base_branch  TEXT NOT NULL,
    pr_url       TEXT,
    note         TEXT NOT NULL DEFAULT '',
    updated_ms   INTEGER NOT NULL
);
