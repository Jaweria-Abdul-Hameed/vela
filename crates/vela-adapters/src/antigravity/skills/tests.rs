use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use vela_domain::ids::{BranchName, CommitSha, OperationId, WorktreePath};
use vela_domain::ports::{GitContext, RepoTrust};
use vela_testkit::fakes::FakeGit;

use super::*;

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "vela-skills-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn skill(&self, root: &str, name: &str, body: &str) -> PathBuf {
        let dir = self.0.join(root).join(".agents").join("skills").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test\n---\n\n{body}\n"),
        )
        .unwrap();
        dir
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn skills_are_found_at_project_and_global_locations_and_missing_ones_reported() {
    let d = Dir::new("detect");
    d.skill("project", "implement", "project implement");
    d.skill("home", "implement", "global implement");
    d.skill("home", "tdd", "global tdd");
    std::fs::write(d.0.join("project").join("AGENTS.md"), "rules").unwrap();
    let report = detect(&d.0.join("project"), &d.0.join("home"), Some("abc123"));
    let names: Vec<(&str, Location)> = report
        .found
        .iter()
        .map(|s| (s.name.as_str(), s.location))
        .collect();
    assert_eq!(
        names,
        [("implement", Location::Project), ("tdd", Location::Global)],
        "the project skill shadows the global one"
    );
    assert_eq!(report.missing, vec!["code-review".to_owned()]);
    assert!(report.ready());
    assert_eq!(report.instruction_files, vec!["AGENTS.md".to_owned()]);
    assert_eq!(report.found[0].source_commit.as_deref(), Some("abc123"));
    assert_eq!(report.found[0].version.len(), 16);
}

#[test]
fn a_missing_required_skill_makes_the_report_not_ready() {
    let d = Dir::new("missing");
    std::fs::create_dir_all(d.0.join("project")).unwrap();
    let report = detect(&d.0.join("project"), &d.0.join("home"), None);
    assert!(!report.ready());
    assert_eq!(report.missing_required(), vec!["implement"]);
}

#[test]
fn the_version_changes_when_the_skill_changes() {
    let d = Dir::new("version");
    let p = d.skill("project", "implement", "v1");
    let v1 = detect(&d.0.join("project"), &d.0.join("home"), None).found[0]
        .version
        .clone();
    std::fs::write(p.join("SKILL.md"), "---\nname: implement\n---\n\nv2\n").unwrap();
    let v2 = detect(&d.0.join("project"), &d.0.join("home"), None).found[0]
        .version
        .clone();
    assert_ne!(v1, v2);
}

#[test]
fn availability_is_checked_statically() {
    let d = Dir::new("verify");
    d.skill("project", "implement", "do the work");
    let ok = detect(&d.0.join("project"), &d.0.join("home"), None)
        .found
        .remove(0);
    verify_available(&ok).unwrap();
    // Name mismatch.
    std::fs::write(ok.path.join("SKILL.md"), "---\nname: other\n---\n\nbody\n").unwrap();
    assert!(
        verify_available(&ok)
            .unwrap_err()
            .what_failed()
            .contains("declares the name")
    );
    // No front matter.
    std::fs::write(ok.path.join("SKILL.md"), "just text").unwrap();
    assert!(verify_available(&ok).is_err());
    // No body.
    std::fs::write(ok.path.join("SKILL.md"), "---\nname: implement\n---\n").unwrap();
    assert!(
        verify_available(&ok)
            .unwrap_err()
            .what_failed()
            .contains("no instructions")
    );
    // Gone.
    std::fs::remove_file(ok.path.join("SKILL.md")).unwrap();
    assert!(verify_available(&ok).is_err());
}

#[test]
fn copying_never_overwrites_an_existing_file_and_skips_links_and_git_data() {
    let d = Dir::new("copy");
    let src = d.skill("home", "implement", "fresh");
    std::fs::create_dir_all(src.join("scripts")).unwrap();
    std::fs::write(src.join("scripts").join("run.ps1"), "echo hi").unwrap();
    std::fs::create_dir_all(src.join(".git")).unwrap();
    std::fs::write(src.join(".git").join("HEAD"), "ref").unwrap();
    let skill = detect(&d.0.join("none"), &d.0.join("home"), None)
        .found
        .remove(0);
    let dest = d.0.join("project");
    // A file that already exists in the destination.
    let existing = dest.join(".agents").join("skills").join("implement");
    std::fs::create_dir_all(&existing).unwrap();
    std::fs::write(existing.join("SKILL.md"), "the user's own version").unwrap();
    let report = copy_skill(&skill, &dest).unwrap();
    assert_eq!(
        std::fs::read_to_string(existing.join("SKILL.md")).unwrap(),
        "the user's own version"
    );
    assert_eq!(report.skipped_existing.len(), 1);
    assert!(existing.join("scripts").join("run.ps1").exists());
    assert!(!existing.join(".git").exists());
    // An oversized skill is refused.
    std::fs::write(
        src.join("big.bin"),
        vec![0u8; usize::try_from(MAX_SKILL_BYTES).unwrap() + 1],
    )
    .unwrap();
    assert!(copy_skill(&skill, &d.0.join("other")).is_err());
}

#[test]
fn global_skills_are_provided_to_an_isolated_profile_by_name() {
    let d = Dir::new("profile");
    d.skill("home", "implement", "x");
    d.skill("home", "grill-me", "y");
    let global = detect(&d.0.join("none"), &d.0.join("home"), None).found;
    let profile_home = d.0.join("profile-home");
    let provided =
        provide_to_profile(&profile_home, &global, &["implement", "not-installed"]).unwrap();
    assert_eq!(provided, vec!["implement".to_owned()]);
    let plugin = profile_home.join(".gemini/config/plugins/vela-skills");
    assert!(plugin.join("skills/implement/SKILL.md").exists());
    assert!(plugin.join("plugin.json").exists());
    assert!(
        !plugin.join("skills/grill-me").exists(),
        "only the named skills are provided"
    );
    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(profile_home.join(".gemini/config/config.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(config["plugins"]["vela-skills"]["enabled"], true);
    // Nothing provided, nothing written.
    let empty_home = d.0.join("empty-home");
    assert!(
        provide_to_profile(&empty_home, &global, &["nope"])
            .unwrap()
            .is_empty()
    );
    assert!(!empty_home.exists());
}

fn ctx() -> GitContext {
    GitContext {
        trust: RepoTrust::Trusted,
        operation: OperationId::new("op-skills").unwrap(),
    }
}

#[tokio::test]
async fn bootstrap_installs_on_a_separate_branch_in_a_managed_worktree_and_records_the_commit() {
    let d = Dir::new("bootstrap");
    d.skill("home", "implement", "x");
    let skills = detect(&d.0.join("none"), &d.0.join("home"), None).found;
    let git = FakeGit::new();
    let wt = d.0.join("managed").join("skills");
    std::fs::create_dir_all(&wt).unwrap();
    let outcome = bootstrap(
        &git,
        &ctx(),
        &Approval::granted_by_user(),
        &WorktreePath::new("C:/repo").unwrap(),
        &WorktreePath::new(wt.to_string_lossy().replace('\\', "/")).unwrap(),
        &BranchName::new("vela/bootstrap-skills").unwrap(),
        &CommitSha::new("a".repeat(40)).unwrap(),
        &skills,
        vec!["AGENTS.md".to_owned()],
    )
    .await
    .unwrap();
    assert_eq!(outcome.branch.as_str(), "vela/bootstrap-skills");
    assert_eq!(outcome.installed, vec!["implement".to_owned()]);
    assert_eq!(outcome.instruction_files, vec!["AGENTS.md".to_owned()]);
    assert!(wt.join(".agents/skills/implement/SKILL.md").exists());
    let calls = git.log().calls();
    assert!(calls.iter().any(|c| c.starts_with("add_worktree:")));
    assert!(
        calls
            .iter()
            .any(|c| c.starts_with("checkpoint:") && c.contains("install skills implement"))
    );
    assert!(
        !calls
            .iter()
            .any(|c| c.starts_with("merge") || c.starts_with("push")),
        "only a worktree and a commit: {calls:?}"
    );
}

#[tokio::test]
async fn bootstrap_with_nothing_to_install_is_an_error() {
    let git = FakeGit::new();
    let err = bootstrap(
        &git,
        &ctx(),
        &Approval::granted_by_user(),
        &WorktreePath::new("C:/repo").unwrap(),
        &WorktreePath::new("C:/managed/s").unwrap(),
        &BranchName::new("vela/b").unwrap(),
        &CommitSha::new("a".repeat(40)).unwrap(),
        &[],
        vec![],
    )
    .await
    .unwrap_err();
    assert!(err.what_failed().contains("no skills"));
}

#[test]
fn skills_never_edit_instruction_files() {
    let source = include_str!("mod.rs");
    let production = source.split("#[cfg(test)]").next().unwrap();
    // It names the files to report them; nothing opens one for writing.
    assert!(!production.contains("OpenOptions"));
    assert!(production.contains("INSTRUCTION_FILES"));
}
