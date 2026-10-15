//! The Git-derived inventory (R05): when the state store is corrupt or lost and no backup restores it, what Vela can still know
//! comes from Git itself: the `vela/<run>/<ticket>` branches, Vela-managed worktrees and the run and ticket trailers on their
//! tip commits. The inventory is **read-only**: it never reconstructs a runnable run and never resumes one. Every run it finds
//! is `NEEDS_HUMAN`, or `FAILED` (`STATE_STORE_UNRECOVERABLE`) only if the user chooses to abandon it. Branches and worktrees are
//! never touched.

use std::collections::BTreeMap;

use vela_domain::errors::VelaError;
use vela_domain::ids::{CommitSha, WorktreePath};
use vela_domain::ports::{GitContext, GitHistory, GitWorktrees};

/// One ticket branch found for a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryTicket {
    /// The ticket, from the branch name.
    pub ticket: String,
    /// The branch.
    pub branch: String,
    /// Its tip.
    pub tip: CommitSha,
    /// The worktree it is checked out in, when one is.
    pub worktree: Option<String>,
    /// True when the tip commit carries the matching `Vela-Run` and `Vela-Ticket` trailers.
    pub trailers_confirm: bool,
}

/// What a reconstructed run is, and what may be done with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reconstructed {
    /// The only state a reconstructed run can be in until the user decides.
    NeedsHuman,
    /// The user chose to abandon it (reason `STATE_STORE_UNRECOVERABLE`).
    Failed,
}

/// A run seen from Git alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryRun {
    /// The run identifier from the branch names.
    pub run_id: String,
    /// The integration branch tip, when that branch exists.
    pub integration: Option<CommitSha>,
    /// Ticket branches.
    pub tickets: Vec<InventoryTicket>,
    /// Its state.
    pub state: Reconstructed,
}

/// Everything found, read-only.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Inventory {
    /// The runs, by identifier.
    pub runs: Vec<InventoryRun>,
}

impl Inventory {
    /// This inventory never resumes anything. Kept as a method so callers cannot forget it.
    pub const fn auto_resumes(&self) -> bool {
        false
    }

    /// The user abandons a run: it becomes `FAILED` with the recorded reason. Its branches and worktrees stay.
    pub fn abandon(&mut self, run_id: &str) -> Option<&'static str> {
        let run = self.runs.iter_mut().find(|r| r.run_id == run_id)?;
        run.state = Reconstructed::Failed;
        Some("STATE_STORE_UNRECOVERABLE")
    }
}

/// Splits `vela/<run>/<name>`.
pub fn parse_branch(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("vela/")?;
    let (run, name) = rest.split_once('/')?;
    (!run.is_empty() && !name.is_empty()).then_some((run, name))
}

/// The value of a `Key: value` trailer in a commit message's final paragraph.
pub fn trailer<'a>(message: &'a str, key: &str) -> Option<&'a str> {
    let paragraphs: Vec<&str> = message.trim_end().split("\n\n").collect();
    // A message of one paragraph is a subject only; trailers need a paragraph of their own after it.
    if paragraphs.len() < 2 {
        return None;
    }
    paragraphs.last()?.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(key).then(|| v.trim())
    })
}

/// Builds the inventory from the repository.
pub async fn rebuild<G>(
    git: &G,
    ctx: &GitContext,
    repo: &WorktreePath,
) -> Result<Inventory, VelaError>
where
    G: GitHistory + GitWorktrees + ?Sized,
{
    let branches = git.list_branches(ctx, repo, "vela/").await?;
    let worktrees = git.list_worktrees(ctx, repo).await?;
    let mut runs: BTreeMap<String, InventoryRun> = BTreeMap::new();
    for (branch, tip) in branches {
        let Some((run, name)) = parse_branch(branch.as_str()) else {
            continue;
        };
        let entry = runs.entry(run.to_owned()).or_insert_with(|| InventoryRun {
            run_id: run.to_owned(),
            integration: None,
            tickets: Vec::new(),
            state: Reconstructed::NeedsHuman,
        });
        if name == "integration" {
            entry.integration = Some(tip);
            continue;
        }
        let message = git
            .commit_message(ctx, repo, &tip)
            .await
            .unwrap_or_default();
        let confirms = trailer(&message, "Vela-Run") == Some(run)
            && trailer(&message, "Vela-Ticket") == Some(name);
        let worktree = worktrees
            .iter()
            .find(|w| w.branch.as_ref().is_some_and(|b| b == &branch))
            .map(|w| w.path.as_str().to_owned());
        entry.tickets.push(InventoryTicket {
            ticket: name.to_owned(),
            branch: branch.as_str().to_owned(),
            tip,
            worktree,
            trailers_confirm: confirms,
        });
    }
    Ok(Inventory {
        runs: runs.into_values().collect(),
    })
}

#[cfg(test)]
mod tests;
