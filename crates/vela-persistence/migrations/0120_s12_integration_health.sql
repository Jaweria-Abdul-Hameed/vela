-- S12: integration health is persisted with the run so a restart knows whether the integration branch is trustworthy, and the
-- number of times each worker was sent back to fixing after a failed integration validation.
ALTER TABLE build_runs ADD COLUMN integration_health TEXT NOT NULL DEFAULT 'HEALTHY'
    CHECK (integration_health IN ('HEALTHY', 'UNKNOWN', 'UNHEALTHY'));

CREATE TABLE integration_fix_counts (
    worker_id  TEXT PRIMARY KEY REFERENCES workers(id),
    attempts   INTEGER NOT NULL
);
