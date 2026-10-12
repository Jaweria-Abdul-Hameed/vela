//! Shared test environment: a real SQLite store in a temp folder, a fake clock and fake identifiers.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use vela_domain::ids::{BranchName, CommitSha, RunId};
use vela_persistence::repositories::f06_core;
use vela_persistence::writer::{StateStore, StoreOptions};
use vela_testkit::fakes::{FakeClock, FakeIds};

use super::Services;
use super::runs::Runs;

/// A throwaway store and the services over it.
pub struct TestEnv {
    /// The services.
    pub services: Arc<Services>,
    /// The clock the services use.
    pub clock: Arc<FakeClock>,
    /// The database file.
    pub db: PathBuf,
    dir: PathBuf,
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn unique_dir(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "vela-core-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An environment with the default store options.
pub fn env(tag: &str) -> TestEnv {
    env_with(tag, StoreOptions::default())
}

/// An environment with explicit store options (for example a payload filter).
pub fn env_with(tag: &str, options: StoreOptions) -> TestEnv {
    let dir = unique_dir(tag);
    let db = dir.join("state.db");
    let store = Arc::new(StateStore::open(&db, options).unwrap());
    let clock = Arc::new(FakeClock::new(1_000_000));
    let services = Services::new(store, clock.clone(), Arc::new(FakeIds::new()));
    TestEnv {
        services,
        clock,
        db,
        dir,
    }
}

impl TestEnv {
    /// Reopens the same database with fresh services (a restart).
    pub fn restart(&self) -> Arc<Services> {
        let store = Arc::new(StateStore::open(&self.db, StoreOptions::default()).unwrap());
        Services::new(store, self.clock.clone(), Arc::new(FakeIds::new()))
    }

    /// Inserts project `p1`.
    pub fn seed_project(&self) {
        self.services
            .store
            .write(|tx| f06_core::insert_project(tx.conn(), "p1", "demo", "C:/demo", 1))
            .unwrap();
    }

    /// Creates a run `run-1` in `CREATED` for project `p1`.
    pub async fn create_run(&self) -> RunId {
        self.seed_project();
        let run = RunId::new("run-1").unwrap();
        Runs::new(&self.services)
            .create_run(
                "p1",
                run.clone(),
                CommitSha::new("a".repeat(40)).unwrap(),
                BranchName::new("vela/run-1/integration").unwrap(),
                "all tickets",
            )
            .await
            .unwrap()
    }

    /// The raw journal payload column of every row, oldest first.
    pub fn raw_journal(&self) -> Vec<(String, String)> {
        self.services
            .store
            .read(|c| {
                let mut s = c
                    .prepare("SELECT kind, payload FROM event_journal ORDER BY seq")
                    .unwrap();
                Ok(s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                    .unwrap()
                    .map(Result::unwrap)
                    .collect())
            })
            .unwrap()
    }
}

impl TestEnv {
    /// Drives a run through the forward phases up to `target` (inclusive), each as a system transition.
    pub async fn drive_run(&self, run: &RunId, target: vela_domain::state::RunPhase) {
        use vela_domain::state::{Cause, IntegrationHealth, RunPhase, RunState};
        let order = [
            RunPhase::Preflight,
            RunPhase::Analyzing,
            RunPhase::GraphReady,
            RunPhase::AwaitingStart,
            RunPhase::Starting,
            RunPhase::Running,
        ];
        let runs = Runs::new(&self.services);
        for phase in order {
            runs.transition_run(
                run,
                RunState::Active(phase),
                Cause::System,
                IntegrationHealth::Healthy,
                Vec::new(),
            )
            .await
            .unwrap();
            if phase == target {
                return;
            }
        }
    }
}
