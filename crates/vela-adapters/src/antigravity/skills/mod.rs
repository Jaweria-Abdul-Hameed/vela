//! Skills detection and bootstrap (A08).
//!
//! Skills are folders `<root>/.agents/skills/<name>/SKILL.md`. Vela finds them at the project and global locations, records a
//! version (a content digest, plus the source commit when the caller knows one), and can copy missing ones into a project on a
//! separate Vela branch, but only after an explicit approval and never over an existing file. It never touches the user's
//! current branch or working tree and never edits instruction files (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`): those are
//! reported so the user can merge by hand. An isolated worker profile cannot see the user's global skills, so
//! [`provide_to_profile`] copies the named ones into the profile's own `.agents/skills`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{BranchName, CommitSha, WorktreePath};
use vela_domain::ports::{CheckpointMessage, GitAdapter, GitContext, Trailer, WorktreeRequest};

/// Skills the build loop needs.
pub const REQUIRED_SKILLS: &[&str] = &["implement"];
/// Skills that make the loop better but are not required.
pub const OPTIONAL_SKILLS: &[&str] = &["tdd", "code-review"];
/// Instruction files Vela never overwrites.
pub const INSTRUCTION_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md", "GEMINI.md"];
/// The most bytes copied for one skill.
pub const MAX_SKILL_BYTES: u64 = 2 * 1024 * 1024;

/// Where a skill was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    /// In the project (`<project>/.agents/skills`).
    Project,
    /// In the user's global folder (`<home>/.agents/skills`).
    Global,
}

/// A skill found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInstall {
    /// The skill's folder name.
    pub name: String,
    /// Where it was found.
    pub location: Location,
    /// The skill folder.
    pub path: PathBuf,
    /// A digest of `SKILL.md`, recorded as the installed version.
    pub version: String,
    /// The source commit, when the caller knows it (the repository the skills came from).
    pub source_commit: Option<String>,
}

/// What detection found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillReport {
    /// Skills found (project first, then global; a project skill shadows a global one of the same name).
    pub found: Vec<SkillInstall>,
    /// Required and optional skills that are nowhere.
    pub missing: Vec<String>,
    /// Instruction files present in the project (never overwritten).
    pub instruction_files: Vec<String>,
}

impl SkillReport {
    /// Required skills that are missing.
    pub fn missing_required(&self) -> Vec<&str> {
        self.missing
            .iter()
            .map(String::as_str)
            .filter(|m| REQUIRED_SKILLS.contains(m))
            .collect()
    }

    /// True when every required skill was found.
    pub fn ready(&self) -> bool {
        self.missing_required().is_empty()
    }
}

/// FNV-1a over the bytes, 16 hex digits.
fn digest(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The `name` in a `SKILL.md` front matter, if any.
pub fn front_matter_name(text: &str) -> Option<String> {
    let body = text.strip_prefix("---")?;
    let end = body.find("\n---")?;
    body[..end]
        .lines()
        .find_map(|l| l.trim().strip_prefix("name:"))
        .map(|n| n.trim().trim_matches(['"', '\'']).to_owned())
        .filter(|n| !n.is_empty())
}

fn read_skill(dir: &Path, location: Location, source_commit: Option<&str>) -> Option<SkillInstall> {
    let name = dir.file_name()?.to_str()?.to_owned();
    let text = std::fs::read(dir.join("SKILL.md")).ok()?;
    Some(SkillInstall {
        name,
        location,
        path: dir.to_path_buf(),
        version: digest(&text),
        source_commit: source_commit.map(str::to_owned),
    })
}

fn list_skills(root: &Path, location: Location, source_commit: Option<&str>) -> Vec<SkillInstall> {
    let dir = root.join(".agents").join("skills");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<SkillInstall> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| read_skill(&e.path(), location, source_commit))
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Detects skills at the project and global locations and reports what is missing.
pub fn detect(project_root: &Path, global_home: &Path, source_commit: Option<&str>) -> SkillReport {
    let mut found = list_skills(project_root, Location::Project, source_commit);
    let project_names: BTreeSet<String> = found.iter().map(|s| s.name.clone()).collect();
    found.extend(
        list_skills(global_home, Location::Global, source_commit)
            .into_iter()
            .filter(|s| !project_names.contains(&s.name)),
    );
    let have: BTreeSet<&str> = found.iter().map(|s| s.name.as_str()).collect();
    let missing = REQUIRED_SKILLS
        .iter()
        .chain(OPTIONAL_SKILLS)
        .filter(|n| !have.contains(**n))
        .map(|n| (*n).to_owned())
        .collect();
    let instruction_files = INSTRUCTION_FILES
        .iter()
        .filter(|f| project_root.join(f).is_file())
        .map(|f| (*f).to_owned())
        .collect();
    SkillReport {
        found,
        missing,
        instruction_files,
    }
}

/// Whether a skill is usable: a readable `SKILL.md` whose front matter names the folder and has a body. This is a static
/// check; running a skill (which may act on the repository) is left to the gated real-environment suite.
pub fn verify_available(skill: &SkillInstall) -> Result<(), VelaError> {
    let text = std::fs::read_to_string(skill.path.join("SKILL.md")).map_err(|e| {
        VelaError::of_class(
            ErrorClass::ValidationFailure,
            format!("skill `{}` is not readable: {e}", skill.name),
        )
    })?;
    match front_matter_name(&text) {
        Some(name) if name == skill.name => {}
        Some(other) => {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!("skill folder `{}` declares the name `{other}`", skill.name),
            ));
        }
        None => {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!("skill `{}` has no front matter name", skill.name),
            ));
        }
    }
    let body_start = text.match_indices("\n---").next().map_or(0, |(i, _)| i + 4);
    if text[body_start.min(text.len())..].trim().is_empty() {
        return Err(VelaError::of_class(
            ErrorClass::ValidationFailure,
            format!("skill `{}` has no instructions", skill.name),
        ));
    }
    Ok(())
}

/// What a copy did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CopyReport {
    /// Files created.
    pub copied: Vec<PathBuf>,
    /// Files that already existed and were left exactly as they were.
    pub skipped_existing: Vec<PathBuf>,
}

fn dir_size(dir: &Path) -> std::io::Result<u64> {
    let mut total = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        total += if meta.is_dir() {
            dir_size(&entry.path())?
        } else {
            meta.len()
        };
    }
    Ok(total)
}

fn copy_tree(src: &Path, dst: &Path, report: &mut CopyReport) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let meta = entry.file_type()?;
        let target = dst.join(&name);
        if meta.is_symlink() {
            continue; // links are never followed
        }
        if meta.is_dir() {
            copy_tree(&entry.path(), &target, report)?;
        } else if target.exists() {
            report.skipped_existing.push(target);
        } else {
            std::fs::copy(entry.path(), &target)?;
            report.copied.push(target);
        }
    }
    Ok(())
}

/// Copies `skill` into `<dest_root>/.agents/skills/<name>`, never overwriting a file.
pub fn copy_skill(skill: &SkillInstall, dest_root: &Path) -> Result<CopyReport, VelaError> {
    copy_skill_to(skill, &dest_root.join(".agents").join("skills"))
}

/// Copies `skill` into `<skills_dir>/<name>`, never overwriting a file.
pub fn copy_skill_to(skill: &SkillInstall, skills_dir: &Path) -> Result<CopyReport, VelaError> {
    let io = |e: std::io::Error| {
        VelaError::of_class(
            ErrorClass::ExternalTemporary,
            format!("could not copy skill `{}`: {e}", skill.name),
        )
    };
    if dir_size(&skill.path).map_err(io)? > MAX_SKILL_BYTES {
        return Err(VelaError::of_class(
            ErrorClass::ValidationFailure,
            format!(
                "skill `{}` is larger than {MAX_SKILL_BYTES} bytes",
                skill.name
            ),
        ));
    }
    let mut report = CopyReport::default();
    copy_tree(&skill.path, &skills_dir.join(&skill.name), &mut report).map_err(io)?;
    Ok(report)
}

/// The plugin name Vela uses to deliver skills to an isolated profile.
pub const PROFILE_PLUGIN: &str = "vela-skills";

/// Makes the named skills visible to an isolated worker profile. The CLI reads global skills from enabled plugins (verified:
/// `~/.gemini/config/plugins/<plugin>/skills/<skill>/SKILL.md` with the plugin enabled in `config.json`), so Vela writes a
/// small plugin into the profile and enables it. Returns the names provided.
pub fn provide_to_profile(
    profile_home: &Path,
    available: &[SkillInstall],
    names: &[&str],
) -> Result<Vec<String>, VelaError> {
    let io = |e: std::io::Error| {
        VelaError::of_class(
            ErrorClass::ExternalTemporary,
            format!("could not write the skills plugin: {e}"),
        )
    };
    let config_dir = profile_home.join(".gemini").join("config");
    let plugin_dir = config_dir.join("plugins").join(PROFILE_PLUGIN);
    let skills_dir = plugin_dir.join("skills");
    let mut provided = Vec::new();
    for name in names {
        if let Some(skill) = available.iter().find(|s| s.name == *name) {
            copy_skill_to(skill, &skills_dir)?;
            provided.push((*name).to_owned());
        }
    }
    if provided.is_empty() {
        return Ok(provided);
    }
    std::fs::create_dir_all(&plugin_dir).map_err(io)?;
    std::fs::write(
        plugin_dir.join("plugin.json"),
        format!("{{\"name\":\"{PROFILE_PLUGIN}\",\"displayName\":\"Vela skills\",\"version\":\"1.0.0\",\"description\":\"Skills Vela provides to this worker\"}}"),
    )
    .map_err(io)?;
    let config_path = config_dir.join("config.json");
    let mut config: serde_json::Value = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    config["plugins"][PROFILE_PLUGIN] = serde_json::json!({ "enabled": true });
    std::fs::write(
        &config_path,
        serde_json::to_vec_pretty(&config).unwrap_or_default(),
    )
    .map_err(io)?;
    Ok(provided)
}

/// Proof that the user approved installing skills. It can only be made by [`Approval::granted_by_user`], which the
/// orchestrator calls after the user's explicit action; nothing else in the crate creates one.
#[derive(Debug, Clone)]
pub struct Approval(());

impl Approval {
    /// The user explicitly approved the bootstrap.
    pub fn granted_by_user() -> Self {
        Self(())
    }
}

/// The result of a bootstrap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapOutcome {
    /// The separate branch the skills were committed on.
    pub branch: BranchName,
    /// The commit that holds them.
    pub commit: CommitSha,
    /// The skills installed.
    pub installed: Vec<String>,
    /// Files that already existed and were left alone.
    pub skipped_existing: Vec<PathBuf>,
    /// Instruction files present (reported, never edited).
    pub instruction_files: Vec<String>,
}

/// Installs `skills` into the project on a new branch in a managed worktree created from `base`, then commits them there. The
/// user's current branch and working tree are never touched. `worktree` must be inside the managed root.
#[allow(clippy::too_many_arguments)]
pub async fn bootstrap(
    git: &dyn GitAdapter,
    ctx: &GitContext,
    _approval: &Approval,
    repo: &WorktreePath,
    worktree: &WorktreePath,
    branch: &BranchName,
    base: &CommitSha,
    skills: &[SkillInstall],
    instruction_files: Vec<String>,
) -> Result<BootstrapOutcome, VelaError> {
    if skills.is_empty() {
        return Err(VelaError::of_class(
            ErrorClass::ValidationFailure,
            "there are no skills to install",
        ));
    }
    let request = WorktreeRequest {
        repo: repo.clone(),
        path: worktree.clone(),
        branch: branch.clone(),
        base: base.clone(),
    };
    git.add_worktree(ctx, &request).await?;
    let mut skipped = Vec::new();
    let mut installed = Vec::new();
    for skill in skills {
        let report = copy_skill(skill, Path::new(worktree.as_str()))?;
        skipped.extend(report.skipped_existing);
        installed.push(skill.name.clone());
    }
    let message = CheckpointMessage::new(
        format!("chore(vela): install skills {}", installed.join(", ")),
        vec![
            Trailer::new("Vela-Bootstrap", "skills")
                .map_err(|e| VelaError::internal(e.to_string()))?,
        ],
    )
    .map_err(|e| VelaError::internal(e.to_string()))?;
    let commit = git.commit_checkpoint(ctx, worktree, &message).await?;
    Ok(BootstrapOutcome {
        branch: branch.clone(),
        commit,
        installed,
        skipped_existing: skipped,
        instruction_files,
    })
}

#[cfg(test)]
mod tests;
