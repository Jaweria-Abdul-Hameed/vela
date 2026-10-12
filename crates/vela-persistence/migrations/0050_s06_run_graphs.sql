-- S06: the validated dependency graph of a run, with its layout, pair verdicts and the analyst's inputs.
CREATE TABLE run_graphs (
    run_id          TEXT PRIMARY KEY REFERENCES build_runs(id),
    snapshot        TEXT NOT NULL,
    layout          TEXT NOT NULL,
    verdicts        TEXT NOT NULL DEFAULT '[]',
    analyst         TEXT NOT NULL DEFAULT '{}',
    warnings        TEXT NOT NULL DEFAULT '[]',
    analysis_state  TEXT NOT NULL CHECK (analysis_state IN ('ANALYZED', 'SEQUENTIAL_FALLBACK')),
    approved_ms     INTEGER,
    last_cycle      TEXT NOT NULL DEFAULT '{}',
    created_ms      INTEGER NOT NULL
);
