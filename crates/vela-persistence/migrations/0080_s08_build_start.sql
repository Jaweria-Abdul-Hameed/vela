-- S08: the base a run started from, its integration branch and worktree, and the worktrees Vela manages.
ALTER TABLE build_runs ADD COLUMN base_sha TEXT;
ALTER TABLE build_runs ADD COLUMN base_branch TEXT;
ALTER TABLE build_runs ADD COLUMN integration_branch TEXT;
ALTER TABLE build_runs ADD COLUMN integration_worktree TEXT;

CREATE TABLE worktrees (
    path        TEXT PRIMARY KEY,
    run_id      TEXT NOT NULL REFERENCES build_runs(id),
    kind        TEXT NOT NULL CHECK (kind IN ('INTEGRATION', 'WORKER')),
    branch      TEXT NOT NULL,
    base_sha    TEXT NOT NULL,
    created_ms  INTEGER NOT NULL,
    removed_ms  INTEGER
);
CREATE INDEX worktrees_run ON worktrees(run_id);
