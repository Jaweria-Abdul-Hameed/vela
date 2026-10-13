//! Repository for approvals (K01, K02, K09).

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// A stored approval request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRow {
    /// Identifier.
    pub id: String,
    /// The run.
    pub run_id: String,
    /// The worker.
    pub worker_id: String,
    /// Where it was observed.
    pub source: String,
    /// `WEAK`, `OBSERVED` or `BOUND`.
    pub evidence_tier: String,
    /// The operation class.
    pub class: String,
    /// The executable's lookup name.
    pub executable: String,
    /// The secret-free fingerprint.
    pub fingerprint: String,
    /// `PENDING`, `ALLOWED`, `DENIED`, `ASKED`, `STALLED` or `UNDELIVERABLE`.
    pub state: String,
    /// `ALLOW`, `ASK` or `DENY` from the policy.
    pub decision: Option<String>,
    /// The rule that decided.
    pub rule_id: Option<String>,
    /// Why.
    pub reason: String,
    /// When it arrived.
    pub created_ms: i64,
    /// When it was decided.
    pub decided_ms: Option<i64>,
}

const COLS: &str = "id, run_id, worker_id, source, evidence_tier, class, executable, fingerprint, state, decision, rule_id, reason, created_ms, decided_ms";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ApprovalRow> {
    Ok(ApprovalRow {
        id: r.get(0)?,
        run_id: r.get(1)?,
        worker_id: r.get(2)?,
        source: r.get(3)?,
        evidence_tier: r.get(4)?,
        class: r.get(5)?,
        executable: r.get(6)?,
        fingerprint: r.get(7)?,
        state: r.get(8)?,
        decision: r.get(9)?,
        rule_id: r.get(10)?,
        reason: r.get(11)?,
        created_ms: r.get(12)?,
        decided_ms: r.get(13)?,
    })
}

/// Stores a new (pending) request.
pub fn insert_request(conn: &Connection, a: &ApprovalRow) -> Result<(), VelaError> {
    conn.execute(
        &format!("INSERT INTO approval_requests ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)"),
        params![
            a.id, a.run_id, a.worker_id, a.source, a.evidence_tier, a.class, a.executable, a.fingerprint, a.state,
            a.decision, a.rule_id, a.reason, a.created_ms, a.decided_ms
        ],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// Records the decision on a request.
pub fn set_decision(
    conn: &Connection,
    id: &str,
    state: &str,
    decision: &str,
    rule_id: Option<&str>,
    reason: &str,
    now_ms: i64,
) -> Result<(), VelaError> {
    conn.execute(
        "UPDATE approval_requests SET state = ?2, decision = ?3, rule_id = ?4, reason = ?5, decided_ms = ?6 WHERE id = ?1",
        params![id, state, decision, rule_id, reason, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// Sets only the state (for example after a failed delivery).
pub fn set_state(conn: &Connection, id: &str, state: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "UPDATE approval_requests SET state = ?2, decided_ms = ?3 WHERE id = ?1",
        params![id, state, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// One request.
pub fn get_request(conn: &Connection, id: &str) -> Result<Option<ApprovalRow>, VelaError> {
    conn.query_row(
        &format!("SELECT {COLS} FROM approval_requests WHERE id = ?1"),
        [id],
        row,
    )
    .optional()
    .map_err(store_error)
}

/// A worker's requests, oldest first.
pub fn list_for_worker(conn: &Connection, worker_id: &str) -> Result<Vec<ApprovalRow>, VelaError> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {COLS} FROM approval_requests WHERE worker_id = ?1 ORDER BY created_ms, id"
        ))
        .map_err(store_error)?;
    let rows = stmt.query_map([worker_id], row).map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}

/// Records a delivery attempt and returns its attempt number for the request.
pub fn insert_delivery(
    conn: &Connection,
    id: &str,
    approval_id: &str,
    adapter: &str,
    verified: bool,
    detail: &str,
    now_ms: i64,
) -> Result<u32, VelaError> {
    let attempt: u32 = conn
        .query_row(
            "SELECT COUNT(*) + 1 FROM approval_deliveries WHERE approval_id = ?1",
            [approval_id],
            |r| r.get(0),
        )
        .map_err(store_error)?;
    conn.execute(
        "INSERT INTO approval_deliveries (id, approval_id, adapter, attempt, verified, detail, ts_ms) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![id, approval_id, adapter, attempt, i64::from(verified), detail, now_ms],
    )
    .map_err(store_error)?;
    Ok(attempt)
}

/// How many delivery attempts a request has had.
pub fn delivery_count(conn: &Connection, approval_id: &str) -> Result<u32, VelaError> {
    conn.query_row(
        "SELECT COUNT(*) FROM approval_deliveries WHERE approval_id = ?1",
        [approval_id],
        |r| r.get(0),
    )
    .map_err(store_error)
}

/// Adds one to the repeat counter of `(worker, fingerprint)` and returns the new count.
pub fn bump_counter(conn: &Connection, worker_id: &str, fingerprint: &str) -> Result<u32, VelaError> {
    conn.execute(
        "INSERT INTO approval_counters (worker_id, fingerprint, count) VALUES (?1, ?2, 1)
         ON CONFLICT(worker_id, fingerprint) DO UPDATE SET count = count + 1",
        params![worker_id, fingerprint],
    )
    .map_err(store_error)?;
    conn.query_row(
        "SELECT count FROM approval_counters WHERE worker_id = ?1 AND fingerprint = ?2",
        params![worker_id, fingerprint],
        |r| r.get(0),
    )
    .map_err(store_error)
}

/// Clears a worker's repeat counters (progress was made).
pub fn clear_counters(conn: &Connection, worker_id: &str) -> Result<(), VelaError> {
    conn.execute("DELETE FROM approval_counters WHERE worker_id = ?1", [worker_id])
        .map(|_| ())
        .map_err(store_error)
}

/// A scoped allow rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleRow {
    /// Identifier.
    pub id: String,
    /// The project it applies to.
    pub project_id: String,
    /// `ONCE` or `PERMANENT`.
    pub kind: String,
    /// The operation class it covers.
    pub class: String,
    /// The executable it covers.
    pub executable: String,
    /// The scope text the user saw.
    pub scope_text: String,
    /// The approval it was created from.
    pub source_approval: Option<String>,
    /// When it was created.
    pub created_ms: i64,
    /// When it stops applying.
    pub expires_ms: Option<i64>,
    /// When it was revoked.
    pub revoked_ms: Option<i64>,
}

const RULE_COLS: &str = "id, project_id, kind, class, executable, scope_text, source_approval, created_ms, expires_ms, revoked_ms";

fn rule_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<RuleRow> {
    Ok(RuleRow {
        id: r.get(0)?,
        project_id: r.get(1)?,
        kind: r.get(2)?,
        class: r.get(3)?,
        executable: r.get(4)?,
        scope_text: r.get(5)?,
        source_approval: r.get(6)?,
        created_ms: r.get(7)?,
        expires_ms: r.get(8)?,
        revoked_ms: r.get(9)?,
    })
}

/// Stores a rule.
pub fn insert_rule(conn: &Connection, r: &RuleRow) -> Result<(), VelaError> {
    conn.execute(
        &format!("INSERT INTO approval_rules ({RULE_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)"),
        params![
            r.id, r.project_id, r.kind, r.class, r.executable, r.scope_text, r.source_approval, r.created_ms,
            r.expires_ms, r.revoked_ms
        ],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// The rules of a project that are in force at `now_ms`: not revoked and not expired.
pub fn active_rules(conn: &Connection, project_id: &str, now_ms: i64) -> Result<Vec<RuleRow>, VelaError> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {RULE_COLS} FROM approval_rules WHERE project_id = ?1 AND revoked_ms IS NULL
             AND (expires_ms IS NULL OR expires_ms > ?2) ORDER BY created_ms, id"
        ))
        .map_err(store_error)?;
    let rows = stmt.query_map(params![project_id, now_ms], rule_row).map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}

/// One rule.
pub fn get_rule(conn: &Connection, id: &str) -> Result<Option<RuleRow>, VelaError> {
    conn.query_row(&format!("SELECT {RULE_COLS} FROM approval_rules WHERE id = ?1"), [id], rule_row)
        .optional()
        .map_err(store_error)
}

/// Revokes a rule. False when it does not exist or was already revoked.
pub fn revoke_rule(conn: &Connection, id: &str, now_ms: i64) -> Result<bool, VelaError> {
    conn.execute(
        "UPDATE approval_rules SET revoked_ms = ?2 WHERE id = ?1 AND revoked_ms IS NULL",
        params![id, now_ms],
    )
    .map(|n| n == 1)
    .map_err(store_error)
}
