-- S03: repository trust. A repository is untrusted until the user trusts it; the command profile is what the user confirmed.
CREATE TABLE project_trust (
    project_id           TEXT PRIMARY KEY REFERENCES projects(id),
    state                TEXT NOT NULL CHECK (state IN ('UNTRUSTED', 'TRUSTED')),
    repository_identity  TEXT NOT NULL,
    consent_text_version TEXT,
    command_profile      TEXT NOT NULL DEFAULT '[]',
    changed_ms           INTEGER NOT NULL
);
