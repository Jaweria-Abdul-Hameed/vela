//! The Git adapter's base and its read capability (G01): the `git` binary under the hardened invocation, `status`,
//! `rev-parse`, `diff`, `merge-base`, `log`, and the read-only remote reachability probe. `GitCli` is also the type the
//! other capabilities (`worktree`, `checkpoint`, `merge`, `remote`) implement their traits on, one file each.
//!
//! There is no libgit2: every operation shells out to `git` through the `ProcessRunner` port, so the output is redacted,
//! the environment is an allowlist, and a timeout kills the whole process tree.

pub mod hardening;

use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use vela_domain::errors::{ErrorClass, ErrorCode, VelaError};
use vela_domain::ids::{CommitSha, WorktreePath};
use vela_domain::ports::{
    CommandSpec, DiffSummary, GitContext, GitRead, OutputChunk, OutputSink, OutputStream,
    PortFuture, ProcessRunner, RemoteName, RepoStatus, RevisionSpec,
};

use crate::lock::OperationLocks;
use hardening::{
    DeclaredDrivers, Mode, NO_DIFF_DRIVERS, hardened_args, hardened_env, parse_declared_drivers,
};

/// The marker in the message of the error returned while Git's index is locked by another process. Callers turn it into a
/// pause, never into deleting the lock.
pub const INDEX_LOCKED: &str = "git index is locked";

/// True when `error` reports a locked Git index.
pub fn is_index_locked(error: &VelaError) -> bool {
    error.what_failed().contains(INDEX_LOCKED)
}

/// What a finished Git command produced.
#[derive(Debug, Clone)]
pub struct GitOutput {
    /// Exit code (`None` when it did not exit by itself).
    pub exit_code: Option<i32>,
    /// Standard output (redacted).
    pub stdout: String,
    /// Standard error (redacted).
    pub stderr: String,
}

impl GitOutput {
    /// True for exit code 0.
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
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

/// The Git adapter.
pub struct GitCli {
    runner: Arc<dyn ProcessRunner>,
    program: String,
    base_env: Vec<(String, String)>,
    timeout_ms: NonZeroU64,
    next_id: AtomicU64,
    managed_root: Option<PathBuf>,
    version: OnceLock<String>,
    pub(crate) locks: Arc<OperationLocks>,
}

impl std::fmt::Debug for GitCli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitCli")
            .field("program", &self.program)
            .field("managed_root", &self.managed_root)
            .finish()
    }
}

impl GitCli {
    /// A Git adapter that runs `git` through `runner`. `base_env` is the environment allowlist (see
    /// `vela_process::runner::baseline_env`); the hardened variables are added to it.
    pub fn new(runner: Arc<dyn ProcessRunner>, base_env: Vec<(String, String)>) -> Self {
        Self {
            runner,
            program: "git".to_owned(),
            base_env,
            timeout_ms: NonZeroU64::new(120_000).expect("non-zero"),
            next_id: AtomicU64::new(0),
            managed_root: None,
            version: OnceLock::new(),
            locks: Arc::new(OperationLocks::new()),
        }
    }

    /// Sets the Vela-managed worktree root (worktrees are created and removed only below it).
    pub fn with_managed_root(mut self, root: PathBuf) -> Self {
        self.managed_root = Some(root);
        self
    }

    /// Sets the Git program (a path or name).
    pub fn with_program(mut self, program: impl Into<String>) -> Self {
        self.program = program.into();
        self
    }

    /// Sets the per-command timeout.
    pub fn with_timeout_ms(mut self, timeout_ms: NonZeroU64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// The managed worktree root, if configured.
    pub fn managed_root(&self) -> Option<&PathBuf> {
        self.managed_root.as_ref()
    }

    /// The per-worktree operation locks.
    pub fn locks(&self) -> &Arc<OperationLocks> {
        &self.locks
    }

    async fn exec(
        &self,
        repo: &WorktreePath,
        args: Vec<String>,
        mode: Mode,
    ) -> Result<GitOutput, VelaError> {
        let mut env = self.base_env.clone();
        env.extend(hardened_env(mode));
        let spec = CommandSpec {
            id: format!("git-{}", self.next_id.fetch_add(1, Ordering::SeqCst) + 1),
            program: self.program.clone(),
            args,
            cwd: repo.clone(),
            env,
            timeout_ms: self.timeout_ms,
        };
        let sink = Capture::default();
        let outcome = self.runner.run(&spec, &sink).await?;
        if outcome.timed_out {
            return Err(VelaError::of_class(
                ErrorClass::ExternalTemporary,
                "git timed out",
            ));
        }
        Ok(GitOutput {
            exit_code: outcome.exit_code,
            stdout: sink.out.into_inner().expect("capture lock"),
            stderr: sink.err.into_inner().expect("capture lock"),
        })
    }

    /// Driver names the repository's own configuration declares (a config-only call that executes nothing).
    pub(crate) async fn declared_drivers(&self, repo: &WorktreePath) -> DeclaredDrivers {
        let minimal = vec![
            "--no-pager".to_owned(),
            "-c".to_owned(),
            "core.fsmonitor=false".to_owned(),
            "-c".to_owned(),
            format!("core.hooksPath={}", hardening::null_device()),
            "config".to_owned(),
            "--list".to_owned(),
            "--name-only".to_owned(),
        ];
        match self.exec(repo, minimal, Mode::Read).await {
            Ok(out) if out.success() => parse_declared_drivers(&out.stdout),
            _ => DeclaredDrivers::default(),
        }
    }

    /// Runs `git <args>` in `repo` under the hardened invocation and returns the raw result (any exit code).
    pub async fn run_raw(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
        mode: Mode,
        args: &[&str],
    ) -> Result<GitOutput, VelaError> {
        let drivers = self.declared_drivers(repo).await;
        let mut full = hardened_args(mode, ctx.trust, &drivers);
        full.extend(args.iter().map(|a| (*a).to_owned()));
        self.exec(repo, full, mode).await
    }

    /// Like [`GitCli::run_raw`], but a non-zero exit is a typed error.
    pub async fn run_ok(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
        mode: Mode,
        args: &[&str],
    ) -> Result<String, VelaError> {
        let out = self.run_raw(ctx, repo, mode, args).await?;
        if out.success() {
            Ok(out.stdout)
        } else {
            Err(classify_failure(
                args.first().copied().unwrap_or("git"),
                &out,
            ))
        }
    }

    /// The Git version (`git version 2.x`), recorded once.
    pub async fn version(&self, repo: &WorktreePath) -> Result<String, VelaError> {
        if let Some(v) = self.version.get() {
            return Ok(v.clone());
        }
        let out = self
            .exec(repo, vec!["--version".to_owned()], Mode::Read)
            .await?;
        if !out.success() {
            return Err(VelaError::of_class(
                ErrorClass::UserActionRequired,
                "Git could not be started; install Git for Windows",
            ));
        }
        let v = out.stdout.trim().to_owned();
        Ok(self.version.get_or_init(|| v).clone())
    }

    /// Read-only remote reachability probe (`ls-remote` under the hardened invocation, no prompts). `Ok(true)` when the remote
    /// answers (even with no matching ref), a typed network or authentication error otherwise.
    pub async fn ls_remote_probe(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
        remote: &RemoteName,
    ) -> Result<bool, VelaError> {
        let out = self
            .run_raw(
                ctx,
                repo,
                Mode::Remote,
                &["ls-remote", "--exit-code", remote.as_str(), "HEAD"],
            )
            .await?;
        match out.exit_code {
            Some(0 | 2) => Ok(true),
            _ => Err(classify_failure("ls-remote", &out)),
        }
    }

    /// One line per commit in `range`, newest first, at most `max` lines (`<sha> <subject>`).
    pub async fn log_oneline(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
        range: &RevisionSpec,
        max: u32,
    ) -> Result<Vec<String>, VelaError> {
        let count = max.to_string();
        let mut args = vec!["log", "--oneline", "--no-decorate", "-n", count.as_str()];
        args.extend(NO_DIFF_DRIVERS);
        args.push("--end-of-options");
        args.push(range.as_str());
        let text = self.run_ok(ctx, repo, Mode::Read, &args).await?;
        Ok(text.lines().map(str::to_owned).collect())
    }
}

/// Turns a failed Git command into a distinct typed error: not a repository, locked index, authentication, network,
/// non-fast-forward, or a generic validation failure carrying a short excerpt of Git's message.
pub fn classify_failure(command: &str, out: &GitOutput) -> VelaError {
    let text = format!("{}\n{}", out.stderr, out.stdout).to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| text.contains(n));
    if has(&["not a git repository"]) {
        return VelaError::of_class(
            ErrorClass::ValidationFailure,
            "the folder is not a Git repository",
        );
    }
    if has(&[
        "index.lock",
        "unable to create",
        "another git process seems to be running",
    ]) && has(&["lock"])
    {
        return VelaError::of_class(
            ErrorClass::ExternalTemporary,
            format!("{INDEX_LOCKED} by another process; try again shortly"),
        );
    }
    if has(&[
        "authentication failed",
        "could not read username",
        "could not read password",
        "permission denied (publickey)",
        "terminal prompts disabled",
        "invalid credentials",
        "repository not found",
    ]) {
        return VelaError::from_code(
            ErrorCode::AuthRequired,
            "Git could not authenticate to the remote; sign in with Git Credential Manager or gh auth login",
        );
    }
    if has(&[
        "non-fast-forward",
        "[rejected]",
        "fetch first",
        "failed to push some refs",
        "updates were rejected",
    ]) {
        return VelaError::of_class(
            ErrorClass::RepositoryConflict,
            "the remote has changes this branch does not (not a fast-forward); nothing was forced",
        );
    }
    if has(&[
        "could not resolve host",
        "connection timed out",
        "network is unreachable",
        "connection refused",
        "unable to access",
        "early eof",
        "remote end hung up",
        "ssl connect error",
    ]) {
        return VelaError::of_class(
            ErrorClass::ExternalTemporary,
            "the remote could not be reached (network)",
        );
    }
    // Prefer Git's own error line over the progress lines that precede it (for example "Preparing worktree").
    let line = out
        .stderr
        .lines()
        .find(|l| l.starts_with("fatal:") || l.starts_with("error:"))
        .or_else(|| out.stderr.lines().next())
        .unwrap_or("");
    let excerpt: String = line.chars().take(200).collect();
    VelaError::of_class(
        ErrorClass::ValidationFailure,
        format!("git {command} failed: {}", excerpt.trim()),
    )
}

/// Parses `git status --porcelain=v1 -z` output into changed paths.
pub fn parse_porcelain_z(text: &str) -> Vec<String> {
    text.split('\0')
        .filter(|entry| entry.len() > 3)
        .map(|entry| entry[3..].to_owned())
        .collect()
}

impl GitRead for GitCli {
    fn status<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
    ) -> PortFuture<'a, RepoStatus> {
        Box::pin(async move {
            let porcelain = self
                .run_ok(
                    ctx,
                    repo,
                    Mode::Read,
                    &[
                        "status",
                        "--porcelain=v1",
                        "-z",
                        "--untracked-files=all",
                        "--no-renames",
                    ],
                )
                .await?;
            let head = self
                .run_raw(
                    ctx,
                    repo,
                    Mode::Read,
                    &["rev-parse", "--verify", "--quiet", "HEAD"],
                )
                .await?;
            let head = if head.success() {
                CommitSha::new(head.stdout.trim()).ok()
            } else {
                None
            };
            let branch = self
                .run_raw(
                    ctx,
                    repo,
                    Mode::Read,
                    &["symbolic-ref", "--quiet", "--short", "HEAD"],
                )
                .await?;
            let branch = if branch.success() {
                vela_domain::ids::BranchName::new(branch.stdout.trim()).ok()
            } else {
                None
            };
            Ok(RepoStatus {
                head,
                branch,
                changed_paths: parse_porcelain_z(&porcelain),
            })
        })
    }

    fn resolve_ref<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        revision: &'a RevisionSpec,
    ) -> PortFuture<'a, CommitSha> {
        Box::pin(async move {
            let spec = format!("{}^{{commit}}", revision.as_str());
            let text = self
                .run_ok(
                    ctx,
                    repo,
                    Mode::Read,
                    &[
                        "rev-parse",
                        "--verify",
                        "--quiet",
                        "--end-of-options",
                        &spec,
                    ],
                )
                .await
                .map_err(|_| {
                    VelaError::of_class(
                        ErrorClass::ValidationFailure,
                        format!(
                            "`{}` does not name a commit in this repository",
                            revision.as_str()
                        ),
                    )
                })?;
            CommitSha::new(text.trim())
                .map_err(|e| VelaError::of_class(ErrorClass::InternalBug, e.to_string()))
        })
    }

    fn merge_base<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        a: &'a CommitSha,
        b: &'a CommitSha,
    ) -> PortFuture<'a, CommitSha> {
        Box::pin(async move {
            let text = self
                .run_ok(
                    ctx,
                    repo,
                    Mode::Read,
                    &["merge-base", a.as_str(), b.as_str()],
                )
                .await?;
            CommitSha::new(text.trim())
                .map_err(|e| VelaError::of_class(ErrorClass::InternalBug, e.to_string()))
        })
    }

    fn diff_summary<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        from: &'a CommitSha,
        to: &'a CommitSha,
    ) -> PortFuture<'a, DiffSummary> {
        Box::pin(async move {
            let mut args = vec!["diff", "--name-only", "-z"];
            args.extend(NO_DIFF_DRIVERS);
            args.extend([from.as_str(), to.as_str(), "--"]);
            let text = self.run_ok(ctx, repo, Mode::Read, &args).await?;
            Ok(DiffSummary {
                changed_paths: text
                    .split('\0')
                    .filter(|p| !p.is_empty())
                    .map(str::to_owned)
                    .collect(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(code: i32, stderr: &str) -> GitOutput {
        GitOutput {
            exit_code: Some(code),
            stdout: String::new(),
            stderr: stderr.to_owned(),
        }
    }

    #[test]
    fn failures_are_distinct_typed_errors() {
        let auth = classify_failure(
            "push",
            &out(128, "fatal: Authentication failed for 'https://x'"),
        );
        assert_eq!(auth.code(), Some(ErrorCode::AuthRequired));
        let prompts = classify_failure(
            "push",
            &out(
                128,
                "fatal: could not read Username for 'https://x': terminal prompts disabled",
            ),
        );
        assert_eq!(prompts.code(), Some(ErrorCode::AuthRequired));
        let net = classify_failure(
            "fetch",
            &out(
                128,
                "fatal: unable to access 'https://x/': Could not resolve host: x",
            ),
        );
        assert_eq!(net.class(), ErrorClass::ExternalTemporary);
        assert!(net.code().is_none());
        let nff = classify_failure(
            "push",
            &out(
                1,
                " ! [rejected]        main -> main (non-fast-forward)\nerror: failed to push some refs",
            ),
        );
        assert_eq!(nff.class(), ErrorClass::RepositoryConflict);
        let lock = classify_failure(
            "add",
            &out(
                128,
                "fatal: Unable to create 'C:/r/.git/index.lock': File exists.",
            ),
        );
        assert!(is_index_locked(&lock));
        assert_eq!(lock.class(), ErrorClass::ExternalTemporary);
        let repo = classify_failure(
            "status",
            &out(
                128,
                "fatal: not a git repository (or any of the parent directories): .git",
            ),
        );
        assert!(repo.what_failed().contains("not a Git repository"));
        let other = classify_failure("tag", &out(1, "error: something odd\nmore"));
        assert!(other.what_failed().contains("something odd"));
        assert!(!is_index_locked(&other));
    }

    #[test]
    fn porcelain_z_output_yields_paths() {
        assert_eq!(
            parse_porcelain_z(" M src/a.rs\0?? new file.txt\0A  b.rs\0"),
            ["src/a.rs", "new file.txt", "b.rs"]
        );
        assert!(parse_porcelain_z("").is_empty());
    }
}
