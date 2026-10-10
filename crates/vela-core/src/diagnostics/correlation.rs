//! Correlation ids: run, ticket, worker, operation (`OBSERVABILITY.md` "Logs").
//!
//! The ids are fields of a `tracing` span. A record emitted while the span is entered carries them, whichever crate
//! emits it, so adapters and the process runner need no knowledge of runs or workers. A future keeps its span across
//! `.await` points when it is wrapped with [`CorrelationIds::scope`]; a task spawned with [`spawn_correlated`] inherits
//! the span of the task that spawned it.

use tokio::task::JoinHandle;
use tracing::instrument::Instrumented;
use tracing::{Instrument, Level, Span, field};
use vela_domain::ids::{OperationId, RunId, TicketId, WorkerId};

/// The name of the span that carries correlation ids.
pub const CORRELATION_SPAN: &str = "correlation";
/// The span field holding the run id.
pub const RUN_ID_FIELD: &str = "run_id";
/// The span field holding the ticket id.
pub const TICKET_ID_FIELD: &str = "ticket_id";
/// The span field holding the worker id.
pub const WORKER_ID_FIELD: &str = "worker_id";
/// The span field holding the operation id.
pub const OPERATION_ID_FIELD: &str = "operation_id";

/// The identifiers that tie a log record, a process and a journal entry to the work they belong to.
///
/// Every id is optional: a run-level record has only a run, a worker's command has all four. Scopes nest, so a worker
/// scope opened inside a run scope yields records with both.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CorrelationIds {
    /// The build run.
    pub run: Option<RunId>,
    /// The ticket being worked on.
    pub ticket: Option<TicketId>,
    /// The worker doing the work.
    pub worker: Option<WorkerId>,
    /// The operation (a non-idempotent external action) in progress.
    pub operation: Option<OperationId>,
}

impl CorrelationIds {
    /// No ids.
    pub fn new() -> Self {
        Self::default()
    }

    /// The ids of run-level work.
    pub fn for_run(run: RunId) -> Self {
        Self::new().with_run(run)
    }

    /// Sets the run.
    #[must_use]
    pub fn with_run(mut self, run: RunId) -> Self {
        self.run = Some(run);
        self
    }

    /// Sets the ticket.
    #[must_use]
    pub fn with_ticket(mut self, ticket: TicketId) -> Self {
        self.ticket = Some(ticket);
        self
    }

    /// Sets the worker.
    #[must_use]
    pub fn with_worker(mut self, worker: WorkerId) -> Self {
        self.worker = Some(worker);
        self
    }

    /// Sets the operation.
    #[must_use]
    pub fn with_operation(mut self, operation: OperationId) -> Self {
        self.operation = Some(operation);
        self
    }

    /// Whether no id is set.
    pub fn is_empty(&self) -> bool {
        self.run.is_none()
            && self.ticket.is_none()
            && self.worker.is_none()
            && self.operation.is_none()
    }

    /// A span carrying these ids, as a child of the current span.
    ///
    /// The span has the highest level, so no level filter can remove the ids from the records inside it.
    pub fn span(&self) -> Span {
        let span = tracing::span!(
            Level::ERROR,
            "correlation",
            run_id = field::Empty,
            ticket_id = field::Empty,
            worker_id = field::Empty,
            operation_id = field::Empty
        );
        if let Some(run) = &self.run {
            span.record(RUN_ID_FIELD, run.as_str());
        }
        if let Some(ticket) = &self.ticket {
            span.record(TICKET_ID_FIELD, ticket.as_str());
        }
        if let Some(worker) = &self.worker {
            span.record(WORKER_ID_FIELD, worker.as_str());
        }
        if let Some(operation) = &self.operation {
            span.record(OPERATION_ID_FIELD, operation.as_str());
        }
        span
    }

    /// Runs `future` with these ids attached to every record it emits, across all of its `.await` points.
    pub fn scope<F: Future>(&self, future: F) -> Instrumented<F> {
        future.instrument(self.span())
    }

    /// Runs `work` with these ids attached to every record it emits.
    pub fn in_scope<T>(&self, work: impl FnOnce() -> T) -> T {
        self.span().in_scope(work)
    }
}

/// Spawns `future` on the runtime with the correlation ids of the task that spawns it.
///
/// A plain `tokio::spawn` starts with no span, so records of the new task would lose their ids.
pub fn spawn_correlated<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(future.in_current_span())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::fmt;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    use tracing::field::{Field, Visit};
    use tracing::{Event, Metadata, Subscriber, span};

    use super::*;

    type Fields = BTreeMap<String, String>;

    struct SpanData {
        parent: Option<u64>,
        fields: Fields,
        metadata: &'static Metadata<'static>,
    }

    #[derive(Default)]
    struct Recorded {
        spans: BTreeMap<u64, SpanData>,
        events: Vec<Fields>,
    }

    /// A minimal subscriber: it keeps the fields of every span and, for every event, the event's own fields merged
    /// with those of the spans it was emitted in (outermost first, as the JSON log layer lists them).
    #[derive(Clone, Default)]
    struct Capture {
        next_id: Arc<AtomicU64>,
        recorded: Arc<Mutex<Recorded>>,
    }

    thread_local! {
        /// The spans entered on this thread, innermost last.
        static ENTERED: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
    }

    struct Collect<'a>(&'a mut Fields);

    impl Visit for Collect<'_> {
        fn record_str(&mut self, field: &Field, value: &str) {
            self.0.insert(field.name().to_owned(), value.to_owned());
        }

        fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
            self.0.insert(field.name().to_owned(), format!("{value:?}"));
        }
    }

    impl Capture {
        fn events(&self) -> Vec<Fields> {
            self.recorded.lock().unwrap().events.clone()
        }

        /// The fields of the event whose `step` field is `step`.
        fn event(&self, step: &str) -> Fields {
            self.events()
                .into_iter()
                .find(|event| event.get("step").map(String::as_str) == Some(step))
                .unwrap_or_else(|| panic!("no event for step {step}"))
        }
    }

    impl Subscriber for Capture {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }

        fn new_span(&self, attributes: &span::Attributes<'_>) -> span::Id {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
            let parent = if attributes.is_contextual() {
                ENTERED.with(|entered| entered.borrow().last().copied())
            } else {
                attributes.parent().map(span::Id::into_u64)
            };
            let mut fields = Fields::new();
            attributes.record(&mut Collect(&mut fields));
            let data = SpanData {
                parent,
                fields,
                metadata: attributes.metadata(),
            };
            self.recorded.lock().unwrap().spans.insert(id, data);
            span::Id::from_u64(id)
        }

        fn record(&self, span: &span::Id, values: &span::Record<'_>) {
            if let Some(data) = self
                .recorded
                .lock()
                .unwrap()
                .spans
                .get_mut(&span.into_u64())
            {
                values.record(&mut Collect(&mut data.fields));
            }
        }

        fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}

        fn event(&self, event: &Event<'_>) {
            let mut recorded = self.recorded.lock().unwrap();
            let mut chain = Vec::new();
            let mut current = ENTERED.with(|entered| entered.borrow().last().copied());
            while let Some(id) = current {
                chain.push(id);
                current = recorded.spans.get(&id).and_then(|data| data.parent);
            }
            let mut fields = Fields::new();
            for id in chain.iter().rev() {
                if let Some(data) = recorded.spans.get(id) {
                    fields.extend(data.fields.clone());
                }
            }
            event.record(&mut Collect(&mut fields));
            recorded.events.push(fields);
        }

        fn enter(&self, span: &span::Id) {
            ENTERED.with(|entered| entered.borrow_mut().push(span.into_u64()));
        }

        fn exit(&self, span: &span::Id) {
            ENTERED.with(|entered| {
                let mut entered = entered.borrow_mut();
                if let Some(position) = entered.iter().rposition(|id| *id == span.into_u64()) {
                    entered.remove(position);
                }
            });
        }

        fn current_span(&self) -> span::Current {
            let current = ENTERED.with(|entered| entered.borrow().last().copied());
            let recorded = self.recorded.lock().unwrap();
            match current.and_then(|id| recorded.spans.get(&id).map(|data| (id, data.metadata))) {
                Some((id, metadata)) => span::Current::new(span::Id::from_u64(id), metadata),
                None => span::Current::none(),
            }
        }
    }

    fn all_ids() -> CorrelationIds {
        CorrelationIds::for_run(RunId::new("run-1").unwrap())
            .with_ticket(TicketId::new("F07").unwrap())
            .with_worker(WorkerId::new("worker-1").unwrap())
            .with_operation(OperationId::new("op-9").unwrap())
    }

    fn assert_all_ids(event: &Fields) {
        assert_eq!(
            event.get(RUN_ID_FIELD).map(String::as_str),
            Some("run-1"),
            "{event:?}"
        );
        assert_eq!(
            event.get(TICKET_ID_FIELD).map(String::as_str),
            Some("F07"),
            "{event:?}"
        );
        assert_eq!(
            event.get(WORKER_ID_FIELD).map(String::as_str),
            Some("worker-1"),
            "{event:?}"
        );
        assert_eq!(
            event.get(OPERATION_ID_FIELD).map(String::as_str),
            Some("op-9"),
            "{event:?}"
        );
    }

    #[tokio::test]
    async fn correlation_ids_propagate_across_async_tasks() {
        let capture = Capture::default();
        let _default = tracing::subscriber::set_default(capture.clone());

        all_ids()
            .scope(async {
                tracing::info!(step = "parent");
                let child = spawn_correlated(async {
                    tracing::info!(step = "child");
                    tokio::task::yield_now().await;
                    let grandchild = spawn_correlated(async {
                        tokio::task::yield_now().await;
                        tracing::info!(step = "grandchild");
                    });
                    grandchild.await.unwrap();
                    tracing::info!(step = "child after await");
                });
                tokio::task::yield_now().await;
                tracing::info!(step = "parent while the child runs");
                child.await.unwrap();
            })
            .await;
        tracing::info!(step = "outside");

        for step in [
            "parent",
            "child",
            "grandchild",
            "child after await",
            "parent while the child runs",
        ] {
            assert_all_ids(&capture.event(step));
        }
        let outside = capture.event("outside");
        assert!(!outside.contains_key(RUN_ID_FIELD), "{outside:?}");
    }

    #[tokio::test]
    async fn concurrent_tasks_keep_their_own_ids() {
        let capture = Capture::default();
        let _default = tracing::subscriber::set_default(capture.clone());
        let run = CorrelationIds::for_run(RunId::new("run-1").unwrap());

        let mut tasks = Vec::new();
        for worker in ["worker-a", "worker-b", "worker-c"] {
            let ids = run.clone().with_worker(WorkerId::new(worker).unwrap());
            tasks.push(tokio::spawn(ids.scope(async move {
                for _ in 0..3 {
                    tokio::task::yield_now().await;
                    tracing::info!(step = worker);
                }
            })));
        }
        for task in tasks {
            task.await.unwrap();
        }

        let events = capture.events();
        assert_eq!(events.len(), 9);
        for event in events {
            assert_eq!(event.get(WORKER_ID_FIELD), event.get("step"), "{event:?}");
            assert_eq!(event.get(RUN_ID_FIELD).map(String::as_str), Some("run-1"));
        }
    }

    #[tokio::test]
    async fn nested_scopes_add_ids_and_a_plain_spawn_loses_them() {
        let capture = Capture::default();
        let _default = tracing::subscriber::set_default(capture.clone());
        let run = CorrelationIds::for_run(RunId::new("run-1").unwrap());
        let worker = CorrelationIds::new().with_worker(WorkerId::new("worker-1").unwrap());

        run.scope(async {
            worker
                .scope(async { tracing::info!(step = "nested") })
                .await;
            worker.in_scope(|| tracing::info!(step = "nested sync"));
            tracing::info!(step = "run only");
            tokio::spawn(async { tracing::info!(step = "plain spawn") })
                .await
                .unwrap();
        })
        .await;

        for step in ["nested", "nested sync"] {
            let event = capture.event(step);
            assert_eq!(
                event.get(RUN_ID_FIELD).map(String::as_str),
                Some("run-1"),
                "{event:?}"
            );
            assert_eq!(
                event.get(WORKER_ID_FIELD).map(String::as_str),
                Some("worker-1")
            );
        }
        let run_only = capture.event("run only");
        assert_eq!(
            run_only.get(RUN_ID_FIELD).map(String::as_str),
            Some("run-1")
        );
        assert!(!run_only.contains_key(WORKER_ID_FIELD), "{run_only:?}");
        assert!(!capture.event("plain spawn").contains_key(RUN_ID_FIELD));
    }

    #[test]
    fn unset_ids_are_not_recorded() {
        let capture = Capture::default();
        tracing::subscriber::with_default(capture.clone(), || {
            CorrelationIds::new().in_scope(|| tracing::info!(step = "empty"));
        });
        let event = capture.event("empty");
        assert_eq!(event.len(), 1, "{event:?}");
        assert!(CorrelationIds::new().is_empty());
        assert!(!all_ids().is_empty());
    }
}
