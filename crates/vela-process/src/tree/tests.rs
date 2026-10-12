//! SP03 evidence tests: a real process tree (cmd -> powershell -> ping) ends completely through the Job Object.

use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use super::*;

fn tree_command() -> Command {
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-Command",
        "Start-Process ping -ArgumentList '-n 120 127.0.0.1' -NoNewWindow; Start-Process ping -ArgumentList '-n 120 127.0.0.1' -NoNewWindow; Start-Sleep 120",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .kill_on_drop(true);
    cmd
}

async fn wait_for_count(tree: &JobTree, at_least: u32) -> u32 {
    for _ in 0..100 {
        let n = tree.active_processes().unwrap();
        if n >= at_least {
            return n;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tree.active_processes().unwrap()
}

#[tokio::test]
async fn terminate_ends_the_whole_tree_including_powershell_children() {
    let tree = JobTree::new().unwrap();
    let mut child = tree_command().spawn().unwrap();
    tree.assign(&child).unwrap();
    let before = wait_for_count(&tree, 3).await;
    assert!(
        before >= 3,
        "expected powershell plus two pings in the job, saw {before}"
    );
    tree.terminate().unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(10), child.wait()).await;
    for _ in 0..50 {
        if tree.active_processes().unwrap() == 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("orphans left: {}", tree.active_processes().unwrap());
}

#[tokio::test]
async fn dropping_the_job_kills_the_tree() {
    let tree = JobTree::new().unwrap();
    let mut child = tree_command().spawn().unwrap();
    tree.assign(&child).unwrap();
    assert!(wait_for_count(&tree, 3).await >= 3);
    drop(tree);
    let status = tokio::time::timeout(Duration::from_secs(10), child.wait()).await;
    assert!(
        status.is_ok(),
        "the root must end when the job handle closes"
    );
}

#[tokio::test]
async fn graceful_then_forced_leaves_nothing_and_registry_tracks_ids() {
    let registry = TreeRegistry::new();
    let tree = JobTree::new().unwrap();
    let mut child = tree_command().spawn().unwrap();
    tree.assign(&child).unwrap();
    let tracked = registry.register("p-1", tree).unwrap();
    assert!(registry.register("p-1", JobTree::new().unwrap()).is_err());
    assert_eq!(registry.running_ids(), vec!["p-1".to_owned()]);
    assert!(wait_for_count(&tracked.tree, 3).await >= 3);
    registry
        .cancel("p-1", Duration::from_millis(300))
        .await
        .unwrap();
    assert!(tracked.was_cancelled());
    let _ = tokio::time::timeout(Duration::from_secs(10), child.wait()).await;
    assert_eq!(tracked.tree.active_processes().unwrap(), 0);
    registry.remove("p-1");
    assert!(registry.running_ids().is_empty());
    registry.cancel("unknown", Duration::ZERO).await.unwrap();
}
