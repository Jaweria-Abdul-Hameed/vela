//! GitHub issue tracker through the `gh` CLI (T02, ADR-017): Vela never handles a token, `gh` owns authentication. Everything
//! read from GitHub (titles, bodies, labels, comments) is untrusted data (ADR-013): untrusted text is only ever passed to `gh`
//! as the value of a `--flag=value` argument, and ticket numbers are digits parsed from a validated id, never free text.

use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use vela_domain::errors::{ErrorClass, ErrorCode, VelaError};
use vela_domain::ids::{TicketId, WorktreePath};
use vela_domain::ports::{
    CommandSpec, IssueTrackerAdapter, OutputChunk, OutputSink, OutputStream, PortFuture,
    ProcessRunner, PullRequestPort, PullRequestRequest, TrackerTicket,
};

/// The label namespace Vela uses for status.
pub const STATUS_LABEL_PREFIX: &str = "status:";

/// Whether `gh` can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhStatus {
    /// Installed and logged in.
    Ready,
    /// Not installed.
    Missing,
    /// Installed but not logged in.
    Unauthenticated,
}

impl GhStatus {
    /// The typed error the run degrades with (local-only, `AUTH_REQUIRED`), or `None` when ready.
    pub fn degraded_error(self) -> Option<VelaError> {
        match self {
            Self::Ready => None,
            Self::Missing => Some(VelaError::from_code(
                ErrorCode::AuthRequired,
                "the GitHub CLI (`gh`) is not installed; continuing local-only",
            )),
            Self::Unauthenticated => Some(VelaError::from_code(
                ErrorCode::AuthRequired,
                "the GitHub CLI is not logged in (`gh auth login`); continuing local-only",
            )),
        }
    }
}

#[derive(Default)]
struct Capture {
    out: Mutex<String>,
    err: Mutex<String>,
}

impl OutputSink for Capture {
    fn on_output(&self, chunk: OutputChunk) {
        let target = match chunk.stream {
            OutputStream::Stdout => &self.out,
            OutputStream::Stderr => &self.err,
        };
        target.lock().expect("capture lock").push_str(&chunk.text);
    }
}

#[derive(Debug)]
struct GhOutput {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

#[derive(Deserialize)]
struct RawLabel {
    name: String,
}

#[derive(Deserialize)]
struct RawComment {
    body: String,
}

#[derive(Deserialize)]
struct RawIssue {
    number: u32,
    title: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    labels: Vec<RawLabel>,
    #[serde(default)]
    comments: Vec<RawComment>,
}

#[derive(Deserialize)]
struct RawBlocker {
    number: u32,
}

/// Valid `owner/name`: no leading dash or dot, only the characters GitHub allows.
fn valid_repo(repo: &str) -> bool {
    let mut parts = repo.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    [owner, name].iter().all(|p| {
        !p.is_empty()
            && !p.starts_with(['-', '.'])
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    })
}

/// The GitHub tracker.
pub struct GhTracker {
    runner: Arc<dyn ProcessRunner>,
    repo: String,
    cwd: WorktreePath,
    env: Vec<(String, String)>,
    timeout_ms: NonZeroU64,
    next_id: AtomicU64,
}

impl std::fmt::Debug for GhTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GhTracker")
            .field("repo", &self.repo)
            .finish()
    }
}

impl GhTracker {
    /// A tracker for `owner/name`. `env` is the process allowlist (it must not contain a token; `gh` finds its own
    /// credentials). Fails when `repo` is not a plain `owner/name`.
    pub fn new(
        runner: Arc<dyn ProcessRunner>,
        repo: &str,
        cwd: WorktreePath,
        env: Vec<(String, String)>,
    ) -> Result<Self, VelaError> {
        if !valid_repo(repo) {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the GitHub repository must be written owner/name",
            ));
        }
        Ok(Self {
            runner,
            repo: repo.to_owned(),
            cwd,
            env,
            timeout_ms: NonZeroU64::new(30_000).expect("non-zero"),
            next_id: AtomicU64::new(0),
        })
    }

    async fn gh(&self, args: Vec<String>) -> Result<GhOutput, VelaError> {
        let id = format!("gh-{}", self.next_id.fetch_add(1, Ordering::SeqCst) + 1);
        let spec = CommandSpec {
            id,
            program: "gh".to_owned(),
            args,
            cwd: self.cwd.clone(),
            env: self.env.clone(),
            timeout_ms: self.timeout_ms,
        };
        let sink = Capture::default();
        let outcome = self.runner.run(&spec, &sink).await?;
        if outcome.timed_out {
            return Err(VelaError::of_class(
                ErrorClass::ExternalTemporary,
                "`gh` timed out",
            ));
        }
        Ok(GhOutput {
            exit_code: outcome.exit_code,
            stdout: sink.out.into_inner().expect("capture lock"),
            stderr: sink.err.into_inner().expect("capture lock"),
        })
    }

    async fn gh_ok(&self, args: Vec<String>) -> Result<String, VelaError> {
        let out = self.gh(args).await?;
        if out.exit_code == Some(0) {
            return Ok(out.stdout);
        }
        Err(classify_failure(&out.stderr))
    }

    /// Detects whether `gh` is installed and logged in. Never fails: problems are the answer.
    pub async fn detect(&self) -> GhStatus {
        match self.gh(vec!["auth".into(), "status".into()]).await {
            Ok(o) if o.exit_code == Some(0) => GhStatus::Ready,
            Ok(_) => GhStatus::Unauthenticated,
            Err(_) => GhStatus::Missing,
        }
    }

    async fn blockers_of(&self, number: u32) -> Result<Vec<TicketId>, VelaError> {
        let text = self
            .gh_ok(vec![
                "api".into(),
                format!(
                    "repos/{}/issues/{number}/dependencies/blocked_by",
                    self.repo
                ),
            ])
            .await?;
        let raw: Vec<RawBlocker> = parse(&text)?;
        raw.into_iter().map(|b| ticket_id(b.number)).collect()
    }

    async fn to_ticket(&self, raw: RawIssue) -> Result<TrackerTicket, VelaError> {
        let blockers = self.blockers_of(raw.number).await?;
        Ok(TrackerTicket {
            id: ticket_id(raw.number)?,
            title: raw.title,
            body: raw.body.unwrap_or_default(),
            blockers,
            labels: raw.labels.into_iter().map(|l| l.name).collect(),
        })
    }

    async fn view(&self, number: u32, fields: &str) -> Result<RawIssue, VelaError> {
        let text = self
            .gh_ok(vec![
                "issue".into(),
                "view".into(),
                number.to_string(),
                "--repo".into(),
                self.repo.clone(),
                "--json".into(),
                fields.to_owned(),
            ])
            .await?;
        parse(&text)
    }
}

fn parse<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, VelaError> {
    serde_json::from_str(text).map_err(|e| {
        VelaError::of_class(
            ErrorClass::ValidationFailure,
            format!("`gh` returned output Vela could not read: {e}"),
        )
    })
}

/// The ticket id Vela uses for GitHub issue `number`.
pub fn ticket_id(number: u32) -> Result<TicketId, VelaError> {
    TicketId::new(format!("gh-{number}"))
        .map_err(|e| VelaError::of_class(ErrorClass::InternalBug, e.to_string()))
}

/// The issue number inside a `gh-<n>` ticket id.
pub fn issue_number(id: &TicketId) -> Result<u32, VelaError> {
    id.as_str()
        .strip_prefix("gh-")
        .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| {
            VelaError::of_class(
                ErrorClass::ValidationFailure,
                "not a GitHub issue ticket id",
            )
        })
}

fn classify_failure(stderr: &str) -> VelaError {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("gh auth login")
        || lower.contains("authentication")
        || lower.contains("bad credentials")
    {
        return VelaError::from_code(
            ErrorCode::AuthRequired,
            "GitHub authentication is required (`gh auth login`)",
        );
    }
    if [
        "network",
        "timeout",
        "timed out",
        "could not resolve",
        "connecting to",
        "502",
        "503",
        "504",
    ]
    .iter()
    .any(|m| lower.contains(m))
    {
        return VelaError::of_class(ErrorClass::ExternalTemporary, "GitHub is unreachable");
    }
    let excerpt: String = stderr.chars().take(200).collect();
    VelaError::of_class(
        ErrorClass::ValidationFailure,
        format!("`gh` failed: {}", excerpt.trim()),
    )
}

impl IssueTrackerAdapter for GhTracker {
    fn list_tickets(&self) -> PortFuture<'_, Vec<TrackerTicket>> {
        Box::pin(async move {
            let text = self
                .gh_ok(vec![
                    "issue".into(),
                    "list".into(),
                    "--repo".into(),
                    self.repo.clone(),
                    "--state".into(),
                    "open".into(),
                    "--limit".into(),
                    "200".into(),
                    "--json".into(),
                    "number,title,body,labels".into(),
                ])
                .await?;
            let raw: Vec<RawIssue> = parse(&text)?;
            let mut out = Vec::with_capacity(raw.len());
            for issue in raw {
                out.push(self.to_ticket(issue).await?);
            }
            Ok(out)
        })
    }

    fn read_ticket<'a>(&'a self, id: &'a TicketId) -> PortFuture<'a, TrackerTicket> {
        Box::pin(async move {
            let number = issue_number(id)?;
            let raw = self.view(number, "number,title,body,labels").await?;
            self.to_ticket(raw).await
        })
    }

    fn update_status<'a>(&'a self, id: &'a TicketId, status: &'a str) -> PortFuture<'a, ()> {
        Box::pin(async move {
            let number = issue_number(id)?;
            let current = self.view(number, "number,title,labels").await?;
            let wanted = format!("{STATUS_LABEL_PREFIX}{status}");
            let mut args = vec![
                "issue".to_owned(),
                "edit".to_owned(),
                number.to_string(),
                "--repo".to_owned(),
                self.repo.clone(),
            ];
            let mut changed = false;
            for l in current
                .labels
                .iter()
                .filter(|l| l.name.starts_with(STATUS_LABEL_PREFIX))
            {
                if l.name != wanted {
                    args.push(format!("--remove-label={}", l.name));
                    changed = true;
                }
            }
            if !current.labels.iter().any(|l| l.name == wanted) {
                args.push(format!("--add-label={wanted}"));
                changed = true;
            }
            if changed {
                self.gh_ok(args).await?;
            }
            Ok(())
        })
    }

    fn comment_evidence<'a>(&'a self, id: &'a TicketId, body: &'a str) -> PortFuture<'a, ()> {
        Box::pin(async move {
            let number = issue_number(id)?;
            self.gh_ok(vec![
                "issue".into(),
                "comment".into(),
                number.to_string(),
                "--repo".into(),
                self.repo.clone(),
                format!("--body={body}"),
            ])
            .await
            .map(|_| ())
        })
    }

    fn has_evidence_marker<'a>(
        &'a self,
        id: &'a TicketId,
        marker: &'a str,
    ) -> PortFuture<'a, bool> {
        Box::pin(async move {
            let number = issue_number(id)?;
            let raw = self.view(number, "number,title,comments").await?;
            Ok(raw.comments.iter().any(|c| c.body.contains(marker)))
        })
    }

    fn close_ticket<'a>(&'a self, id: &'a TicketId) -> PortFuture<'a, ()> {
        Box::pin(async move {
            let number = issue_number(id)?;
            self.gh_ok(vec![
                "issue".into(),
                "close".into(),
                number.to_string(),
                "--repo".into(),
                self.repo.clone(),
            ])
            .await
            .map(|_| ())
        })
    }
}

/// A pull request URL: https on github.com, no whitespace, nothing that could be read as an option.
fn valid_pr_url(url: &str) -> bool {
    url.starts_with("https://github.com/")
        && url.len() < 300
        && url
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.' | ':'))
}

impl PullRequestPort for GhTracker {
    fn create_draft<'a>(&'a self, request: &'a PullRequestRequest) -> PortFuture<'a, String> {
        Box::pin(async move {
            let out = self
                .gh_ok(vec![
                    "pr".into(),
                    "create".into(),
                    "--draft".into(),
                    "--repo".into(),
                    self.repo.clone(),
                    format!("--head={}", request.branch.as_str()),
                    format!("--base={}", request.base.as_str()),
                    format!("--title={}", request.title),
                    format!("--body={}", request.body),
                ])
                .await?;
            let url = out
                .lines()
                .rev()
                .map(str::trim)
                .find(|l| l.starts_with("https://"))
                .unwrap_or("");
            if valid_pr_url(url) {
                Ok(url.to_owned())
            } else {
                Err(VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    "`gh` did not return a pull request address",
                ))
            }
        })
    }

    fn mark_ready<'a>(&'a self, url: &'a str) -> PortFuture<'a, ()> {
        Box::pin(async move {
            if !valid_pr_url(url) {
                return Err(VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    "not a pull request address",
                ));
            }
            self.gh_ok(vec![
                "pr".into(),
                "ready".into(),
                url.to_owned(),
                "--repo".into(),
                self.repo.clone(),
            ])
            .await
            .map(|_| ())
        })
    }

    fn merge<'a>(&'a self, url: &'a str) -> PortFuture<'a, ()> {
        Box::pin(async move {
            if !valid_pr_url(url) {
                return Err(VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    "not a pull request address",
                ));
            }
            self.gh_ok(vec![
                "pr".into(),
                "merge".into(),
                url.to_owned(),
                "--repo".into(),
                self.repo.clone(),
                "--merge".into(),
            ])
            .await
            .map(|_| ())
        })
    }
}

#[cfg(test)]
mod tests;
