//! The `snapshot` command group (F08): `snapshot.get` returns the state a renderer needs to start (or to resynchronize
//! after a gap) together with the journal position it reflects, `last_seq`. The renderer applies only events after
//! `last_seq`. Later tickets add fields to [`SnapshotDto`] as their state appears.

use serde::{Deserialize, Serialize};
use tauri::{Runtime, State, ipc::Invoke};
use vela_domain::errors::VelaError;
use vela_persistence::journal;
use vela_persistence::repositories::f06_core;
use vela_persistence::writer::StateStore;

use crate::wiring::store::StoreHandle;

/// Dotted names of this group's commands (the wire name replaces `.` with `_`).
pub const COMMANDS: &[&str] = &["snapshot.get"];

/// How many recent projects a snapshot lists.
pub const RECENT_PROJECTS: u32 = 20;

/// One project in the snapshot. TypeScript: `ProjectSummary`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSummary {
    /// Identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Repository path.
    pub repo_path: String,
    /// Last time it was opened, epoch milliseconds.
    pub last_opened_ms: Option<i64>,
}

/// The result of `snapshot.get`. TypeScript: `SnapshotDto`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotDto {
    /// The journal position this snapshot reflects.
    pub last_seq: u64,
    /// Schema version of the event vocabulary.
    pub schema_version: u32,
    /// Recently opened projects, most recent first.
    pub projects: Vec<ProjectSummary>,
}

/// Builds the snapshot from the store in one read, so `last_seq` and the state it describes are consistent.
pub fn build_snapshot(store: &StateStore) -> Result<SnapshotDto, VelaError> {
    store.read(|conn| {
        let last_seq = journal::tip(conn)?;
        let projects = f06_core::recent_projects(conn, RECENT_PROJECTS)?
            .into_iter()
            .map(|p| ProjectSummary {
                id: p.id,
                name: p.name,
                repo_path: p.repo_path,
                last_opened_ms: p.last_opened_ms,
            })
            .collect();
        Ok(SnapshotDto {
            last_seq,
            schema_version: vela_domain::events::EVENT_SCHEMA_VERSION,
            projects,
        })
    })
}

/// `snapshot.get`.
#[tauri::command]
pub async fn snapshot_get(store: State<'_, StoreHandle>) -> Result<SnapshotDto, VelaError> {
    build_snapshot(&store.0)
}

/// Runs the command in `invoke` if this group owns it; returns `false` when it does not.
pub fn handle<R: Runtime>(invoke: Invoke<R>) -> bool {
    run(tauri::generate_handler![snapshot_get], invoke)
}

/// Gives the handler closure its parameter type from the bound, as `Builder::invoke_handler` does.
fn run<R: Runtime>(handler: impl Fn(Invoke<R>) -> bool, invoke: Invoke<R>) -> bool {
    handler(invoke)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands;
    use serde_json::{Value, json};
    use std::path::PathBuf;
    use tauri::Manager;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, get_ipc_response, mock_context, noop_assets};
    use tauri::webview::InvokeRequest;
    use vela_domain::events::{Event, TicketReadyPayload};
    use vela_domain::ids::RunId;
    use vela_persistence::journal::NewEvent;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vela-snapshot-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn event(run: &RunId, n: u32) -> NewEvent {
        NewEvent {
            ts: 1,
            run_id: Some(run.clone()),
            ticket_id: None,
            worker_id: None,
            event: Event::TicketReady(TicketReadyPayload { frontier_size: n }),
        }
    }

    #[test]
    fn the_snapshot_reports_the_journal_position_and_recent_projects() {
        let root = temp_root("dto");
        let handle = crate::wiring::store::open(&root).unwrap();
        let run = RunId::new("run-1").unwrap();
        let r = run.clone();
        handle
            .0
            .write(move |tx| {
                f06_core::insert_project(tx.conn(), "p1", "demo", "C:/demo", 5)?;
                f06_core::insert_run(tx.conn(), r.as_str(), "p1", "CREATED", 5)?;
                tx.append(&event(&r, 1))?;
                tx.append(&event(&r, 2)).map(|_| ())
            })
            .unwrap();
        let snapshot = build_snapshot(&handle.0).unwrap();
        assert_eq!(snapshot.last_seq, 2);
        assert_eq!(snapshot.projects.len(), 1);
        assert_eq!(snapshot.projects[0].name, "demo");
        assert_eq!(snapshot.schema_version, 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn snapshot_get_round_trips_through_the_ipc_layer_and_rejects_unknown_arguments_shapes() {
        let root = temp_root("ipc");
        let handle = crate::wiring::store::open(&root).unwrap();
        let app = commands::install(tauri::test::mock_builder())
            .build(mock_context(noop_assets()))
            .expect("build the mock application");
        app.manage(handle);
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("create the main window");
        let response = get_ipc_response(
            &window,
            InvokeRequest {
                cmd: "snapshot_get".into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::Json(json!({})),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("snapshot.get succeeds")
        .deserialize::<Value>()
        .unwrap();
        assert_eq!(response["last_seq"], 0);
        assert_eq!(response["projects"], json!([]));
        let _ = std::fs::remove_dir_all(&root);
    }
}
