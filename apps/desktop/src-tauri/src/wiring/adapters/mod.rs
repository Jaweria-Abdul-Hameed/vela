//! The concrete adapters (W01), built once at startup: the process runner with tree kill, the Git adapter under its managed
//! worktree root, the Antigravity driver and its profile manager. vela-core never names any of them; this is the only place they
//! meet.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use vela_adapters::antigravity::profile::ProfileManager;
use vela_adapters::antigravity::session::AgyDriver;
use vela_git::read::GitCli;
use vela_process::redact::{PatternRedactor, SharedRedactor};
use vela_process::runner::{TokioProcessRunner, baseline_env};
use vela_process::tree::TreeRegistry;

/// File name of the hook binary that ships next to the application.
#[cfg(windows)]
pub const HOOK_FILE: &str = "vela-hook.exe";
/// File name of the hook binary that ships next to the application.
#[cfg(not(windows))]
pub const HOOK_FILE: &str = "vela-hook";

/// Where `vela-hook` is: next to the running executable, which is the installed layout and also where Cargo puts it in
/// development. One function, used by the profile generator.
pub fn hook_exe_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(HOOK_FILE)))
        .unwrap_or_else(|| PathBuf::from(HOOK_FILE))
}

/// Everything concrete.
pub struct Adapters {
    /// Runs processes, redacts their output, kills their trees.
    pub runner: Arc<TokioProcessRunner>,
    /// Git, with worktrees confined below the managed root.
    pub git: Arc<GitCli>,
    /// The Antigravity CLI.
    pub agy: Arc<AgyDriver>,
    /// Isolated per-worker Antigravity profiles.
    pub profiles: ProfileManager,
    /// The data root.
    pub data_root: PathBuf,
}

/// Builds the adapters under `data_root`.
pub fn build(data_root: &Path) -> Adapters {
    let redactor: SharedRedactor = Arc::new(PatternRedactor::new());
    let runner = Arc::new(TokioProcessRunner::new(
        redactor,
        Arc::new(TreeRegistry::new()),
    ));
    let git = Arc::new(
        GitCli::new(runner.clone(), baseline_env()).with_managed_root(data_root.join("worktrees")),
    );
    let agy = Arc::new(AgyDriver::new(runner.clone(), baseline_env()));
    Adapters {
        runner,
        git,
        agy,
        profiles: ProfileManager::new(data_root.join("profiles"), hook_exe_path()),
        data_root: data_root.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hook_is_looked_for_next_to_the_executable() {
        let p = hook_exe_path();
        assert!(p.file_name().is_some_and(|n| n == HOOK_FILE));
        assert!(p.parent().is_some());
    }

    #[test]
    fn adapters_are_built_under_the_data_root() {
        let root = std::env::temp_dir().join(format!("vela-adapters-{}", std::process::id()));
        let a = build(&root);
        assert_eq!(a.data_root, root);
        assert_eq!(a.git.managed_root(), Some(&root.join("worktrees")));
    }
}
