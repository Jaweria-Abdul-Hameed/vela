//! Commands the policy asked about (ADR-018 section 4): a command outside the confirmed profile is blocked by the hook, and when the
//! worker cannot go on without it the person is asked. On "allow once", Vela adds an exact, expiring rule to that worker's profile and
//! the worker resumes with an explicit note that the operation was approved. Which executable was asked about comes from the hook's
//! own records (never from the agent's words); the provider specifics live behind [`AskedCommands`].

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use vela_domain::errors::VelaError;
use vela_domain::ids::WorkerId;

/// How long a one-time approval stays usable.
pub const ALLOW_ONCE_TTL_MS: i64 = 10 * 60 * 1000;

/// What the provider adapter can say about asked commands and grant once.
pub trait AskedCommands: Send + Sync {
    /// The executables the policy asked about for `worker` since the last call, each reported once.
    fn take_asked(&self, worker: &WorkerId) -> Vec<String>;
    /// Lets `worker` run `executable` once more: an exact rule that expires after [`ALLOW_ONCE_TTL_MS`].
    fn grant_once(&self, worker: &WorkerId, executable: &str, now_ms: i64)
    -> Result<(), VelaError>;
}

/// The registered adapter, found through the shared services.
pub struct AskedCommandsService(pub Arc<dyn AskedCommands>);

/// Approvals the person gave, waiting to be told to the worker that resumes.
#[derive(Default)]
pub struct ApprovedNotes(Mutex<HashMap<String, Vec<String>>>);

impl ApprovedNotes {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that `executable` was approved once for `worker`.
    pub fn add(&self, worker: &WorkerId, executable: &str) {
        self.0
            .lock()
            .expect("notes lock")
            .entry(worker.as_str().to_owned())
            .or_default()
            .push(executable.to_owned());
    }

    /// The approvals for `worker`, cleared.
    pub fn take(&self, worker: &WorkerId) -> Vec<String> {
        self.0
            .lock()
            .expect("notes lock")
            .remove(worker.as_str())
            .unwrap_or_default()
    }
}

/// What the resumed session is told.
pub fn approval_note(executables: &[String]) -> String {
    let list = executables
        .iter()
        .map(|e| format!("`{e}`"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "The user approved running {list} once. Retry it now and continue with the ticket; ask again if you need anything else outside the allowed commands."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approvals_are_kept_per_worker_and_taken_once() {
        let n = ApprovedNotes::new();
        let (a, b) = (WorkerId::new("w-1").unwrap(), WorkerId::new("w-2").unwrap());
        n.add(&a, "npm");
        n.add(&a, "cargo");
        n.add(&b, "git");
        assert_eq!(n.take(&a), ["npm", "cargo"]);
        assert!(n.take(&a).is_empty());
        assert_eq!(n.take(&b), ["git"]);
    }

    #[test]
    fn the_note_names_each_approved_executable() {
        let s = approval_note(&["npm".into(), "cargo".into()]);
        assert!(s.contains("`npm`, `cargo`") && s.contains("approved"));
    }
}
