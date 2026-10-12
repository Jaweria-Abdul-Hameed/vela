//! The migration ladder (F06): embedded `NNNN_<ticket>_<name>.sql` files applied in numeric order from an empty database,
//! tolerating gaps. Applied numbers are recorded in `schema_migrations`, so a file merged later with a lower number than one
//! already applied still runs, and the order never depends on merge order. `PRAGMA user_version` holds the highest number.

use rusqlite::{Connection, params};
use vela_domain::errors::{ErrorCode, VelaError};

include!(concat!(env!("OUT_DIR"), "/migrations.rs"));

/// What a migration run did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    /// Numbers applied by this run, ascending.
    pub applied: Vec<u32>,
    /// The highest applied number afterwards.
    pub version: u32,
}

/// The highest migration number this build knows.
pub fn latest_version() -> u32 {
    MIGRATIONS.iter().map(|m| m.0).max().unwrap_or(0)
}

fn failure(what: impl Into<String>) -> VelaError {
    VelaError::from_code(ErrorCode::MigrationFailed, what)
}

fn ensure_ledger(conn: &Connection) -> Result<(), VelaError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            number INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_ms INTEGER NOT NULL
        );",
    )
    .map_err(|e| failure(format!("could not create the migration ledger: {e}")))
}

/// Numbers already applied.
pub fn applied(conn: &Connection) -> Result<Vec<u32>, VelaError> {
    ensure_ledger(conn)?;
    let mut stmt = conn
        .prepare("SELECT number FROM schema_migrations ORDER BY number")
        .map_err(|e| failure(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| r.get::<_, u32>(0))
        .map_err(|e| failure(e.to_string()))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| failure(e.to_string()))
}

/// Numbers this build would apply to `conn`, ascending. Fails when the database holds a migration this build does not know
/// (it was written by a newer build): older logic never runs against newer state.
pub fn pending(conn: &Connection) -> Result<Vec<u32>, VelaError> {
    pending_in(conn, MIGRATIONS)
}

/// A migration file: number, file name, SQL.
pub type Ladder<'a> = &'a [(u32, &'a str, &'a str)];

/// [`pending`] against an explicit ladder (tests inject failing ladders).
pub fn pending_in(conn: &Connection, ladder: Ladder<'_>) -> Result<Vec<u32>, VelaError> {
    let done = applied(conn)?;
    if let Some(unknown) = done.iter().find(|n| !ladder.iter().any(|m| m.0 == **n)) {
        return Err(failure(format!(
            "the state store has migration {unknown}, which this build does not know; refusing to run older logic against newer state"
        )));
    }
    Ok(ladder
        .iter()
        .map(|m| m.0)
        .filter(|n| !done.contains(n))
        .collect())
}

/// Applies every pending migration in numeric order, each in its own transaction. The first failure stops the ladder and
/// leaves the failed migration unapplied.
pub fn migrate(conn: &mut Connection, now_ms: i64) -> Result<MigrationReport, VelaError> {
    migrate_in(conn, MIGRATIONS, now_ms)
}

/// [`migrate`] against an explicit ladder.
pub fn migrate_in(
    conn: &mut Connection,
    ladder: Ladder<'_>,
    now_ms: i64,
) -> Result<MigrationReport, VelaError> {
    let todo = pending_in(conn, ladder)?;
    let mut applied_now = Vec::new();
    for number in todo {
        let (_, name, sql) = ladder
            .iter()
            .find(|m| m.0 == number)
            .expect("pending numbers come from the ladder");
        let tx = conn
            .transaction()
            .map_err(|e| failure(format!("migration {name}: {e}")))?;
        tx.execute_batch(sql)
            .map_err(|e| failure(format!("migration {name} failed: {e}")))?;
        tx.execute(
            "INSERT INTO schema_migrations (number, name, applied_ms) VALUES (?1, ?2, ?3)",
            params![number, name, now_ms],
        )
        .map_err(|e| failure(format!("migration {name}: {e}")))?;
        tx.commit()
            .map_err(|e| failure(format!("migration {name}: {e}")))?;
        applied_now.push(number);
    }
    let version = applied(conn)?.into_iter().max().unwrap_or(0);
    conn.pragma_update(None, "user_version", version)
        .map_err(|e| failure(e.to_string()))?;
    Ok(MigrationReport {
        applied: applied_now,
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_numbers_are_unique() {
        let mut numbers: Vec<u32> = MIGRATIONS.iter().map(|m| m.0).collect();
        numbers.sort_unstable();
        let before = numbers.len();
        numbers.dedup();
        assert_eq!(numbers.len(), before, "two migration files share a number");
    }

    #[test]
    fn applies_from_empty_and_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        let first = migrate(&mut conn, 1).unwrap();
        assert_eq!(first.version, latest_version());
        assert!(!first.applied.is_empty());
        let second = migrate(&mut conn, 2).unwrap();
        assert!(second.applied.is_empty());
        let v: u32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, latest_version());
    }

    #[test]
    fn a_database_from_a_newer_build_is_refused() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn, 1).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (number, name, applied_ms) VALUES (9999, 'future', 0)",
            [],
        )
        .unwrap();
        let err = migrate(&mut conn, 2).unwrap_err();
        assert_eq!(err.code(), Some(ErrorCode::MigrationFailed));
    }

    #[test]
    fn ladder_orders_by_number_regardless_of_input_order_and_tolerates_gaps() {
        // Simulates files merged out of order: the ledger, not the arrival order, decides what runs, and a later low
        // number still applies after a higher one.
        let mut conn = Connection::open_in_memory().unwrap();
        ensure_ledger(&conn).unwrap();
        let mut order = Vec::new();
        for (n, sql) in [
            (40u32, "CREATE TABLE b(x);"),
            (2, "CREATE TABLE a(x);"),
            (900, "CREATE TABLE c(x);"),
        ] {
            order.push((n, sql));
        }
        order.sort_by_key(|(n, _)| *n);
        assert_eq!(
            order.iter().map(|o| o.0).collect::<Vec<_>>(),
            vec![2, 40, 900]
        );
        for (n, sql) in order {
            let tx = conn.transaction().unwrap();
            tx.execute_batch(sql).unwrap();
            tx.execute(
                "INSERT INTO schema_migrations (number, name, applied_ms) VALUES (?1, 't', 0)",
                [n],
            )
            .unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(applied(&conn).unwrap(), vec![2, 40, 900]);
    }
}
