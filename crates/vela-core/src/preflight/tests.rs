use std::sync::Arc;

use super::*;
use crate::orchestrator::test_support::env;

fn healthy() -> PreflightFacts {
    PreflightFacts {
        repo_path: "C:/work/app".into(),
        is_git_repo: true,
        head_detached: false,
        operation_in_progress: false,
        dirty_paths: 0,
        default_branch: Some("main".into()),
        has_remote: true,
        git_version: Some((2, 45, 0)),
        gh: GhState::Ready,
        docs: vec!["AGENTS.md".into()],
        trust: TrustState::Trusted,
        toolchains: vec!["Cargo.toml".into()],
        in_synced_folder: false,
        longest_worktree_path: 120,
        has_submodules: false,
        autocrlf: None,
        has_gitattributes: true,
        webview2: Some("130.0".into()),
        free_disk_bytes: Some(500_000_000_000),
        planned_worktrees: 4,
        bytes_per_worktree: 1_000_000_000,
    }
}

type Change = Box<dyn Fn(&mut PreflightFacts)>;

fn verdict(f: &PreflightFacts) -> (Severity, Vec<String>) {
    let e = env("verdict");
    e.seed_project();
    let r = PreflightEngine::new(e.services.clone())
        .run("p1", f)
        .unwrap();
    let ids = r
        .checks
        .iter()
        .filter(|c| c.severity != Severity::Pass)
        .map(|c| c.id.clone())
        .collect();
    (r.verdict, ids)
}

#[test]
fn a_healthy_project_passes_with_nothing_to_fix() {
    let (v, issues) = verdict(&healthy());
    assert_eq!(v, Severity::Pass);
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn each_severity_row_produces_its_level_and_a_remediation() {
    let cases: Vec<(&str, Change, Severity)> = vec![
        (
            "repo.git",
            Box::new(|f| f.is_git_repo = false),
            Severity::Block,
        ),
        (
            "repo.operation",
            Box::new(|f| f.operation_in_progress = true),
            Severity::Block,
        ),
        (
            "trust.state",
            Box::new(|f| f.trust = TrustState::Untrusted),
            Severity::Block,
        ),
        (
            "git.version",
            Box::new(|f| f.git_version = None),
            Severity::Block,
        ),
        (
            "git.version",
            Box::new(|f| f.git_version = Some((2, 20, 0))),
            Severity::Block,
        ),
        (
            "git.version",
            Box::new(|f| f.git_version = Some((2, 30, 0))),
            Severity::Warn,
        ),
        (
            "win.synced",
            Box::new(|f| f.in_synced_folder = true),
            Severity::Block,
        ),
        (
            "win.path",
            Box::new(|f| f.longest_worktree_path = 250),
            Severity::Block,
        ),
        (
            "win.path",
            Box::new(|f| f.longest_worktree_path = 210),
            Severity::Warn,
        ),
        (
            "win.webview2",
            Box::new(|f| f.webview2 = None),
            Severity::Block,
        ),
        (
            "win.disk",
            Box::new(|f| f.free_disk_bytes = Some(5_000_000_000)),
            Severity::Block,
        ),
        (
            "win.disk",
            Box::new(|f| f.free_disk_bytes = Some(10_000_000_000)),
            Severity::Warn,
        ),
        (
            "repo.head",
            Box::new(|f| f.head_detached = true),
            Severity::Warn,
        ),
        (
            "repo.dirty",
            Box::new(|f| f.dirty_paths = 3),
            Severity::Warn,
        ),
        (
            "repo.default_branch",
            Box::new(|f| f.default_branch = None),
            Severity::Warn,
        ),
        (
            "git.remote",
            Box::new(|f| f.has_remote = false),
            Severity::Warn,
        ),
        (
            "git.gh",
            Box::new(|f| f.gh = GhState::Missing),
            Severity::Warn,
        ),
        (
            "git.gh",
            Box::new(|f| f.gh = GhState::Unauthenticated),
            Severity::Warn,
        ),
        ("docs.agents", Box::new(|f| f.docs.clear()), Severity::Warn),
        (
            "toolchain.manifest",
            Box::new(|f| f.toolchains.clear()),
            Severity::Warn,
        ),
        (
            "win.submodules",
            Box::new(|f| f.has_submodules = true),
            Severity::Warn,
        ),
        (
            "win.eol",
            Box::new(|f| {
                f.autocrlf = Some("true".into());
                f.has_gitattributes = false;
            }),
            Severity::Warn,
        ),
    ];
    for (id, change, expected) in cases {
        let mut f = healthy();
        change(&mut f);
        let e = env("row");
        e.seed_project();
        let r = PreflightEngine::new(e.services.clone())
            .run("p1", &f)
            .unwrap();
        let c = r
            .checks
            .iter()
            .find(|c| c.id == id)
            .unwrap_or_else(|| panic!("{id} missing"));
        assert_eq!(c.severity, expected, "{id}: {}", c.detail);
        assert!(c.remediation.is_some(), "{id} has no remediation");
        assert_eq!(r.verdict, expected, "{id}");
    }
}

#[test]
fn the_verdict_is_the_worst_finding() {
    let mut f = healthy();
    f.dirty_paths = 1;
    f.in_synced_folder = true;
    let (v, issues) = verdict(&f);
    assert_eq!(v, Severity::Block);
    assert_eq!(issues, ["repo.dirty", "win.synced"]);
}

struct Extra;

impl CheckProvider for Extra {
    fn id(&self) -> &'static str {
        "antigravity"
    }
    fn run(&self, _f: &PreflightFacts) -> Vec<Check> {
        vec![Check::new(
            "agy.signed_in",
            "Antigravity signed in",
            Severity::Block,
            "not signed in",
            Some("sign in"),
        )]
    }
}

#[test]
fn later_tickets_add_providers_without_touching_the_engine() {
    let e = env("provider");
    e.seed_project();
    let engine = PreflightEngine::new(e.services.clone());
    assert_eq!(
        engine.run("p1", &healthy()).unwrap().verdict,
        Severity::Pass
    );
    engine.register(Arc::new(Extra));
    let r = engine.run("p1", &healthy()).unwrap();
    assert_eq!(r.verdict, Severity::Block);
    assert_eq!(r.blockers().count(), 1);
}

#[test]
fn reports_are_kept_as_history_newest_first() {
    let e = env("history");
    e.seed_project();
    let engine = PreflightEngine::new(e.services.clone());
    let first = engine.run("p1", &healthy()).unwrap();
    e.clock.advance(1000);
    let mut bad = healthy();
    bad.is_git_repo = false;
    let second = engine.run("p1", &bad).unwrap();
    let history = engine.history("p1", 10).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].id, second.id);
    assert_eq!(history[1], first);
    assert!(engine.history("other", 10).unwrap().is_empty());
}

#[test]
fn local_facts_are_read_as_data() {
    let dir = std::env::temp_dir().join(format!("vela-pf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::write(dir.join(".git/MERGE_HEAD"), "abc").unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[package]").unwrap();
    std::fs::write(dir.join("AGENTS.md"), "x").unwrap();
    std::fs::write(dir.join(".gitmodules"), "").unwrap();
    let f = collect_local(&dir);
    assert!(f.is_git_repo && f.operation_in_progress && f.has_submodules && !f.head_detached);
    assert_eq!(f.toolchains, ["Cargo.toml"]);
    assert!(f.docs.contains(&"AGENTS.md".to_owned()));
    std::fs::write(dir.join(".git/HEAD"), "0123456789abcdef\n").unwrap();
    assert!(collect_local(&dir).head_detached);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!collect_local(&dir).is_git_repo);
    assert!(collect_local(Path::new("C:/Users/x/OneDrive/app")).in_synced_folder);
}
