//! The hostile local-config repository: a `.git` whose own configuration and hooks name programs that would write a
//! marker file if Git ever ran them (the verified P10 exposure: `core.fsmonitor`, `diff.external`, a textconv driver,
//! clean/smudge filters, and `pre-commit`, `post-checkout`, `reference-transaction`, `post-merge` hooks).
//!
//! Building this repository never runs any payload: the repository is created and committed first (with the
//! neutralising configuration), and only then are the hostile configuration lines and hook files written, as plain file
//! writes. Every payload is `echo fired > ../markers/<name>` run from the working-tree root, so the only effect of a
//! payload that does run is a marker file in the fixture's own `markers` directory.

use super::FixtureError;
use super::git::{GitRepo, HOSTILE_DRIVER};
use std::io::Write;
use std::path::Path;

/// Marker file names, one per payload. A marker exists exactly when its payload ran.
pub const HOSTILE_MARKERS: &[&str] = &[
    "fsmonitor",
    "diff-external",
    "textconv",
    "filter-clean",
    "filter-smudge",
    "pre-commit",
    "post-checkout",
    "reference-transaction",
    "post-merge",
];

/// The hooks the fixture installs.
pub const HOSTILE_HOOKS: &[&str] = &[
    "pre-commit",
    "post-checkout",
    "reference-transaction",
    "post-merge",
];

fn payload(marker: &str) -> String {
    format!("echo fired > ../markers/{marker}")
}

/// The hostile lines appended to `.git/config`.
pub(crate) fn hostile_config() -> String {
    format!(
        "[core]\n\tfsmonitor = {fsmonitor}\n[diff]\n\texternal = {external}\n[diff \"{d}\"]\n\ttextconv = {textconv}\n[filter \"{d}\"]\n\tclean = {clean}\n\tsmudge = {smudge}\n",
        fsmonitor = payload("fsmonitor"),
        external = payload("diff-external"),
        textconv = payload("textconv"),
        clean = payload("filter-clean"),
        smudge = payload("filter-smudge"),
        d = HOSTILE_DRIVER,
    )
}

/// Installs the hostile configuration and hooks into the repository at `git.dir`, and creates `markers_dir`.
/// Plain file writes only: no Git command runs.
pub(crate) fn install(git: &GitRepo, markers_dir: &Path) -> Result<(), FixtureError> {
    std::fs::create_dir_all(markers_dir)
        .map_err(|source| FixtureError::io(format!("create {}", markers_dir.display()), source))?;
    let git_dir = git.dir.join(".git");
    let config_path = git_dir.join("config");
    let mut config = std::fs::OpenOptions::new()
        .append(true)
        .open(&config_path)
        .map_err(|source| FixtureError::io(format!("open {}", config_path.display()), source))?;
    config
        .write_all(hostile_config().as_bytes())
        .map_err(|source| FixtureError::io(format!("write {}", config_path.display()), source))?;
    drop(config);

    let hooks_dir = git_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir)
        .map_err(|source| FixtureError::io(format!("create {}", hooks_dir.display()), source))?;
    for hook in HOSTILE_HOOKS {
        let path = hooks_dir.join(hook);
        let script = format!("#!/bin/sh\n{}\nexit 0\n", payload(hook));
        std::fs::write(&path, script.as_bytes())
            .map_err(|source| FixtureError::io(format!("write {}", path.display()), source))?;
        make_executable(&path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), FixtureError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|source| FixtureError::io(format!("chmod {}", path.display()), source))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), FixtureError> {
    // Git for Windows runs hooks through its own `sh` by shebang; there is no executable bit to set.
    Ok(())
}
