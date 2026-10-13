//! Conversations (A12): every provider conversation Vela started, for visibility, and an **Open in Antigravity** action that offers
//! only what was verified.
//!
//! Conversation ids are recorded by the sessions that own them (workers today; reviewer, verifier and analyst sessions when those
//! exist). This module is the read model across them. Whether Antigravity's Desktop app can be asked to show a conversation by id
//! is a spike result (SP08), registered here once someone has verified it; until then the action says so plainly instead of
//! pretending, and the AT-026 check fails the requirement rather than passing on Vela's own rendering.

use serde::{Deserialize, Serialize};
use vela_domain::errors::VelaError;
use vela_domain::ids::RunId;
use vela_persistence::repositories::o01_workers;

use crate::orchestrator::Services;

/// Which kind of session owned a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    /// A worker's implementation session.
    Worker,
    /// A reviewer's session.
    Reviewer,
    /// A verifier's session.
    Verifier,
    /// The dependency analyst's session.
    Analyst,
}

/// One conversation Vela can show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationRef {
    /// The kind of session.
    pub kind: SessionKind,
    /// The ticket it concerns, when it concerns one.
    pub ticket: Option<String>,
    /// The worker, for worker sessions.
    pub worker: Option<String>,
    /// The provider's conversation id.
    pub conversation_id: String,
}

/// The conversations of a run, across the session kinds that record one.
pub fn list_for_run(services: &Services, run: &RunId) -> Result<Vec<ConversationRef>, VelaError> {
    let r = run.as_str().to_owned();
    let workers = services.store.read(|c| o01_workers::list_for_run(c, &r))?;
    Ok(workers
        .into_iter()
        .filter_map(|w| {
            w.conversation_id.map(|id| ConversationRef {
                kind: SessionKind::Worker,
                ticket: Some(w.ticket_id),
                worker: Some(w.id),
                conversation_id: id,
            })
        })
        .collect())
}

/// How a conversation can be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAction {
    /// Start `program` with `args` (the SP08-verified mechanism).
    Launch {
        /// The program.
        program: String,
        /// Its arguments, with the conversation id already in place.
        args: Vec<String>,
    },
    /// It cannot be opened, and why (shown to the user as is).
    Unavailable {
        /// The reason.
        reason: String,
    },
}

/// A verified way to open a conversation: arguments with `{id}` where the conversation id goes. Registered after the spike.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenMechanism {
    /// The program.
    pub program: String,
    /// Argument templates; every `{id}` is replaced.
    pub args: Vec<String>,
}

/// The action for a conversation. Without a registered mechanism it is honestly unavailable. An id that could be read as an option
/// or contains anything beyond the id alphabet is never put in a command line.
pub fn open_action(services: &Services, c: &ConversationRef) -> OpenAction {
    let Some(m) = services.get::<OpenMechanism>() else {
        return OpenAction::Unavailable {
            reason: "Antigravity cannot yet be asked to open a conversation by its id: that mechanism has not been verified (spike SP08). The conversation id is shown so you can find it yourself.".to_owned(),
        };
    };
    let ok = !c.conversation_id.is_empty()
        && !c.conversation_id.starts_with('-')
        && c.conversation_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'));
    if !ok {
        return OpenAction::Unavailable {
            reason: "the conversation id is not safe to pass on a command line".to_owned(),
        };
    }
    OpenAction::Launch {
        program: m.program.clone(),
        args: m
            .args
            .iter()
            .map(|a| a.replace("{id}", &c.conversation_id))
            .collect(),
    }
}

/// AT-026: every in-scope conversation can be opened from the inspector. Returns the conversations that cannot, so a harness can
/// fail the requirement honestly instead of passing on how Vela renders the list.
pub fn unopenable(services: &Services, run: &RunId) -> Result<Vec<ConversationRef>, VelaError> {
    Ok(list_for_run(services, run)?
        .into_iter()
        .filter(|c| matches!(open_action(services, c), OpenAction::Unavailable { .. }))
        .collect())
}

#[cfg(test)]
mod tests;
