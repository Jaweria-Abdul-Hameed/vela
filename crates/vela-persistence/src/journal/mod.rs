//! The append-only event journal (F06). `seq` is assigned by the store (a strictly increasing `AUTOINCREMENT`), never by the
//! producer, and every payload passes through the injected filter (the central redactor) before it is written.

use std::sync::Arc;

use rusqlite::{Connection, params};
use vela_domain::errors::VelaError;
use vela_domain::events::{Event, EventEnvelope, SchemaVersion};
use vela_domain::ids::{RunId, TicketId, WorkerId};

use crate::writer::store_error;

/// Redacts a serialized payload before it is journaled. Must keep the JSON valid.
pub type PayloadFilter = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// A filter that changes nothing (tests).
pub fn identity_filter() -> PayloadFilter {
    Arc::new(|s: &str| s.to_owned())
}

/// An event waiting to be journaled.
#[derive(Debug, Clone, PartialEq)]
pub struct NewEvent {
    /// UTC epoch milliseconds.
    pub ts: i64,
    /// The run it belongs to, if any.
    pub run_id: Option<RunId>,
    /// The ticket it concerns, if any.
    pub ticket_id: Option<TicketId>,
    /// The worker it concerns, if any.
    pub worker_id: Option<WorkerId>,
    /// Kind and payload.
    pub event: Event,
}

/// Appends `event` inside the caller's transaction and returns the envelope with its assigned `seq`. The owning run row (if
/// any) gets `last_event_seq` in the same transaction.
pub fn append(
    conn: &Connection,
    event: &NewEvent,
    filter: &PayloadFilter,
) -> Result<EventEnvelope, VelaError> {
    let value = serde_json::to_value(&event.event).map_err(store_error)?;
    let payload_json =
        serde_json::to_string(value.get("payload").unwrap_or(&serde_json::Value::Null))
            .map_err(store_error)?;
    let payload_json = filter(&payload_json);
    let payload_value: serde_json::Value =
        serde_json::from_str(&payload_json).map_err(store_error)?;
    let stored: Event = serde_json::from_value(serde_json::json!({
        "kind": event.event.kind().as_str(),
        "payload": payload_value,
    }))
    .map_err(store_error)?;
    let version = SchemaVersion::current();
    conn.execute(
        "INSERT INTO event_journal (ts, run_id, ticket_id, worker_id, kind, payload, schema_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            event.ts,
            event.run_id.as_ref().map(RunId::as_str),
            event.ticket_id.as_ref().map(TicketId::as_str),
            event.worker_id.as_ref().map(WorkerId::as_str),
            stored.kind().as_str(),
            payload_json,
            version.get(),
        ],
    )
    .map_err(store_error)?;
    let seq = u64::try_from(conn.last_insert_rowid()).unwrap_or(0);
    if let Some(run) = &event.run_id {
        conn.execute(
            "UPDATE build_runs SET last_event_seq = ?1 WHERE id = ?2",
            params![i64::try_from(seq).unwrap_or(i64::MAX), run.as_str()],
        )
        .map_err(store_error)?;
    }
    Ok(EventEnvelope {
        seq,
        ts: event.ts,
        run_id: event.run_id.clone(),
        ticket_id: event.ticket_id.clone(),
        worker_id: event.worker_id.clone(),
        event: stored,
        schema_version: version,
    })
}

/// The highest assigned `seq` (0 for an empty journal).
pub fn tip(conn: &Connection) -> Result<u64, VelaError> {
    conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM event_journal", [], |r| {
        r.get::<_, i64>(0)
    })
    .map(|n| u64::try_from(n).unwrap_or(0))
    .map_err(store_error)
}

/// Events with `seq > after_seq`, ascending, at most `limit`.
pub fn read_after(
    conn: &Connection,
    after_seq: u64,
    limit: u32,
) -> Result<Vec<EventEnvelope>, VelaError> {
    let mut stmt = conn
        .prepare(
            "SELECT seq, ts, run_id, ticket_id, worker_id, kind, payload, schema_version
             FROM event_journal WHERE seq > ?1 ORDER BY seq LIMIT ?2",
        )
        .map_err(store_error)?;
    let rows = stmt
        .query_map(
            params![i64::try_from(after_seq).unwrap_or(i64::MAX), limit],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, u32>(7)?,
                ))
            },
        )
        .map_err(store_error)?;
    let mut out = Vec::new();
    for row in rows {
        let (seq, ts, run, ticket, worker, kind, payload, version) = row.map_err(store_error)?;
        let payload: serde_json::Value = serde_json::from_str(&payload).map_err(store_error)?;
        let event: Event =
            serde_json::from_value(serde_json::json!({ "kind": kind, "payload": payload }))
                .map_err(store_error)?;
        out.push(EventEnvelope {
            seq: u64::try_from(seq).unwrap_or(0),
            ts,
            run_id: run.map(RunId::new).transpose().map_err(store_error)?,
            ticket_id: ticket.map(TicketId::new).transpose().map_err(store_error)?,
            worker_id: worker.map(WorkerId::new).transpose().map_err(store_error)?,
            event,
            schema_version: SchemaVersion::new(version).map_err(store_error)?,
        });
    }
    Ok(out)
}
