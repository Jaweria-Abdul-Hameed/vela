-- K01/K02/K09: approval requests and their decisions, delivery attempts, loop-guard counters and scoped allow rules.
CREATE TABLE approval_requests (
    id            TEXT PRIMARY KEY,
    run_id        TEXT NOT NULL REFERENCES build_runs(id),
    worker_id     TEXT NOT NULL,
    source        TEXT NOT NULL,
    evidence_tier TEXT NOT NULL CHECK (evidence_tier IN ('WEAK', 'OBSERVED', 'BOUND')),
    class         TEXT NOT NULL,
    executable    TEXT NOT NULL,
    fingerprint   TEXT NOT NULL,
    state         TEXT NOT NULL CHECK (state IN ('PENDING', 'ALLOWED', 'DENIED', 'ASKED', 'STALLED', 'UNDELIVERABLE')),
    decision      TEXT,
    rule_id       TEXT,
    reason        TEXT NOT NULL DEFAULT '',
    created_ms    INTEGER NOT NULL,
    decided_ms    INTEGER
);
CREATE INDEX approval_requests_worker ON approval_requests(worker_id, created_ms);

CREATE TABLE approval_deliveries (
    id           TEXT PRIMARY KEY,
    approval_id  TEXT NOT NULL REFERENCES approval_requests(id),
    adapter      TEXT NOT NULL,
    attempt      INTEGER NOT NULL,
    verified     INTEGER NOT NULL CHECK (verified IN (0, 1)),
    detail       TEXT NOT NULL DEFAULT '',
    ts_ms        INTEGER NOT NULL
);

CREATE TABLE approval_counters (
    worker_id    TEXT NOT NULL,
    fingerprint  TEXT NOT NULL,
    count        INTEGER NOT NULL,
    PRIMARY KEY (worker_id, fingerprint)
);

CREATE TABLE approval_rules (
    id           TEXT PRIMARY KEY,
    project_id   TEXT NOT NULL REFERENCES projects(id),
    kind         TEXT NOT NULL CHECK (kind IN ('ONCE', 'PERMANENT')),
    class        TEXT NOT NULL,
    executable   TEXT NOT NULL,
    scope_text   TEXT NOT NULL,
    source_approval TEXT,
    created_ms   INTEGER NOT NULL,
    expires_ms   INTEGER,
    revoked_ms   INTEGER
);
CREATE INDEX approval_rules_project ON approval_rules(project_id);
