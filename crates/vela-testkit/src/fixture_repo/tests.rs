//! Fixture builder tests. Every kind is built for real and inspected with real `git` commands.

use super::*;
use serde_json::Value;
use std::collections::BTreeSet;

fn spec() -> Value {
    serde_json::from_str(content::FIXTURE_SPEC).expect("the fixture specification is valid JSON")
}

fn spec_entry(kind: FixtureKind) -> Value {
    spec()["kinds"]
        .as_array()
        .expect("kinds array")
        .iter()
        .find(|k| k["kind"] == kind.name())
        .unwrap_or_else(|| panic!("no spec entry for {}", kind.name()))
        .clone()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("array")
        .iter()
        .map(|v| v.as_str().expect("string").to_owned())
        .collect()
}

fn lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

fn branches(repo: &FixtureRepo) -> Vec<String> {
    lines(
        &repo
            .git(&["for-each-ref", "--format=%(refname:short)", "refs/heads"])
            .unwrap(),
    )
}

fn files_on(repo: &FixtureRepo, rev: &str) -> BTreeSet<String> {
    lines(&repo.git(&["ls-tree", "-r", "--name-only", rev]).unwrap())
        .into_iter()
        .collect()
}

#[test]
fn the_specification_lists_exactly_the_kinds_the_builder_knows() {
    let spec_kinds: BTreeSet<String> = spec()["kinds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["kind"].as_str().unwrap().to_owned())
        .collect();
    let code_kinds: BTreeSet<String> = FixtureKind::ALL
        .iter()
        .map(|k| k.name().to_owned())
        .collect();
    assert_eq!(spec_kinds, code_kinds);
    assert_eq!(spec()["default_branch"], MAIN_BRANCH);
}

#[test]
fn every_kind_matches_its_specification_when_inspected_with_git() {
    for kind in FixtureKind::ALL {
        let repo = FixtureRepo::create(*kind).unwrap();
        let entry = spec_entry(*kind);

        // The repository is real: Git agrees it is a work tree and its default branch is `main`.
        assert_eq!(
            repo.git(&["rev-parse", "--is-inside-work-tree"]).unwrap(),
            "true"
        );
        assert_eq!(
            repo.git(&["symbolic-ref", "--short", "HEAD"]).unwrap(),
            MAIN_BRANCH,
            "{}",
            kind.name()
        );

        let mut want = strings(&entry["branches"]);
        want.sort();
        let mut got = branches(&repo);
        got.sort();
        assert_eq!(got, want, "branches of {}", kind.name());

        let count = repo
            .git(&["rev-list", "--count", MAIN_BRANCH])
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert_eq!(
            Some(count),
            entry["main_commits"].as_u64(),
            "commits on main of {}",
            kind.name()
        );

        let tracked = files_on(&repo, MAIN_BRANCH);
        for file in strings(&entry["files_on_main"]) {
            assert!(
                tracked.contains(&file),
                "{} should track {file}; tracked: {tracked:?}",
                kind.name()
            );
        }

        // A fresh fixture is clean and sits on main.
        assert_eq!(repo.git(&["status", "--porcelain"]).unwrap(), "");
    }
}

#[test]
fn building_the_same_kind_twice_gives_identical_commits() {
    for kind in FixtureKind::ALL {
        let a = FixtureRepo::create(*kind).unwrap();
        let b = FixtureRepo::create(*kind).unwrap();
        assert_ne!(a.path(), b.path(), "two builds use distinct directories");
        assert_eq!(
            a.refs_snapshot().unwrap(),
            b.refs_snapshot().unwrap(),
            "{} is not deterministic",
            kind.name()
        );
        assert_eq!(
            a.git(&["rev-parse", "main^{tree}"]).unwrap(),
            b.git(&["rev-parse", "main^{tree}"]).unwrap()
        );
    }
}

#[test]
fn different_kinds_are_distinguishable() {
    let snapshots: BTreeSet<String> = FixtureKind::ALL
        .iter()
        .map(|k| FixtureRepo::create(*k).unwrap().refs_snapshot().unwrap())
        .collect();
    assert_eq!(snapshots.len(), FixtureKind::ALL.len());
}

#[test]
fn commits_carry_the_fixed_identity_and_a_monotonic_fixed_clock() {
    let repo = FixtureRepo::create(FixtureKind::SeededTickets).unwrap();
    let log = repo
        .git(&[
            "log",
            "--reverse",
            "--format=%an|%ae|%at|%cn|%ce|%ct",
            MAIN_BRANCH,
        ])
        .unwrap();
    let rows = lines(&log);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0],
        format!(
            "Vela Fixture|fixture@vela.invalid|{BASE_EPOCH}|Vela Fixture|fixture@vela.invalid|{BASE_EPOCH}"
        )
    );
    let second = BASE_EPOCH + COMMIT_SPACING_SECONDS;
    assert_eq!(
        rows[1],
        format!(
            "Vela Fixture|fixture@vela.invalid|{second}|Vela Fixture|fixture@vela.invalid|{second}"
        )
    );
}

#[test]
fn the_host_git_configuration_does_not_leak_into_a_fixture() {
    // The empty global configuration plus -c flags decide everything: nothing from the host is inherited.
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    let global = repo.git(&["config", "--global", "--list"]).unwrap();
    assert_eq!(global, "");
    assert_eq!(
        repo.git(&["config", "--get", "core.autocrlf"]).unwrap(),
        "false"
    );
}

#[test]
fn the_multi_package_project_has_four_packages() {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    let tracked = files_on(&repo, MAIN_BRANCH);
    for package in ["core", "util", "app", "store"] {
        assert!(
            tracked.contains(&format!("packages/{package}/package.json")),
            "missing package {package}"
        );
    }
    let root = repo.git(&["show", "main:package.json"]).unwrap();
    assert!(root.contains("\"workspaces\""), "{root}");
}

#[test]
fn seeded_tickets_are_the_markdown_issue_set() {
    let repo = FixtureRepo::create(FixtureKind::SeededTickets).unwrap();
    for (name, text) in content::SEEDED_TICKETS {
        let committed = repo
            .git(&["show", &format!("main:tickets/{name}")])
            .unwrap();
        assert_eq!(committed, content::lf(text).trim_end(), "{name}");
    }
    let two = repo.git(&["show", "main:tickets/TKT-002.md"]).unwrap();
    assert!(two.contains("**Blocked by:** TKT-001"), "{two}");
}

#[test]
fn the_seeded_review_defect_is_one_known_line_on_its_branch() {
    let repo = FixtureRepo::create(FixtureKind::SeededReviewDefect).unwrap();
    let defect = seeded_defect();

    // The diff against main is exactly the one file and one line.
    let numstat = repo
        .git(&[
            "diff",
            "--numstat",
            &format!("{MAIN_BRANCH}...{}", defect.branch),
        ])
        .unwrap();
    assert_eq!(numstat, format!("1\t1\t{}", defect.path));

    // The defective line is at the recorded line number on the branch, and the correct line is there on main.
    let on_branch = repo
        .git(&["show", &format!("{}:{}", defect.branch, defect.path)])
        .unwrap();
    let on_main = repo
        .git(&["show", &format!("{MAIN_BRANCH}:{}", defect.path)])
        .unwrap();
    assert_eq!(
        on_branch.lines().nth(defect.line - 1),
        Some(defect.defective_line)
    );
    assert_eq!(
        on_main.lines().nth(defect.line - 1),
        Some(defect.correct_line)
    );
    assert_ne!(defect.defective_line, defect.correct_line);

    // The branch has a single commit past main.
    assert_eq!(
        repo.git(&[
            "rev-list",
            "--count",
            &format!("{MAIN_BRANCH}..{}", defect.branch)
        ])
        .unwrap(),
        "1"
    );
}

#[test]
fn the_schema_overlap_pair_touches_the_same_schema_and_merges_cleanly() {
    let repo = FixtureRepo::create(FixtureKind::SchemaOverlapPair).unwrap();
    let [a, b] = SCHEMA_BRANCHES;

    let changed = |branch: &str| -> BTreeSet<String> {
        lines(
            &repo
                .git(&["diff", "--name-only", &format!("{MAIN_BRANCH}...{branch}")])
                .unwrap(),
        )
        .into_iter()
        .collect()
    };
    let (changed_a, changed_b) = (changed(a), changed(b));
    let overlap: Vec<&String> = changed_a.intersection(&changed_b).collect();
    assert_eq!(overlap, [&SCHEMA_PATH.to_owned()]);

    // Both add a migration numbered 0002, under different names: a semantic overlap Git does not report.
    let migration = |set: &BTreeSet<String>| -> String {
        set.iter()
            .find(|f| f.contains("migrations/0002_"))
            .cloned()
            .expect("a 0002 migration")
    };
    assert_ne!(migration(&changed_a), migration(&changed_b));

    // Textually the two merge without conflict (the regions of schema.sql they edit are apart).
    repo.git(&["checkout", b]).unwrap();
    let merge = repo.git_output(&["merge", "--no-edit", a]).unwrap();
    assert!(
        merge.status.success(),
        "merge failed: {}",
        String::from_utf8_lossy(&merge.stderr)
    );
    let schema = repo.git(&["show", &format!("HEAD:{SCHEMA_PATH}")]).unwrap();
    assert!(
        schema.contains("owner TEXT") && schema.contains("status TEXT"),
        "{schema}"
    );
}

#[test]
fn the_merge_conflict_pair_really_conflicts() {
    let repo = FixtureRepo::create(FixtureKind::MergeConflictPair).unwrap();
    let [a, b] = CONFLICT_BRANCHES;
    repo.git(&["checkout", a]).unwrap();

    let merge = repo.git_output(&["merge", "--no-edit", b]).unwrap();
    assert!(!merge.status.success(), "the merge should conflict");

    let unmerged = repo
        .git(&["diff", "--name-only", "--diff-filter=U"])
        .unwrap();
    assert_eq!(unmerged, CONFLICT_PATH);
    let staged = repo.git(&["ls-files", "--unmerged"]).unwrap();
    assert_eq!(staged.lines().count(), 3, "base, ours, theirs: {staged}");
    let text = std::fs::read_to_string(repo.path().join(CONFLICT_PATH)).unwrap();
    assert!(
        text.contains("<<<<<<<") && text.contains("=======") && text.contains(">>>>>>>"),
        "{text}"
    );

    repo.git(&["merge", "--abort"]).unwrap();
    assert_eq!(repo.git(&["status", "--porcelain"]).unwrap(), "");
}

/// The configuration keys the hostile repository defines, with the marker each payload writes.
const HOSTILE_KEYS: &[(&str, &str)] = &[
    ("core.fsmonitor", "fsmonitor"),
    ("diff.external", "diff-external"),
    ("diff.hostile.textconv", "textconv"),
    ("filter.hostile.clean", "filter-clean"),
    ("filter.hostile.smudge", "filter-smudge"),
];

#[test]
fn the_hostile_repository_really_contains_the_neutralizable_drivers_and_hooks() {
    let repo = FixtureRepo::create(FixtureKind::HostileLocalConfig).unwrap();

    // Configuration drivers, read back by Git itself.
    for (key, marker) in HOSTILE_KEYS {
        let value = repo.git(&["config", "--local", "--get", *key]).unwrap();
        assert!(
            value.contains(&format!("../markers/{marker}")),
            "{key} = {value}"
        );
    }

    // Hooks exist, are shell scripts, and point at their own marker.
    for hook in HOSTILE_HOOKS {
        let path = repo.path().join(".git").join("hooks").join(hook);
        let script = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("hook {hook} missing at {}: {e}", path.display()));
        assert!(script.starts_with("#!/bin/sh\n"), "{script}");
        assert!(script.contains(&format!("../markers/{hook}")), "{script}");
    }

    // The attributes route the payload file through the hostile driver, and Git agrees.
    let attr = repo
        .git(&["check-attr", "filter", "diff", "--", HOSTILE_PAYLOAD_PATH])
        .unwrap();
    assert!(attr.contains("filter: hostile"), "{attr}");
    assert!(attr.contains("diff: hostile"), "{attr}");

    // Every marker name is accounted for by a key or a hook, and building ran nothing.
    let accounted: BTreeSet<&str> = HOSTILE_KEYS
        .iter()
        .map(|(_, m)| *m)
        .chain(HOSTILE_HOOKS.iter().copied())
        .collect();
    let all: BTreeSet<&str> = HOSTILE_MARKERS.iter().copied().collect();
    assert_eq!(accounted, all);
    assert_eq!(repo.fired_markers().unwrap(), Vec::<String>::new());
}

/// One step of the hostile workout.
enum Step {
    /// A Git command.
    Git(&'static [&'static str]),
    /// A diff-producing Git command; under neutralization it also gets `--no-ext-diff --no-textconv` (an empty
    /// `diff.external` alone makes Git try to spawn an empty program, which is why P10 passes the flags).
    Diff(&'static [&'static str]),
    /// Writes a tracked-path file in the working tree.
    Write(&'static str, &'static str),
}

/// Exercises every Git path that P10 found to execute repository-local code: status (fsmonitor), diff (external
/// diff), log -p (textconv), add (clean filter), commit (pre-commit, reference-transaction), checkout (post-checkout,
/// smudge filter on a file that has to be written), and a real merge commit (post-merge).
const WORKOUT: &[Step] = &[
    Step::Write(HOSTILE_PAYLOAD_PATH, "payload v2\n"),
    Step::Git(&["status", "--porcelain"]),
    Step::Diff(&["diff"]),
    Step::Git(&["add", "-A"]),
    Step::Git(&["commit", "--allow-empty", "-m", "second"]),
    Step::Git(&["checkout", "-b", "other"]),
    Step::Write("data/extra.dat", "extra\n"),
    Step::Git(&["add", "-A"]),
    Step::Git(&["commit", "--allow-empty", "-m", "third"]),
    Step::Diff(&["log", "-p", "-1"]),
    Step::Git(&["checkout", "main"]),
    Step::Git(&["checkout", "other"]),
    Step::Git(&["checkout", "main"]),
    Step::Git(&["merge", "--no-ff", "--no-edit", "other"]),
];

fn run_workout(repo: &FixtureRepo, neutralized: bool) {
    for step in WORKOUT {
        match step {
            Step::Write(path, text) => {
                let target = repo.path().join(path);
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::write(target, text).unwrap();
            }
            Step::Git(args) | Step::Diff(args) => {
                let mut args: Vec<&str> = args.to_vec();
                if neutralized && matches!(step, Step::Diff(_)) {
                    args.extend(["--no-ext-diff", "--no-textconv"]);
                }
                if neutralized {
                    repo.git(&args).unwrap();
                } else {
                    let output = repo.git_unneutralized(&args).unwrap();
                    assert!(
                        output.status.success(),
                        "git {args:?} failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
        }
    }
}

#[test]
fn neutralized_git_never_runs_a_hostile_payload() {
    let repo = FixtureRepo::create(FixtureKind::HostileLocalConfig).unwrap();
    run_workout(&repo, true);
    assert_eq!(
        repo.fired_markers().unwrap(),
        Vec::<String>::new(),
        "a payload ran under the neutralising configuration"
    );
    // The workout really happened (real content, not filtered): main holds the second payload and the merged file.
    assert_eq!(
        repo.git(&["show", &format!("main:{HOSTILE_PAYLOAD_PATH}")])
            .unwrap(),
        "payload v2"
    );
    assert_eq!(repo.git(&["show", "main:data/extra.dat"]).unwrap(), "extra");
    assert_eq!(
        repo.git(&["rev-list", "--merges", "--count", "main"])
            .unwrap(),
        "1"
    );
}

#[test]
fn control_without_neutralizing_every_hostile_payload_does_fire() {
    // Proves the markers detect execution, so the "nothing fired" assertion above means something. The payloads only
    // write marker files inside the fixture's own directory.
    let repo = FixtureRepo::create(FixtureKind::HostileLocalConfig).unwrap();
    run_workout(&repo, false);
    let mut want: Vec<String> = HOSTILE_MARKERS.iter().map(|m| (*m).to_owned()).collect();
    want.sort();
    assert_eq!(repo.fired_markers().unwrap(), want);
}

#[test]
fn a_onedrive_parent_is_rejected_by_the_builder_and_nothing_is_created() {
    let parent = Path::new(r"C:\Users\Nobody\OneDrive\Documents\vela-fixtures");
    match FixtureRepo::builder(FixtureKind::MultiPackage)
        .parent_dir(parent)
        .build()
    {
        Err(FixtureError::Guard(GuardError::Synced { provider, .. })) => {
            assert_eq!(provider, SyncProvider::OneDrive)
        }
        other => panic!("expected the guard to refuse, got {other:?}"),
    }
    assert!(!parent.exists());
}

#[test]
fn every_synced_provider_parent_is_rejected_by_the_builder() {
    for (path, provider) in [
        (
            r"C:\Users\Nobody\OneDrive - Contoso\x",
            SyncProvider::OneDrive,
        ),
        (r"C:\Users\Nobody\Dropbox\x", SyncProvider::Dropbox),
        (r"G:\My Drive\x", SyncProvider::GoogleDrive),
        (r"C:\Users\Nobody\iCloudDrive\x", SyncProvider::ICloud),
    ] {
        match FixtureRepo::builder(FixtureKind::HostileLocalConfig)
            .parent_dir(path)
            .build()
        {
            Err(FixtureError::Guard(GuardError::Synced { provider: got, .. })) => {
                assert_eq!(got, provider, "{path}")
            }
            other => panic!("expected the guard to refuse {path}, got {other:?}"),
        }
    }
}

#[test]
fn a_fixture_lives_outside_the_vela_repository_and_is_removed_on_drop() {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    let root = repo.root().to_path_buf();
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(!root.starts_with(manifest), "{}", root.display());
    assert!(root.join("repo").join(".git").is_dir());
    drop(repo);
    assert!(!root.exists(), "{} was not cleaned up", root.display());
}

#[test]
fn the_neutralizing_arguments_cover_the_p10_set() {
    let args = neutralizing_args().join(" ");
    for needle in [
        "--no-pager",
        "core.fsmonitor=false",
        "core.hooksPath=",
        "core.pager=cat",
        "diff.external=",
        "filter.hostile.clean=",
        "filter.hostile.smudge=",
        "diff.hostile.textconv=",
    ] {
        assert!(args.contains(needle), "missing {needle} in {args}");
    }
}
