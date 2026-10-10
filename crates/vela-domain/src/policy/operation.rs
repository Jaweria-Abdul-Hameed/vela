//! The operation and context model of the policy engine.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::normalizer::{Dialect, NormalizedCommand, normalize};
use super::path::{Placement, ResolvedPath};

/// What an agent asks to do. Raw command text is normalized inside the engine so a caller cannot skip normalization.
///
/// The `Debug` form never prints command text or paths, which can carry secrets or user names.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Operation {
    /// Run a command line.
    Command {
        /// The syntax the text is written in.
        dialect: Dialect,
        /// The command line as the agent or the approval surface presents it.
        text: String,
        /// The working directory, when known; the worktree root when absent.
        cwd: Option<String>,
    },
    /// Read a file or directory.
    FileRead {
        /// The path.
        path: String,
    },
    /// Create or modify a file or directory.
    FileWrite {
        /// The path.
        path: String,
    },
    /// Delete a file or directory.
    FileDelete {
        /// The path.
        path: String,
        /// True when everything below a directory is deleted too.
        recursive: bool,
    },
}

impl fmt::Debug for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Operation::Command { .. } => "Command",
            Operation::FileRead { .. } => "FileRead",
            Operation::FileWrite { .. } => "FileWrite",
            Operation::FileDelete { .. } => "FileDelete",
        };
        write!(f, "Operation::{kind}(..)")
    }
}

/// Whether the repository has been trusted by an explicit user action (ADR-013). Repository, issue and `AGENTS.md` text
/// can never change this; only the settings store can, from a user action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustState {
    /// The default for every imported repository: data-only.
    Untrusted,
    /// The user explicitly trusted the repository.
    Trusted,
}

/// The run's autonomy mode (`AUTONOMY_MODES.md`). The core policy decides identically in both; it is carried so the broker
/// and the hook table record the mode a decision was made under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Policy-allowed routine requests are delivered; push, merge and promotion remain gated.
    Supervised,
    /// A broader project-scoped allow policy; still bounded by this engine.
    Autonomous,
}

/// One command of the user-confirmed command profile: an exact normalized command and the directory it runs in.
///
/// Deserialization re-validates: the stored form must be what the normalizer produces for the stored command, so a
/// persisted or tampered entry cannot hold something the normalizer would reject.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawProfileEntry")]
pub struct ProfileEntry {
    executable: String,
    args: Vec<String>,
    /// Absolute working directory in canonical form, below or at the worktree root.
    cwd: String,
    /// `cwd` resolved once, for matching.
    #[serde(skip)]
    resolved_cwd: ResolvedPath,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProfileEntry {
    executable: String,
    args: Vec<String>,
    cwd: String,
}

impl TryFrom<RawProfileEntry> for ProfileEntry {
    type Error = ContextError;

    fn try_from(raw: RawProfileEntry) -> Result<Self, Self::Error> {
        let canonical = NormalizedCommand::from_parts(raw.executable.clone(), raw.args.clone())
            .canonical_text();
        // The stored form must re-parse to exactly the stored command under at least one dialect (`cmd` is the more
        // permissive about bare words, PowerShell does not split `tools/x` at the slash).
        let reproduces = [Dialect::Cmd, Dialect::PowerShell]
            .into_iter()
            .any(|dialect| {
                normalize(&canonical, dialect).is_ok_and(|again| {
                    again.executable() == raw.executable && again.args() == raw.args.as_slice()
                })
            });
        if !reproduces {
            return Err(ContextError::UnnormalizableCommand);
        }
        let cwd = ResolvedPath::resolve(None, &raw.cwd)
            .map_err(|_| ContextError::InvalidProfileDirectory)?;
        if cwd.display() != raw.cwd {
            return Err(ContextError::InvalidProfileDirectory);
        }
        Ok(ProfileEntry {
            executable: raw.executable,
            args: raw.args,
            cwd: raw.cwd,
            resolved_cwd: cwd,
        })
    }
}

impl fmt::Debug for ProfileEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ProfileEntry({} +{} args)",
            self.executable,
            self.args.len()
        )
    }
}

/// Why a profile entry or context could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextError {
    /// The worktree root is not an absolute, resolvable path.
    InvalidRoot,
    /// A profile command is not a single simple command.
    UnnormalizableCommand,
    /// A profile directory is not resolvable or is outside the worktree root.
    InvalidProfileDirectory,
    /// The profile holds more than [`MAX_PROFILE_ENTRIES`] entries.
    ProfileTooLarge,
}

impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ContextError::InvalidRoot => "worktree root is not an absolute resolvable path",
            ContextError::UnnormalizableCommand => "profile command is not a single simple command",
            ContextError::InvalidProfileDirectory => {
                "profile directory is invalid or outside the worktree"
            }
            ContextError::ProfileTooLarge => "command profile is too large",
        })
    }
}

impl std::error::Error for ContextError {}

/// Most entries a command profile may hold.
pub const MAX_PROFILE_ENTRIES: usize = 512;

impl ProfileEntry {
    /// Builds an entry from the command text the user confirmed. `cwd` defaults to the worktree root and must be at or
    /// below it.
    pub fn parse(
        text: &str,
        dialect: Dialect,
        cwd: Option<&str>,
        root: &str,
    ) -> Result<ProfileEntry, ContextError> {
        let root = ResolvedPath::resolve(None, root).map_err(|_| ContextError::InvalidRoot)?;
        let command = normalize(text, dialect).map_err(|_| ContextError::UnnormalizableCommand)?;
        let dir = match cwd {
            Some(raw) => ResolvedPath::resolve(Some(&root), raw)
                .map_err(|_| ContextError::InvalidProfileDirectory)?,
            None => root.clone(),
        };
        if dir.placement(&root) != Placement::InsideRoot {
            return Err(ContextError::InvalidProfileDirectory);
        }
        let entry = ProfileEntry::try_from(RawProfileEntry {
            executable: command.executable().to_string(),
            args: command.args().to_vec(),
            cwd: dir.display(),
        })?;
        Ok(entry)
    }

    /// The normalized executable.
    pub fn executable(&self) -> &str {
        &self.executable
    }

    /// The literal arguments.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// The canonical absolute working directory.
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    /// The command as one canonical line.
    pub fn canonical_text(&self) -> String {
        NormalizedCommand::from_parts(self.executable.clone(), self.args.clone()).canonical_text()
    }
}

/// The user-confirmed command profile of a project (ADR-013 decision 5). Only exact matches are eligible for auto-allow;
/// a command being named `test` or `build` confers nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct CommandProfile {
    entries: Vec<ProfileEntry>,
}

impl CommandProfile {
    /// Builds a profile, dropping duplicates and keeping order.
    pub fn new(entries: Vec<ProfileEntry>) -> Result<CommandProfile, ContextError> {
        // The cap comes first so the de-duplication below is bounded.
        if entries.len() > MAX_PROFILE_ENTRIES {
            return Err(ContextError::ProfileTooLarge);
        }
        let mut unique: Vec<ProfileEntry> = Vec::with_capacity(entries.len());
        for entry in entries {
            if !unique.contains(&entry) {
                unique.push(entry);
            }
        }
        Ok(CommandProfile { entries: unique })
    }

    /// The entries in order.
    pub fn entries(&self) -> &[ProfileEntry] {
        &self.entries
    }

    /// True when `command` run in `cwd` equals an entry exactly.
    pub(crate) fn matches(&self, command: &NormalizedCommand, cwd: &ResolvedPath) -> bool {
        self.entries.iter().any(|e| {
            e.executable == command.executable()
                && e.args == command.args()
                && e.resolved_cwd.same_as(cwd)
        })
    }
}

/// Everything about the situation that the decision depends on, apart from the operation itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyContext {
    root: ResolvedPath,
    trust: TrustState,
    run_mode: RunMode,
    profile: CommandProfile,
}

impl PolicyContext {
    /// Builds a context. `worktree_root` is the assigned worktree, absolute, with links already resolved by the caller.
    pub fn new(
        worktree_root: &str,
        trust: TrustState,
        run_mode: RunMode,
        profile: CommandProfile,
    ) -> Result<PolicyContext, ContextError> {
        let root =
            ResolvedPath::resolve(None, worktree_root).map_err(|_| ContextError::InvalidRoot)?;
        if root.is_prefix_root() {
            // A whole drive or share is never a worktree.
            return Err(ContextError::InvalidRoot);
        }
        for entry in profile.entries() {
            let dir = ResolvedPath::resolve(None, &entry.cwd)
                .map_err(|_| ContextError::InvalidProfileDirectory)?;
            if dir.placement(&root) != Placement::InsideRoot {
                return Err(ContextError::InvalidProfileDirectory);
            }
        }
        Ok(PolicyContext {
            root,
            trust,
            run_mode,
            profile,
        })
    }

    /// The canonical worktree root.
    pub fn worktree_root(&self) -> String {
        self.root.display()
    }

    /// The repository trust state.
    pub fn trust(&self) -> TrustState {
        self.trust
    }

    /// The run mode.
    pub fn run_mode(&self) -> RunMode {
        self.run_mode
    }

    /// The confirmed command profile.
    pub fn profile(&self) -> &CommandProfile {
        &self.profile
    }

    pub(crate) fn root(&self) -> &ResolvedPath {
        &self.root
    }
}
