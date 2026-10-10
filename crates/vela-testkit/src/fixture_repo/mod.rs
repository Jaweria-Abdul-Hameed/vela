//! `FixtureRepo`: deterministic throwaway Git repositories for tests (F03).
//!
//! A [`FixtureRepo`] is generated into a uniquely named temporary directory that is never inside a cloud-synced folder
//! or the Vela repository ([`guard`]) and is deleted when the value is dropped. Every [`FixtureKind`] is built from fixed
//! text with a fixed author, committer, and commit clock, so two builds of the same kind have identical commit ids.
//! Later tests add kinds additively by new builder functions and an entry in `tests/fixtures/specs/fixture-repos.json`;
//! they do not edit the builder core.
//!
//! Layout of one fixture: `<scratch>/repo` (the working tree), `<scratch>/gitconfig-empty` (the global Git
//! configuration used for every command, so the host's own configuration never leaks in), and, for the hostile kind,
//! `<scratch>/markers` (see [`hostile`]).

#![warn(missing_docs)]

mod content;
mod git;
pub mod guard;
pub mod hostile;
mod scratch;

#[cfg(test)]
mod tests;

pub use git::{BASE_EPOCH, COMMIT_SPACING_SECONDS, HOSTILE_DRIVER, neutralizing_args, null_device};
pub use guard::{GuardError, SyncProvider};
pub use hostile::{HOSTILE_HOOKS, HOSTILE_MARKERS};
pub use scratch::ScratchDir;

use git::GitRepo;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Output;

/// The default branch of every fixture.
pub const MAIN_BRANCH: &str = "main";
/// Branch carrying the seeded review defect ([`FixtureKind::SeededReviewDefect`]).
pub const DEFECT_BRANCH: &str = "feature/range-rewrite";
/// The two schema-overlap branches ([`FixtureKind::SchemaOverlapPair`]).
pub const SCHEMA_BRANCHES: [&str; 2] = ["ticket/schema-a", "ticket/schema-b"];
/// The two merge-conflict branches ([`FixtureKind::MergeConflictPair`]).
pub const CONFLICT_BRANCHES: [&str; 2] = ["ticket/conflict-a", "ticket/conflict-b"];
/// The file both schema-overlap branches modify.
pub const SCHEMA_PATH: &str = content::SCHEMA_PATH;
/// The file both merge-conflict branches modify.
pub const CONFLICT_PATH: &str = content::CORE_INDEX_PATH;
/// The tracked file the hostile repository's attributes send through the hostile driver.
pub const HOSTILE_PAYLOAD_PATH: &str = content::HOSTILE_PAYLOAD_PATH;

/// Which fixture repository to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixtureKind {
    /// A small multi-package project (core, util, app, store), one commit on `main`.
    MultiPackage,
    /// The multi-package project plus a seeded Markdown ticket set under `tickets/`.
    SeededTickets,
    /// The multi-package project plus a branch ([`DEFECT_BRANCH`]) whose only commit introduces a known defect.
    SeededReviewDefect,
    /// Two branches ([`SCHEMA_BRANCHES`]) that both change the schema file (different regions, so they merge
    /// textually clean) and both add a migration numbered `0002`: a semantic overlap Git does not report.
    SchemaOverlapPair,
    /// Two branches ([`CONFLICT_BRANCHES`]) that rewrite the same line of the same file differently.
    MergeConflictPair,
    /// A repository whose own configuration and hooks name programs that would write marker files if run.
    HostileLocalConfig,
}

impl FixtureKind {
    /// Every kind.
    pub const ALL: &'static [FixtureKind] = &[
        FixtureKind::MultiPackage,
        FixtureKind::SeededTickets,
        FixtureKind::SeededReviewDefect,
        FixtureKind::SchemaOverlapPair,
        FixtureKind::MergeConflictPair,
        FixtureKind::HostileLocalConfig,
    ];

    /// The stable name used in the fixture specification and in scratch directory names.
    pub const fn name(self) -> &'static str {
        match self {
            FixtureKind::MultiPackage => "multi_package",
            FixtureKind::SeededTickets => "seeded_tickets",
            FixtureKind::SeededReviewDefect => "seeded_review_defect",
            FixtureKind::SchemaOverlapPair => "schema_overlap_pair",
            FixtureKind::MergeConflictPair => "merge_conflict_pair",
            FixtureKind::HostileLocalConfig => "hostile_local_config",
        }
    }
}

/// The review defect seeded into [`FixtureKind::SeededReviewDefect`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeededDefect {
    /// The branch whose commit introduces the defect.
    pub branch: &'static str,
    /// The defective file.
    pub path: &'static str,
    /// The 1-based line of the defect on [`SeededDefect::branch`].
    pub line: usize,
    /// The defective line text.
    pub defective_line: &'static str,
    /// What the line should be.
    pub correct_line: &'static str,
}

/// Where the seeded review defect lives. The line number is computed from the fixture text, so it cannot drift.
pub fn seeded_defect() -> SeededDefect {
    SeededDefect {
        branch: DEFECT_BRANCH,
        path: content::RANGE_PATH,
        line: content::line_number_of(content::range_source(), content::RANGE_CORRECT_LINE)
            .expect("the range source contains the loop line"),
        defective_line: content::RANGE_DEFECTIVE_LINE,
        correct_line: content::RANGE_CORRECT_LINE,
    }
}

/// Why a fixture could not be built or inspected.
#[derive(Debug)]
pub enum FixtureError {
    /// The location was refused by the synced-folder guard.
    Guard(GuardError),
    /// A filesystem or process-spawn failure.
    Io {
        /// What was being done.
        context: String,
        /// The underlying error.
        source: std::io::Error,
    },
    /// `git` ran and exited non-zero.
    Git {
        /// The arguments (without the neutralising prefix).
        command: String,
        /// The exit code, if the process exited normally.
        exit_code: Option<i32>,
        /// Trimmed standard error.
        stderr: String,
    },
}

impl FixtureError {
    pub(crate) fn io(context: String, source: std::io::Error) -> Self {
        FixtureError::Io { context, source }
    }
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FixtureError::Guard(e) => write!(f, "{e}"),
            FixtureError::Io { context, source } => write!(f, "{context}: {source}"),
            FixtureError::Git {
                command,
                exit_code,
                stderr,
            } => write!(f, "git {command} failed (exit {exit_code:?}): {stderr}"),
        }
    }
}

impl std::error::Error for FixtureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FixtureError::Guard(e) => Some(e),
            FixtureError::Io { source, .. } => Some(source),
            FixtureError::Git { .. } => None,
        }
    }
}

impl From<GuardError> for FixtureError {
    fn from(e: GuardError) -> Self {
        FixtureError::Guard(e)
    }
}

/// Builder for a [`FixtureRepo`].
#[derive(Debug, Clone)]
pub struct FixtureRepoBuilder {
    kind: FixtureKind,
    parent: Option<PathBuf>,
}

impl FixtureRepoBuilder {
    /// Builds under `parent` (which must exist) instead of the OS temp directory. The guard still applies.
    pub fn parent_dir(mut self, parent: impl Into<PathBuf>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    /// Generates the repository. The guard runs before anything is created.
    pub fn build(self) -> Result<FixtureRepo, FixtureError> {
        let scratch = match &self.parent {
            Some(parent) => ScratchDir::create_in(parent, self.kind.name())?,
            None => ScratchDir::create(self.kind.name())?,
        };
        let repo_dir = scratch.path().join("repo");
        std::fs::create_dir(&repo_dir)
            .map_err(|source| FixtureError::io(format!("create {}", repo_dir.display()), source))?;
        let config = git::empty_config_path(scratch.path());
        std::fs::write(&config, b"")
            .map_err(|source| FixtureError::io(format!("write {}", config.display()), source))?;
        let mut git = GitRepo::new(repo_dir, config);
        git.run(&["init", "-b", MAIN_BRANCH])?;
        let markers = match self.kind {
            FixtureKind::MultiPackage => {
                write_base(&mut git)?;
                None
            }
            FixtureKind::SeededTickets => {
                write_base(&mut git)?;
                for (name, text) in content::SEEDED_TICKETS {
                    git.write(&format!("tickets/{name}"), &content::lf(text))?;
                }
                git.commit_all("docs: seed the ticket set")?;
                None
            }
            FixtureKind::SeededReviewDefect => {
                write_base(&mut git)?;
                git.run(&["checkout", "-b", DEFECT_BRANCH])?;
                git.write(content::RANGE_PATH, &content::range_with_defect())?;
                git.commit_all("perf(util): simplify the range loop")?;
                git.run(&["checkout", MAIN_BRANCH])?;
                None
            }
            FixtureKind::SchemaOverlapPair => {
                write_base(&mut git)?;
                git.run(&["checkout", "-b", SCHEMA_BRANCHES[0]])?;
                git.write(content::SCHEMA_PATH, &content::schema_side_a())?;
                git.write(
                    "packages/store/migrations/0002_add_ticket_owner.sql",
                    "-- 0002: tickets gain an owner.\nALTER TABLE tickets ADD COLUMN owner TEXT;\n",
                )?;
                git.commit_all("feat(store): add a ticket owner")?;
                git.run(&["checkout", MAIN_BRANCH])?;
                git.run(&["checkout", "-b", SCHEMA_BRANCHES[1]])?;
                git.write(content::SCHEMA_PATH, &content::schema_side_b())?;
                git.write(
                    "packages/store/migrations/0002_add_run_status.sql",
                    "-- 0002: runs gain a status.\nALTER TABLE runs ADD COLUMN status TEXT;\n",
                )?;
                git.commit_all("feat(store): add a run status")?;
                git.run(&["checkout", MAIN_BRANCH])?;
                None
            }
            FixtureKind::MergeConflictPair => {
                write_base(&mut git)?;
                git.run(&["checkout", "-b", CONFLICT_BRANCHES[0]])?;
                git.write(
                    content::CORE_INDEX_PATH,
                    &content::core_with_greeting(content::CONFLICT_LINE_A),
                )?;
                git.commit_all("feat(core): shorten the greeting")?;
                git.run(&["checkout", MAIN_BRANCH])?;
                git.run(&["checkout", "-b", CONFLICT_BRANCHES[1]])?;
                git.write(
                    content::CORE_INDEX_PATH,
                    &content::core_with_greeting(content::CONFLICT_LINE_B),
                )?;
                git.commit_all("feat(core): make the greeting welcoming")?;
                git.run(&["checkout", MAIN_BRANCH])?;
                None
            }
            FixtureKind::HostileLocalConfig => {
                for (path, text) in content::HOSTILE_FILES {
                    git.write(path, &content::lf(text))?;
                }
                git.commit_all("chore: initial hostile-config fixture")?;
                let markers = scratch.path().join("markers");
                hostile::install(&git, &markers)?;
                Some(markers)
            }
        };
        Ok(FixtureRepo {
            kind: self.kind,
            scratch,
            git,
            markers,
        })
    }
}

fn write_base(git: &mut GitRepo) -> Result<(), FixtureError> {
    for (path, text) in content::BASE_FILES {
        git.write(path, &content::lf(text))?;
    }
    git.commit_all("chore: initial multi-package workspace")
}

/// A generated throwaway repository. Deleted on drop.
#[derive(Debug)]
pub struct FixtureRepo {
    kind: FixtureKind,
    scratch: ScratchDir,
    git: GitRepo,
    markers: Option<PathBuf>,
}

impl FixtureRepo {
    /// Starts building a fixture of `kind`.
    pub fn builder(kind: FixtureKind) -> FixtureRepoBuilder {
        FixtureRepoBuilder { kind, parent: None }
    }

    /// Builds a fixture of `kind` in the OS temp directory.
    pub fn create(kind: FixtureKind) -> Result<FixtureRepo, FixtureError> {
        Self::builder(kind).build()
    }

    /// The kind that was built.
    pub fn kind(&self) -> FixtureKind {
        self.kind
    }

    /// The repository working tree.
    pub fn path(&self) -> &Path {
        &self.git.dir
    }

    /// The scratch directory that holds the working tree and everything else of this fixture.
    pub fn root(&self) -> &Path {
        self.scratch.path()
    }

    /// Runs `git` in the repository with the neutralising configuration; returns trimmed stdout, and an error for a
    /// non-zero exit. Safe on the hostile repository: `diff`, `log` and `show` automatically get
    /// `--no-ext-diff --no-textconv` in addition to the neutralising configuration.
    pub fn git(&self, args: &[&str]) -> Result<String, FixtureError> {
        self.git.run(args)
    }

    /// Like [`FixtureRepo::git`], but returns the raw output so a test can inspect a non-zero exit (for example a
    /// merge conflict).
    pub fn git_output(&self, args: &[&str]) -> Result<Output, FixtureError> {
        self.git.output(args)
    }

    /// Runs `git` WITHOUT the neutralising configuration. On the hostile repository this executes its payloads.
    #[cfg(test)]
    pub(crate) fn git_unneutralized(&self, args: &[&str]) -> Result<Output, FixtureError> {
        self.git.output_unneutralized(args)
    }

    /// The commit id `HEAD` points at.
    pub fn head(&self) -> Result<String, FixtureError> {
        self.git(&["rev-parse", "HEAD"])
    }

    /// Every branch with its commit id (`refs/heads/<name> <sha>` lines, sorted): the repository's identity.
    pub fn refs_snapshot(&self) -> Result<String, FixtureError> {
        self.git(&[
            "for-each-ref",
            "--sort=refname",
            "--format=%(refname) %(objectname)",
            "refs/heads",
        ])
    }

    /// The directory the hostile repository's payloads write markers into (`None` for the other kinds).
    pub fn markers_dir(&self) -> Option<&Path> {
        self.markers.as_deref()
    }

    /// The names of the marker files that exist, sorted: the payloads that ran. Empty for the other kinds.
    pub fn fired_markers(&self) -> Result<Vec<String>, FixtureError> {
        let Some(dir) = &self.markers else {
            return Ok(Vec::new());
        };
        let mut names = Vec::new();
        let entries = std::fs::read_dir(dir)
            .map_err(|source| FixtureError::io(format!("read {}", dir.display()), source))?;
        for entry in entries {
            let entry = entry
                .map_err(|source| FixtureError::io(format!("read {}", dir.display()), source))?;
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        Ok(names)
    }
}
