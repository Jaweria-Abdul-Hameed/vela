//! Startup integrity checks (F06). A mismatch between materialized state and the journal is reported and **never silently
//! repaired**: the caller decides (the rebuild path belongs to R05).

use rusqlite::Connection;
use vela_domain::errors::{ErrorCode, VelaError};

use crate::journal;
use crate::writer::store_error;

/// One thing found wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityIssue {
    /// SQLite's own `PRAGMA integrity_check` reported a problem.
    Sqlite(String),
    /// A foreign key points at a missing row.
    ForeignKey(String),
    /// A run claims an event the journal does not have.
    RunAheadOfJournal {
        /// The run.
        run_id: String,
        /// What the row says.
        last_event_seq: u64,
        /// The journal's tip.
        journal_tip: u64,
    },
    /// A run's `last_event_seq` names an event that belongs to a different run.
    RunPointsAtForeignEvent {
        /// The run.
        run_id: String,
        /// The sequence it names.
        last_event_seq: u64,
    },
    /// The journal has holes (count differs from the highest `seq`).
    JournalGap {
        /// Rows present.
        rows: u64,
        /// Highest `seq`.
        tip: u64,
    },
}

/// Runs every check and returns what it found (empty means healthy).
pub fn check(conn: &Connection) -> Result<Vec<IntegrityIssue>, VelaError> {
    let mut issues = Vec::new();

    let mut stmt = conn
        .prepare("PRAGMA integrity_check")
        .map_err(store_error)?;
    let lines: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(store_error)?
        .collect::<Result<_, _>>()
        .map_err(store_error)?;
    issues.extend(
        lines
            .into_iter()
            .filter(|l| l != "ok")
            .map(IntegrityIssue::Sqlite),
    );

    let mut stmt = conn
        .prepare("PRAGMA foreign_key_check")
        .map_err(store_error)?;
    let fks: Vec<String> = stmt
        .query_map([], |r| {
            Ok(format!(
                "{} row {:?} -> {}",
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, String>(2)?
            ))
        })
        .map_err(store_error)?
        .collect::<Result<_, _>>()
        .map_err(store_error)?;
    issues.extend(fks.into_iter().map(IntegrityIssue::ForeignKey));

    let tip = journal::tip(conn)?;
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM event_journal", [], |r| r.get(0))
        .map_err(store_error)?;
    let rows = u64::try_from(rows).unwrap_or(0);
    if rows != tip {
        issues.push(IntegrityIssue::JournalGap { rows, tip });
    }

    let mut stmt = conn
        .prepare("SELECT id, last_event_seq FROM build_runs WHERE last_event_seq > 0")
        .map_err(store_error)?;
    let runs: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(store_error)?
        .collect::<Result<_, _>>()
        .map_err(store_error)?;
    for (run_id, seq) in runs {
        let seq = u64::try_from(seq).unwrap_or(0);
        if seq > tip {
            issues.push(IntegrityIssue::RunAheadOfJournal {
                run_id,
                last_event_seq: seq,
                journal_tip: tip,
            });
            continue;
        }
        let owner: Option<Option<String>> = conn
            .query_row(
                "SELECT run_id FROM event_journal WHERE seq = ?1",
                [i64::try_from(seq).unwrap_or(i64::MAX)],
                |r| r.get(0),
            )
            .ok();
        if owner.flatten().as_deref() != Some(run_id.as_str()) {
            issues.push(IntegrityIssue::RunPointsAtForeignEvent {
                run_id,
                last_event_seq: seq,
            });
        }
    }
    Ok(issues)
}

/// Turns a non-empty issue list into the typed `StateStoreCorrupt` error (the store is never repaired here).
pub fn require_healthy(conn: &Connection) -> Result<(), VelaError> {
    let issues = check(conn)?;
    if issues.is_empty() {
        Ok(())
    } else {
        Err(VelaError::from_code(
            ErrorCode::StateStoreCorrupt,
            format!("the state store failed its integrity check: {issues:?}"),
        ))
    }
}
