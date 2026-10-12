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
        let failure = match tokio::spawn(work).await {
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
