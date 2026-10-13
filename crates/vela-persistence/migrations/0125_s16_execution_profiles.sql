-- S16: execution profiles (a provider identity as Vela sees it) with capacity cooldowns and the provider-policy-block hold.
CREATE TABLE execution_profiles (
    id                TEXT PRIMARY KEY,
    state             TEXT NOT NULL CHECK (state IN ('AVAILABLE', 'BUSY', 'COOLDOWN', 'AUTH_REQUIRED', 'UNAVAILABLE', 'DISABLED', 'POLICY_BLOCKED')),
    cooldown_until_ms INTEGER,
    attempts          INTEGER NOT NULL DEFAULT 0,
    reason            TEXT NOT NULL DEFAULT '',
    blocked_ms        INTEGER,
    access_ok_ms      INTEGER,
    updated_ms        INTEGER NOT NULL
);
