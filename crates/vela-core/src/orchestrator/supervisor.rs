//! Worker task supervision: a worker runs as its own task, and if it fails or panics only that worker fails (state `FAILED`,
//! `WorkerFailed` journaled). The Orchestrator and every other worker carry on.

use std::future::Future;
use std::sync::Arc;

use tokio::task::JoinHandle;
use vela_domain::errors::VelaError;
use vela_domain::events::{Event, WorkerFailedPayload};
use vela_domain::ids::{RunId, WorkerId};
use vela_domain::state::{Cause, IntegrationHealth, WorkerState};

use super::Services;
use super::runs::Runs;

/// A task that is aborted when this guard is dropped. A worker's task must not outlive whoever is waiting for it: when the
/// driver is cancelled (Stop All, shutdown) the worker stops with it instead of carrying on detached and changing state under the
/// feet of the recovery that follows.
pub struct AbortOnDrop<T>(pub JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Runs `work` as the worker's task. The returned handle finishes after the outcome (including a failure) was recorded.
pub fn supervise<F>(
    services: Arc<Services>,
    run: RunId,
    worker: WorkerId,
    work: F,
) -> JoinHandle<()>
where
    F: Future<Output = Result<(), VelaError>> + Send + 'static,
{
    tokio::spawn(async move {
        let mut inner = AbortOnDrop(tokio::spawn(work));
        let failure = match (&mut inner.0).await {
            Ok(Ok(())) => return,
            Ok(Err(error)) => error,
            Err(join) if join.is_panic() => {
                VelaError::internal("the worker task panicked; only this worker failed")
            }
            Err(_) => VelaError::internal("the worker task was cancelled"),
        };
        let runs = Runs::new(&services);
        let recorded = runs
            .transition_worker(
                &run,
                &worker,
                WorkerState::Failed,
                Cause::System,
                IntegrationHealth::Healthy,
                vec![Event::WorkerFailed(WorkerFailedPayload { error: failure })],
            )
            .await;
        if let Err(e) = recorded {
            tracing::error!(worker = %worker, "could not record a worker failure: {e}");
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn a_task_behind_the_guard_stops_when_the_guard_is_dropped() {
        let finished = Arc::new(AtomicBool::new(false));
        let flag = finished.clone();
        let guard = AbortOnDrop(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            flag.store(true, Ordering::SeqCst);
        }));
        tokio::time::sleep(Duration::from_millis(20)).await;
        drop(guard);
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            !finished.load(Ordering::SeqCst),
            "the task kept running detached"
        );
    }

    #[tokio::test]
    async fn cancelling_the_waiter_cancels_a_supervised_worker_and_records_no_failure() {
        let dir = std::env::temp_dir().join(format!("vela-supervise-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Arc::new(
            vela_persistence::writer::StateStore::open(
                &dir.join("s.db"),
                vela_persistence::writer::StoreOptions::default(),
            )
            .unwrap(),
        );
        let services = Services::new(
            store,
            Arc::new(super::super::system::SystemClock),
            Arc::new(super::super::system::SystemIds::new()),
        );
        let ran = Arc::new(AtomicBool::new(false));
        let flag = ran.clone();
        let handle = supervise(
            services,
            RunId::new("run-1").unwrap(),
            WorkerId::new("w-1").unwrap(),
            async move {
                tokio::time::sleep(Duration::from_millis(300)).await;
                flag.store(true, Ordering::SeqCst);
                Ok(())
            },
        );
        let waiter = tokio::spawn(async move {
            let mut guard = AbortOnDrop(handle);
            let _ = (&mut guard.0).await;
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        waiter.abort();
        let _ = waiter.await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            !ran.load(Ordering::SeqCst),
            "the worker outlived its waiter"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
