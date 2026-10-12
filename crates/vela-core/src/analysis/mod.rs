//! Graph ingestion (S06, S07): tracker issues become a validated graph snapshot with a stable layout, parallelization verdicts
//! and an analysis state.
//!
//! Ingestion goes through the tracker port, so local Markdown issues and GitHub issues (native blocked-by relationships) produce
//! the same graph. Everything read from a tracker is untrusted data: it can name tickets and blockers, never change policy.
//! The analyst hook point is optional; without it every pair with unknown write sets runs sequentially (the safe fallback).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::events::{Event, GraphBuiltPayload};
use vela_domain::graph::{DependencyGraph, GraphError, GraphSnapshot};
use vela_domain::ids::{DependencyEdge, EdgeOrigin, RunId, TicketId};
use vela_domain::layout::{GraphLayout, layout};
use vela_domain::parallel::{
    AnalystHazard, PairVerdict, PredictedWriteSet, SurfaceRules, classify_pair,
};
use vela_domain::ports::{IssueTrackerAdapter, PortFuture, TrackerTicket};
use vela_domain::state::{Cause, IntegrationHealth, InterventionKind, RunPhase, RunState};
use vela_persistence::repositories::s06_run_graphs::{self, RunGraph};

use crate::intervention::{InterventionDetails, Interventions};
use crate::orchestrator::Services;
use crate::orchestrator::runs::Runs;

pub mod analyst;
pub mod graph_build;
pub mod ingest;

/// Most pairs whose verdict is stored (a bound, not a limit on scheduling, which recomputes pairs on demand).
const MAX_STORED_VERDICTS: usize = 400;

/// What the dependency analyst contributes (A11 registers an implementation).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AnalystOutput {
    /// Proposed `(blocker, blocked)` edges; Vela drops any that would make a cycle.
    pub edges: Vec<(TicketId, TicketId)>,
    /// Predicted write sets by ticket.
    pub write_sets: BTreeMap<TicketId, PredictedWriteSet>,
    /// Hazards that add serialization.
    pub hazards: Vec<AnalystHazard>,
    /// The analyst that produced this (a prompt version or model label), stored with the graph.
    #[serde(default)]
    pub version: String,
}

/// The analyst hook point.
pub trait AnalystHook: Send + Sync {
    /// Analyzes the tickets (untrusted text) and the authored graph.
    fn analyze<'a>(
        &'a self,
        tickets: &'a [TrackerTicket],
        graph: &'a DependencyGraph,
    ) -> PortFuture<'a, AnalystOutput>;
}

/// How the graph was analyzed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisState {
    /// The analyst contributed.
    Analyzed,
    /// No analyst (or it failed): unknown pairs run sequentially.
    SequentialFallback,
}

impl AnalysisState {
    fn name(self) -> &'static str {
        match self {
            AnalysisState::Analyzed => "ANALYZED",
            AnalysisState::SequentialFallback => "SEQUENTIAL_FALLBACK",
        }
    }
}

/// The result of analyzing a run.
#[derive(Debug, Clone, PartialEq)]
pub enum Analysis {
    /// A valid graph was stored and the run is `GRAPH_READY`.
    Ready {
        /// The snapshot.
        snapshot: GraphSnapshot,
        /// Its layout.
        layout: GraphLayout,
        /// Things the user should know (ignored blockers, analyst failure).
        warnings: Vec<String>,
        /// How it was analyzed.
        state: AnalysisState,
    },
    /// The authored graph has a cycle; a `GRAPH_INVALID` intervention was raised and nothing was stored.
    Invalid {
        /// The intervention.
        intervention_id: String,
        /// Human-readable cycles.
        cycles: Vec<String>,
    },
}

/// Builds the authored graph from tickets. Blockers that name unknown tickets are ignored with a warning, never invented.
pub fn build_authored(
    tickets: &[TrackerTicket],
) -> Result<(DependencyGraph, Vec<String>), VelaError> {
    let mut graph = DependencyGraph::new();
    let mut warnings = Vec::new();
    for t in tickets {
        graph.add_ticket(t.id.clone()).map_err(|e| {
            VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!("the tracker returned an unusable ticket list: {e}"),
            )
        })?;
    }
    let known: BTreeSet<&TicketId> = tickets.iter().map(|t| &t.id).collect();
    for t in tickets {
        for blocker in &t.blockers {
            if !known.contains(blocker) {
                warnings.push(format!(
                    "{} names {blocker} as a blocker, but that ticket is not in the tracker; the dependency was ignored",
                    t.id
                ));
                continue;
            }
            let edge = DependencyEdge::new(blocker.clone(), t.id.clone(), EdgeOrigin::Authored)
                .map_err(|e| {
                    VelaError::of_class(
                        ErrorClass::ValidationFailure,
                        format!("{} cannot depend on itself: {e}", t.id),
                    )
                })?;
            match graph.add_edge(&edge) {
                Ok(()) | Err(GraphError::Cycle(_)) => {}
                Err(e) => warnings.push(format!("a dependency of {} was not used: {e}", t.id)),
            }
        }
    }
    Ok((graph, warnings))
}

/// Graph analysis over the shared services.
pub struct GraphService {
    services: Arc<Services>,
    tracker: Arc<dyn IssueTrackerAdapter>,
    analyst: Option<Arc<dyn AnalystHook>>,
}

impl GraphService {
    /// A service reading tickets from `tracker`.
    pub fn new(
        services: Arc<Services>,
        tracker: Arc<dyn IssueTrackerAdapter>,
        analyst: Option<Arc<dyn AnalystHook>>,
    ) -> Self {
        Self {
            services,
            tracker,
            analyst,
        }
    }

    /// Analyzes a run that is in `ANALYZING`: reads the tickets, validates the graph, stores it and moves the run to
    /// `GRAPH_READY` (one transaction with the `GraphBuilt` event). A cycle raises a `GRAPH_INVALID` intervention instead.
    pub async fn analyze(&self, run: &RunId) -> Result<Analysis, VelaError> {
        let runs = Runs::new(&self.services);
        if runs.run_state(run)? != RunState::Active(RunPhase::Analyzing) {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the run is not waiting for analysis",
            ));
        }
        let tickets = self.tracker.list_tickets().await?;
        if tickets.is_empty() {
            return Err(VelaError::of_class(
                ErrorClass::UserActionRequired,
                "the tracker has no tickets to build",
            )
            .with_user_action("add issues, then analyze again"));
        }
        let (mut graph, mut warnings) = build_authored(&tickets)?;

        if let Err(GraphError::Cycle(report)) = graph.validate() {
            let cycles: Vec<String> = report.cycles.iter().map(|c| c.to_string()).collect();
            let id = Interventions::new(&self.services)
                .raise_for_run(
                    run,
                    InterventionKind::GraphInvalid,
                    InterventionDetails {
                        summary: "the issues contain a dependency cycle".to_owned(),
                        evidence: cycles.clone(),
                        preserved_work: "nothing was started".to_owned(),
                        last_safe_commit: None,
                        actions: vec![
                            "remove one dependency in the tracker, then analyze again".to_owned(),
                            "cancel the run".to_owned(),
                        ],
                        consequences: "the run waits until the cycle is gone".to_owned(),
                    },
                )
                .await?;
            return Ok(Analysis::Invalid {
                intervention_id: id,
                cycles,
            });
        }

        let (state, output) = match &self.analyst {
            None => (AnalysisState::SequentialFallback, AnalystOutput::default()),
            Some(hook) => match hook.analyze(&tickets, &graph).await {
                Ok(out) => (AnalysisState::Analyzed, out),
                Err(e) => {
                    warnings.push(format!(
                        "the dependency analyst failed ({}); unknown pairs run sequentially",
                        e.what_failed()
                    ));
                    (AnalysisState::SequentialFallback, AnalystOutput::default())
                }
            },
        };
        for (blocker, blocked) in &output.edges {
            if let Ok(edge) =
                DependencyEdge::new(blocker.clone(), blocked.clone(), EdgeOrigin::Analyst)
            {
                // An edge to or from an unknown ticket, or one that closes a cycle, is dropped by Vela, never trusted.
                let _ = graph.add_edge(&edge);
            }
        }
        let dropped = graph.drop_cyclic_analyst_edges();
        if !dropped.is_empty() {
            warnings.push(format!(
                "{} analyst dependencies were dropped because they would create a cycle",
                dropped.len()
            ));
        }
        let snapshot = graph.snapshot().map_err(|e| {
            VelaError::internal(format!("a validated graph cannot be snapshotted: {e}"))
        })?;
        let graph_layout = layout(&snapshot);
        let verdicts = pair_verdicts(&graph, &snapshot, &output);

        let row = RunGraph {
            run_id: run.as_str().to_owned(),
            snapshot: to_json(&snapshot)?,
            layout: to_json(&graph_layout)?,
            verdicts: to_json(&verdicts)?,
            analyst: to_json(&output)?,
            warnings: to_json(&warnings)?,
            analysis_state: state.name().to_owned(),
            approved_ms: None,
            last_cycle: "{}".to_owned(),
            created_ms: self.services.now_ms(),
        };
        let built = Event::GraphBuilt(GraphBuiltPayload {
            graph_snapshot_id: format!("graph-{}", run.as_str()),
            ticket_count: u32::try_from(snapshot.tickets.len()).unwrap_or(u32::MAX),
            edge_count: u32::try_from(snapshot.edges.len()).unwrap_or(u32::MAX),
        });
        self.services
            .store
            .write(move |tx| s06_run_graphs::upsert(tx.conn(), &row))?;
        runs.transition_run(
            run,
            RunState::Active(RunPhase::GraphReady),
            Cause::System,
            IntegrationHealth::Healthy,
            vec![built],
        )
        .await?;
        Ok(Analysis::Ready {
            snapshot,
            layout: graph_layout,
            warnings,
            state,
        })
    }

    /// The stored graph of a run.
    pub fn stored(&self, run: &RunId) -> Result<Option<StoredGraph>, VelaError> {
        stored_graph(&self.services, run)
    }
}

/// The stored graph of a run.
pub fn stored_graph(services: &Services, run: &RunId) -> Result<Option<StoredGraph>, VelaError> {
    let r = run.as_str().to_owned();
    let row = services.store.read(|c| s06_run_graphs::get(c, &r))?;
    row.map(StoredGraph::from_row).transpose()
}

fn to_json<T: Serialize>(value: &T) -> Result<String, VelaError> {
    serde_json::to_string(value).map_err(|e| VelaError::internal(e.to_string()))
}

fn from_json<T: serde::de::DeserializeOwned>(text: &str, what: &str) -> Result<T, VelaError> {
    serde_json::from_str(text)
        .map_err(|e| VelaError::internal(format!("the stored {what} is unreadable: {e}")))
}

/// Verdicts for the pairs inside each layer (the pairs that could ever run together).
fn pair_verdicts(
    graph: &DependencyGraph,
    snapshot: &GraphSnapshot,
    output: &AnalystOutput,
) -> Vec<PairVerdict> {
    let rules = SurfaceRules::default();
    let mut out = Vec::new();
    for layer in &snapshot.layers {
        for (i, a) in layer.iter().enumerate() {
            for b in &layer[i + 1..] {
                if out.len() >= MAX_STORED_VERDICTS {
                    return out;
                }
                out.push(classify_pair(
                    graph,
                    (a, output.write_sets.get(a)),
                    (b, output.write_sets.get(b)),
                    &output.hazards,
                    &rules,
                ));
            }
        }
    }
    out
}

/// A graph as stored, decoded.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredGraph {
    /// The snapshot.
    pub snapshot: GraphSnapshot,
    /// The layout.
    pub layout: GraphLayout,
    /// Verdicts for same-layer pairs.
    pub verdicts: Vec<PairVerdict>,
    /// The analyst's output.
    pub analyst: AnalystOutput,
    /// Warnings.
    pub warnings: Vec<String>,
    /// True for `ANALYZED`.
    pub analyzed: bool,
    /// When approved.
    pub approved_ms: Option<i64>,
}

impl StoredGraph {
    fn from_row(r: RunGraph) -> Result<Self, VelaError> {
        Ok(Self {
            snapshot: from_json(&r.snapshot, "graph")?,
            layout: from_json(&r.layout, "layout")?,
            verdicts: from_json(&r.verdicts, "verdicts")?,
            analyst: from_json(&r.analyst, "analysis")?,
            warnings: from_json(&r.warnings, "warnings")?,
            analyzed: r.analysis_state == "ANALYZED",
            approved_ms: r.approved_ms,
        })
    }

    /// The graph, re-validated from the snapshot.
    pub fn graph(&self) -> Result<DependencyGraph, VelaError> {
        DependencyGraph::from_snapshot(&self.snapshot)
            .map_err(|e| VelaError::internal(format!("the stored graph is invalid: {e}")))
    }
}

#[cfg(test)]
mod tests;
