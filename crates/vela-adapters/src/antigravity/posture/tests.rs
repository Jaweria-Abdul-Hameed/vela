use std::collections::BTreeMap;

use vela_domain::ports::{ConversationId, SessionRef, SessionStatus, TurnResult};

use super::*;

struct Scripted(BTreeMap<Scenario, Result<Observation, &'static str>>);

impl ScenarioRunner for Scripted {
    fn run<'a>(&'a self, scenario: Scenario) -> PortFuture<'a, Observation> {
        let r = self.0[&scenario];
        Box::pin(async move { r.map_err(VelaError::internal) })
    }
}

fn obs(executed: bool, denied: bool) -> Result<Observation, &'static str> {
    Ok(Observation { executed, denied })
}

fn correct() -> BTreeMap<Scenario, Result<Observation, &'static str>> {
    BTreeMap::from([
        (Scenario::DenyBlocks, obs(false, true)),
        (Scenario::UnlistedDenied, obs(false, true)),
        (Scenario::AllowRuns, obs(true, false)),
        (Scenario::HookFailureBlocks, obs(false, true)),
    ])
}

#[tokio::test]
async fn four_correct_behaviours_meet_the_specification() {
    let r = probe(&Scripted(correct())).await;
    assert_eq!(r.posture, PermissionPosture::Meets);
    assert!(r.results.iter().all(|(_, o)| *o == Outcome::Pass));
    assert!(autonomy_allowed(r.posture));
}

#[tokio::test]
async fn a_denied_command_that_ran_is_the_worst_failure() {
    for broken in [
        Scenario::DenyBlocks,
        Scenario::UnlistedDenied,
        Scenario::HookFailureBlocks,
    ] {
        let mut m = correct();
        m.insert(broken, obs(true, false));
        let r = probe(&Scripted(m)).await;
        assert_eq!(r.posture, PermissionPosture::DoesNotMeet, "{broken:?}");
        assert!(!autonomy_allowed(r.posture));
        let Outcome::Fail(why) = &r.results.iter().find(|(s, _)| *s == broken).unwrap().1 else {
            panic!()
        };
        assert!(why.contains("ran although it must not"));
    }
}

#[tokio::test]
async fn an_allow_rule_that_does_not_run_also_fails_because_autonomy_could_not_work() {
    let mut m = correct();
    m.insert(Scenario::AllowRuns, obs(false, true));
    let r = probe(&Scripted(m)).await;
    assert_eq!(r.posture, PermissionPosture::DoesNotMeet);
}

#[tokio::test]
async fn a_scenario_that_cannot_be_judged_makes_the_posture_unknown_not_met() {
    let mut m = correct();
    m.insert(Scenario::AllowRuns, Err("agy crashed"));
    let r = probe(&Scripted(m)).await;
    assert_eq!(r.posture, PermissionPosture::Unknown);
    assert!(!autonomy_allowed(r.posture));
    // A real failure outranks an inconclusive one.
    let mut m = correct();
    m.insert(Scenario::AllowRuns, Err("agy crashed"));
    m.insert(Scenario::DenyBlocks, obs(true, false));
    assert_eq!(
        probe(&Scripted(m)).await.posture,
        PermissionPosture::DoesNotMeet
    );
}

struct FakeSession {
    marker_for_next: Mutex<Option<PathBuf>>,
    deny: bool,
}

impl AgentSession for FakeSession {
    fn run_turn<'a>(
        &'a self,
        _r: &'a TurnRequest,
        sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, TurnResult> {
        Box::pin(async move {
            if let Some(m) = self.marker_for_next.lock().unwrap().clone() {
                std::fs::write(m, "x").unwrap();
            }
            if self.deny {
                sink.on_event(AgentEvent::ToolDenied {
                    tool: "run_command".into(),
                    reason: "policy".into(),
                });
            }
            Ok(TurnResult {
                conversation: Some(ConversationId::new("c1").unwrap()),
                finished: true,
                response: String::new(),
            })
        })
    }
    fn cancel<'a>(&'a self, _s: &'a SessionRef) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn status<'a>(&'a self, _s: &'a SessionRef) -> PortFuture<'a, SessionStatus> {
        Box::pin(async { Ok(SessionStatus::Idle) })
    }
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vela-posture-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[tokio::test]
async fn the_session_runner_judges_by_the_marker_file_and_cleans_up() {
    let d = scratch("runner");
    let cases: Vec<ProbeCase> = Scenario::ALL
        .iter()
        .map(|s| ProbeCase {
            scenario: *s,
            worker: WorkerId::new("probe").unwrap(),
            worktree: WorktreePath::new(d.to_string_lossy().replace('\\', "/")).unwrap(),
            prompt: "run it".into(),
            marker: marker_path(&d, *s),
        })
        .collect();
    // Session that creates the marker: the command ran.
    let ran = Arc::new(FakeSession {
        marker_for_next: Mutex::new(Some(marker_path(&d, Scenario::AllowRuns))),
        deny: false,
    });
    let runner = SessionScenarioRunner::new(ran, cases.clone());
    assert_eq!(
        runner.run(Scenario::AllowRuns).await.unwrap(),
        Observation {
            executed: true,
            denied: false
        }
    );
    assert!(
        !marker_path(&d, Scenario::AllowRuns).exists(),
        "the marker is removed afterwards"
    );
    // Session that only reports a denial: the command did not run.
    let denied = Arc::new(FakeSession {
        marker_for_next: Mutex::new(None),
        deny: true,
    });
    let runner = SessionScenarioRunner::new(denied, cases);
    assert_eq!(
        runner.run(Scenario::DenyBlocks).await.unwrap(),
        Observation {
            executed: false,
            denied: true
        }
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn policy_files_must_be_exactly_what_vela_wrote() {
    let d = scratch("integrity");
    let ok = d.join("settings.json");
    let changed = d.join("hooks.json");
    std::fs::write(&ok, "{\"a\":1}").unwrap();
    std::fs::write(&changed, "{\"hook\":\"evil\"}").unwrap();
    let expected = vec![
        ExpectedFile {
            path: ok,
            content: "{\"a\":1}".into(),
        },
        ExpectedFile {
            path: changed.clone(),
            content: "{\"hook\":\"vela-hook\"}".into(),
        },
        ExpectedFile {
            path: d.join("gone.json"),
            content: "{}".into(),
        },
    ];
    let problems = verify_policy_files(&expected);
    assert_eq!(
        problems,
        vec![
            Integrity::Changed(changed),
            Integrity::Missing(d.join("gone.json"))
        ]
    );
    let _ = std::fs::remove_dir_all(&d);
}
