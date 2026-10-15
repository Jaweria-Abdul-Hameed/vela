use super::*;

fn snap(version: &str) -> VersionSnapshot {
    VersionSnapshot {
        agy_version: version.into(),
        desktop_version: Some("2.5.0".into()),
        capabilities: ["CAP-01", "CAP-02", "CAP-10"].map(String::from).into(),
        posture: "p1".into(),
    }
}

#[test]
fn a_version_change_blocks_resume_until_redetection_is_confirmed() {
    let mut g = DriftGuard::new(snap("1.3.0"));
    assert_eq!(g.check(&snap("1.3.0")), Gate::Proceed);
    let gate = g.check(&snap("1.3.1"));
    assert!(matches!(&gate, Gate::Blocked(r) if matches!(r[0], DriftReason::AgyVersion { .. })));
    // Still blocked on the next boundary even if the cheap probe now looks the same as the old one.
    assert!(g.is_blocked());
    assert!(matches!(g.check(&snap("1.3.0")), Gate::Blocked(_)));
    g.confirm(snap("1.3.1"));
    assert!(!g.is_blocked());
    assert_eq!(g.check(&snap("1.3.1")), Gate::Proceed);
    // And a later change is noticed against the new baseline.
    assert!(matches!(g.check(&snap("1.4.0")), Gate::Blocked(_)));
}

#[test]
fn capability_and_posture_changes_are_each_drift() {
    let mut now = snap("1.3.0");
    now.capabilities.remove("CAP-10");
    now.capabilities.insert("CAP-12".into());
    now.posture = "p2".into();
    now.desktop_version = None;
    let r = diff(&snap("1.3.0"), &now);
    assert!(r.contains(&DriftReason::Posture) && r.contains(&DriftReason::DesktopVersion));
    assert!(r.contains(&DriftReason::Capabilities {
        lost: vec!["CAP-10".into()],
        gained: vec!["CAP-12".into()]
    }));
    assert!(diff(&snap("1.3.0"), &snap("1.3.0")).is_empty());
}

fn p(pid: u32, parent: u32, started: i64, exe: &str) -> ProcessEntry {
    ProcessEntry {
        pid,
        parent,
        started_ms: started,
        exe: exe.into(),
    }
}

fn rec(worker: &str, pid: u32, started: i64) -> RecordedProcess {
    RecordedProcess {
        worker: worker.into(),
        pid,
        started_ms: started,
        exe: "agy.exe".into(),
    }
}

#[test]
fn only_vela_created_trees_of_stopped_workers_are_killed_and_everything_else_is_untouched() {
    let table = vec![
        p(100, 1, 10_000, "agy.exe"),    // Vela's, worker w1 stopped
        p(101, 100, 10_500, "node.exe"), // its child
        p(102, 101, 11_000, "cmd.exe"),  // grandchild
        p(200, 1, 20_000, "agy.exe"),    // Vela's, worker w2 still running
        p(300, 1, 30_000, "agy.exe"),    // someone else's agy: never recorded
        p(301, 300, 30_100, "node.exe"),
        p(400, 1, 99_000, "agy.exe"), // pid 400 was reused: recorded start was 40_000
    ];
    let recorded = vec![
        rec("w1", 100, 10_000),
        rec("w2", 200, 20_000),
        rec("w3", 400, 40_000),
        rec("w4", 500, 50_000), // gone entirely
    ];
    let running = HashSet::from(["w2".to_owned()]);
    let actions = classify_orphans(&table, &recorded, &running);
    assert_eq!(
        actions,
        vec![
            OrphanAction::Kill {
                worker: "w1".into(),
                root: 100,
                descendants: vec![101, 102]
            },
            OrphanAction::Keep {
                worker: "w2".into(),
                root: 200
            },
        ]
    );
    let touched: Vec<u32> = actions
        .iter()
        .flat_map(|a| match a {
            OrphanAction::Kill {
                root, descendants, ..
            } => {
                let mut v = vec![*root];
                v.extend(descendants);
                v
            }
            OrphanAction::Keep { .. } => vec![],
        })
        .collect();
    assert!(
        !touched.contains(&300) && !touched.contains(&301),
        "a foreign agy is never touched"
    );
    assert!(!touched.contains(&400), "a reused pid is not ours");
}

#[test]
fn a_child_older_than_its_root_is_a_reused_pid_not_a_descendant() {
    let table = vec![p(100, 1, 10_000, "agy.exe"), p(101, 100, 1_000, "node.exe")];
    let actions = classify_orphans(&table, &[rec("w1", 100, 10_000)], &HashSet::new());
    assert_eq!(
        actions,
        vec![OrphanAction::Kill {
            worker: "w1".into(),
            root: 100,
            descendants: vec![]
        }]
    );
}

fn recorded() -> RecordedSession {
    RecordedSession {
        conversation_id: "c-1".into(),
        profile_home: PathBuf::from("C:/p/run/w1"),
        agy_version: "1.3.0".into(),
    }
}

fn seen() -> ObservedSession {
    ObservedSession {
        conversation_present: true,
        profile_intact: true,
        agy_version: "1.3.0".into(),
        resume_supported: true,
    }
}

#[test]
fn the_identity_decision_table_resumes_only_on_a_full_match() {
    assert_eq!(
        verify_session(Some(&recorded()), &seen()),
        ResumeDecision::ResumeSame
    );
    assert!(matches!(
        verify_session(None, &seen()),
        ResumeDecision::StartNew(_)
    ));
    let mut r = recorded();
    r.conversation_id.clear();
    assert!(matches!(
        verify_session(Some(&r), &seen()),
        ResumeDecision::StartNew(_)
    ));
    for mutate in [
        (|s: &mut ObservedSession| s.resume_supported = false) as fn(&mut ObservedSession),
        |s| s.profile_intact = false,
        |s| s.conversation_present = false,
        |s| s.agy_version = "1.3.1".into(),
    ] {
        let mut s = seen();
        mutate(&mut s);
        assert!(matches!(
            verify_session(Some(&recorded()), &s),
            ResumeDecision::StartNew(_)
        ));
    }
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vela-a14-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn stale_profiles_and_spool_of_non_running_workers_are_found_and_cleaned_and_running_ones_are_not()
{
    let root = dir("stale");
    for (run, w) in [("r1", "w1"), ("r1", "w2"), ("r2", "w3")] {
        let home = root.join(run).join(w);
        std::fs::create_dir_all(home.join(".gemini").join("vela-spool")).unwrap();
        std::fs::write(
            home.join(".gemini").join("vela-spool").join("a.jsonl"),
            b"{}",
        )
        .unwrap();
    }
    let running = HashSet::from([("r1".to_owned(), "w2".to_owned())]);
    let stale = scan_profiles(&root, &running, "vela-spool").unwrap();
    let who: Vec<(&str, &str, StaleKind)> = stale
        .iter()
        .map(|a| (a.run.as_str(), a.worker.as_str(), a.kind))
        .collect();
    assert!(
        who.contains(&("r1", "w1", StaleKind::Profile))
            && who.contains(&("r1", "w1", StaleKind::Spool))
    );
    assert!(who.contains(&("r2", "w3", StaleKind::Profile)));
    assert!(
        who.iter().all(|(_, w, _)| *w != "w2"),
        "a running worker is never listed"
    );
    clean(&stale).unwrap();
    assert!(!root.join("r1").join("w1").exists());
    assert!(!root.join("r2").join("w3").exists());
    assert!(
        root.join("r1")
            .join("w2")
            .join(".gemini")
            .join("vela-spool")
            .join("a.jsonl")
            .exists()
    );
    // Scanning a missing root is not an error.
    assert!(
        scan_profiles(&root.join("nope"), &running, "vela-spool")
            .unwrap()
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rules_expire_and_never_outlive_their_worker() {
    let rules = vec![
        TimedRule {
            id: "a".into(),
            worker: "w1".into(),
            expires_ms: 5_000,
        },
        TimedRule {
            id: "b".into(),
            worker: "w1".into(),
            expires_ms: 50_000,
        },
        TimedRule {
            id: "c".into(),
            worker: "w2".into(),
            expires_ms: 50_000,
        },
    ];
    let running = HashSet::from(["w1".to_owned()]);
    assert_eq!(expired_rules(&rules, &running, 10_000), vec!["a", "c"]);
    assert_eq!(
        expired_rules(&rules, &running, 5_000),
        vec!["a", "c"],
        "expiry is inclusive"
    );
}
