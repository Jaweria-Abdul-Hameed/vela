use std::sync::Arc;

use vela_domain::ids::{BranchName, CommitSha, TicketId, WorkerId, WorktreePath};
use vela_domain::state::IntegrationHealth;
use vela_persistence::repositories::o01_workers;

use super::*;
use crate::orchestrator::runs::Runs;
use crate::orchestrator::test_support::{TestEnv, env};

async fn with_workers(e: &TestEnv, with_conversation: &[bool]) -> RunId {
    let run = e.create_run().await;
    let runs = Runs::new(&e.services);
    for (n, has) in with_conversation.iter().enumerate() {
        let w = WorkerId::new(format!("w-{n}")).unwrap();
        runs.create_worker(
            &run,
            w.clone(),
            &TicketId::new(format!("T-{n}")).unwrap(),
            BranchName::new(format!("vela/run-1/t-{n}")).unwrap(),
            WorktreePath::new(format!("C:/wt/{n}")).unwrap(),
            CommitSha::new("a".repeat(40)).unwrap(),
            "default",
            IntegrationHealth::Healthy,
        )
        .await
        .unwrap();
        if *has {
            let id = w.as_str().to_owned();
            e.services
                .store
                .write(move |tx| {
                    o01_workers::set_conversation(tx.conn(), &id, &format!("conv-{n}"), 1)
                })
                .unwrap();
        }
    }
    run
}

#[tokio::test]
async fn the_read_model_lists_every_recorded_conversation_with_its_owner() {
    let e = env("conv-list");
    let run = with_workers(&e, &[true, false, true]).await;
    let list = list_for_run(&e.services, &run).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].kind, SessionKind::Worker);
    assert_eq!(
        (list[0].ticket.as_deref(), list[0].conversation_id.as_str()),
        (Some("T-0"), "conv-0")
    );
    assert_eq!(list[1].worker.as_deref(), Some("w-2"));
}

#[tokio::test]
async fn open_in_antigravity_is_honestly_unavailable_until_the_mechanism_is_verified() {
    let e = env("conv-open");
    let run = with_workers(&e, &[true]).await;
    let c = list_for_run(&e.services, &run).unwrap().remove(0);
    let OpenAction::Unavailable { reason } = open_action(&e.services, &c) else {
        panic!("expected unavailable")
    };
    assert!(reason.contains("SP08") && !reason.contains("conv-0"));
    // AT-026 fails the requirement rather than passing on Vela's own list.
    assert_eq!(unopenable(&e.services, &run).unwrap().len(), 1);
}

#[tokio::test]
async fn a_verified_mechanism_launches_with_the_id_in_place_and_refuses_unsafe_ids() {
    let e = env("conv-mechanism");
    let run = with_workers(&e, &[true]).await;
    e.services.register(Arc::new(OpenMechanism {
        program: "agy-desktop".into(),
        args: vec!["--open".into(), "{id}".into()],
    }));
    let c = list_for_run(&e.services, &run).unwrap().remove(0);
    assert_eq!(
        open_action(&e.services, &c),
        OpenAction::Launch {
            program: "agy-desktop".into(),
            args: vec!["--open".into(), "conv-0".into()]
        }
    );
    assert!(unopenable(&e.services, &run).unwrap().is_empty());
    for bad in ["", "-rf", "a b", "x;y", "a\"b"] {
        let hostile = ConversationRef {
            conversation_id: bad.into(),
            ..c.clone()
        };
        assert!(
            matches!(
                open_action(&e.services, &hostile),
                OpenAction::Unavailable { .. }
            ),
            "{bad:?}"
        );
    }
}
