//! Local Markdown tracker (T01; `ADAPTERS.md` "IssueTrackerAdapter", FR-005).
//!
//! Reads a directory of Markdown tickets into the normalized ticket model ([`TrackerTicket`]), turns explicit blockers
//! into [`DependencyEdge`]s, and reports malformed metadata as typed [`TicketWarning`]s without ever failing the whole
//! load. Ticket text is untrusted (ADR-013): it is never used to build a path (files are found by listing the one
//! configured directory, never by an id or a name taken from a ticket), symbolic links and oversized or non-UTF-8
//! files are skipped with a warning, and nothing read here changes Vela's policy.
//!
//! Writing status, evidence and closure back to local files is not part of this ticket; those operations report
//! `CapabilityMissing` until a ticket that owns them is scheduled.

mod parse;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use vela_domain::errors::{ErrorCode, VelaError};
use vela_domain::ids::{DependencyEdge, EdgeOrigin, TicketId, case_fold_key};
use vela_domain::ports::{IssueTrackerAdapter, PortFuture, TrackerTicket};

pub use parse::{MAX_TITLE_CHARS, ParsedTicket, TicketWarning, WarningKind, parse_ticket};

/// Largest ticket file read; bigger files are skipped with a warning.
pub const MAX_TICKET_BYTES: u64 = 1024 * 1024;

/// Everything read from one directory of tickets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TicketCorpus {
    /// The tickets, in file-name order.
    pub tickets: Vec<TrackerTicket>,
    /// Explicit (authored) blocker edges between tickets that are both in the corpus.
    pub edges: Vec<DependencyEdge>,
    /// Every problem found.
    pub warnings: Vec<TicketWarning>,
}

impl TicketCorpus {
    /// Builds a corpus from `(file name, text)` pairs. Pure; the order of the pairs decides which of two tickets with
    /// the same id is kept.
    pub fn from_texts<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut corpus = Self::default();
        let mut seen: HashSet<String> = HashSet::new();
        let mut sources: Vec<String> = Vec::new();
        for (name, text) in files {
            let parsed = parse_ticket(name, text);
            corpus.warnings.extend(parsed.warnings);
            let Some(ticket) = parsed.ticket else {
                continue;
            };
            // Ids that differ only in case are one id on a case-insensitive filesystem.
            if !seen.insert(case_fold_key(ticket.id.as_str())) {
                corpus.warnings.push(TicketWarning::new(
                    name,
                    WarningKind::DuplicateTicketId {
                        id: ticket.id.to_string(),
                    },
                ));
                continue;
            }
            sources.push(name.to_owned());
            corpus.tickets.push(ticket);
        }
        let mut edges = Vec::new();
        let mut warnings = Vec::new();
        for (ticket, source) in corpus.tickets.iter().zip(&sources) {
            for blocker in &ticket.blockers {
                let known = corpus.tickets.iter().any(|other| &other.id == blocker);
                let edge = if known {
                    DependencyEdge::new(blocker.clone(), ticket.id.clone(), EdgeOrigin::Authored)
                        .ok()
                } else {
                    None
                };
                match edge {
                    Some(edge) => edges.push(edge),
                    None => warnings.push(TicketWarning::new(
                        source,
                        WarningKind::UnknownBlocker {
                            blocker: blocker.to_string(),
                        },
                    )),
                }
            }
        }
        corpus.edges = edges;
        corpus.warnings.extend(warnings);
        corpus
    }

    /// Reads every regular `*.md` file directly inside `dir`. An unreadable directory is an error; an unreadable or
    /// malformed file is a warning.
    pub fn load_dir(dir: &Path) -> Result<Self, VelaError> {
        let entries = fs::read_dir(dir).map_err(|error| {
            VelaError::internal(format!(
                "the local tracker directory could not be listed: {}",
                error.kind()
            ))
        })?;
        let mut names: Vec<String> = Vec::new();
        let mut warnings: Vec<TicketWarning> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !Path::new(&name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            {
                continue;
            }
            // Not a followed symbolic link: a ticket cannot point the tracker at a file outside its directory.
            match fs::symlink_metadata(entry.path()) {
                Ok(meta) if meta.is_file() => names.push(name),
                Ok(_) => {}
                Err(_) => warnings.push(TicketWarning::new(&name, WarningKind::Unreadable)),
            }
        }
        names.sort();
        let mut texts: Vec<(String, String)> = Vec::new();
        for name in names {
            let path = dir.join(&name);
            match fs::metadata(&path) {
                Ok(meta) if meta.len() > MAX_TICKET_BYTES => {
                    warnings.push(TicketWarning::new(
                        &name,
                        WarningKind::TooLarge {
                            max_bytes: MAX_TICKET_BYTES,
                        },
                    ));
                    continue;
                }
                Ok(_) => {}
                Err(_) => {
                    warnings.push(TicketWarning::new(&name, WarningKind::Unreadable));
                    continue;
                }
            }
            match fs::read(&path) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(text) => texts.push((name, text)),
                    Err(_) => warnings.push(TicketWarning::new(&name, WarningKind::NotUtf8)),
                },
                Err(_) => warnings.push(TicketWarning::new(&name, WarningKind::Unreadable)),
            }
        }
        let mut corpus = Self::from_texts(
            texts
                .iter()
                .map(|(name, text)| (name.as_str(), text.as_str())),
        );
        warnings.append(&mut corpus.warnings);
        corpus.warnings = warnings;
        Ok(corpus)
    }

    /// The ticket with this id.
    pub fn ticket(&self, id: &TicketId) -> Option<&TrackerTicket> {
        self.tickets.iter().find(|ticket| &ticket.id == id)
    }
}

/// The local Markdown tracker: tickets are the `*.md` files of one directory.
#[derive(Debug, Clone)]
pub struct LocalTracker {
    dir: PathBuf,
}

impl LocalTracker {
    /// A tracker over the directory of tickets.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Reads the directory (blocking). Prefer the port methods from async code.
    pub fn load(&self) -> Result<TicketCorpus, VelaError> {
        TicketCorpus::load_dir(&self.dir)
    }

    async fn load_async(&self) -> Result<TicketCorpus, VelaError> {
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || TicketCorpus::load_dir(&dir))
            .await
            .map_err(|_| VelaError::internal("the local tracker read was interrupted"))?
    }
}

fn read_only() -> VelaError {
    VelaError::from_code(
        ErrorCode::CapabilityMissing,
        "the local Markdown tracker is read-only",
    )
}

impl IssueTrackerAdapter for LocalTracker {
    fn list_tickets(&self) -> PortFuture<'_, Vec<TrackerTicket>> {
        Box::pin(async move { Ok(self.load_async().await?.tickets) })
    }

    fn read_ticket<'a>(&'a self, id: &'a TicketId) -> PortFuture<'a, TrackerTicket> {
        Box::pin(async move {
            self.load_async()
                .await?
                .tickets
                .into_iter()
                .find(|ticket| &ticket.id == id)
                .ok_or_else(|| {
                    VelaError::internal(format!("ticket {id} is not in the local tracker"))
                })
        })
    }

    fn update_status<'a>(&'a self, _id: &'a TicketId, _status: &'a str) -> PortFuture<'a, ()> {
        vela_domain::ports::ready(Err(read_only()))
    }

    fn comment_evidence<'a>(&'a self, _id: &'a TicketId, _body: &'a str) -> PortFuture<'a, ()> {
        vela_domain::ports::ready(Err(read_only()))
    }

    fn close_ticket<'a>(&'a self, _id: &'a TicketId) -> PortFuture<'a, ()> {
        vela_domain::ports::ready(Err(read_only()))
    }
}

#[cfg(test)]
mod tests;
