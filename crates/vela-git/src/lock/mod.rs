//! The per-worktree operation lock (G01): every Git operation that mutates a worktree (checkpoint, merge, discard, cleanup)
//! holds the lock of that worktree for its whole duration, so two holders of the same worktree are serialized while a
//! different worktree is never blocked. Reads do not lock. The key is the case-folded worktree path, because two spellings
//! of one path name the same directory on NTFS.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};
use vela_domain::ids::{OperationId, WorktreePath, case_fold_key};

/// Held while an operation runs; dropping it releases the worktree.
#[derive(Debug)]
pub struct WorktreeGuard {
    _guard: OwnedMutexGuard<()>,
    key: String,
    holder: OperationId,
    holders: Arc<Mutex<HashMap<String, OperationId>>>,
}

impl Drop for WorktreeGuard {
    fn drop(&mut self) {
        if let Ok(mut holders) = self.holders.lock()
            && holders.get(&self.key) == Some(&self.holder)
        {
            holders.remove(&self.key);
        }
    }
}

/// The set of per-worktree locks.
#[derive(Debug, Default)]
pub struct OperationLocks {
    locks: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
    holders: Arc<Mutex<HashMap<String, OperationId>>>,
}

fn key_of(path: &WorktreePath) -> String {
    let normalized = path.as_str().replace('\\', "/");
    case_fold_key(normalized.trim_end_matches('/'))
}

impl OperationLocks {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Waits for the worktree's lock and takes it on behalf of `operation`.
    pub async fn acquire(&self, path: &WorktreePath, operation: &OperationId) -> WorktreeGuard {
        let key = key_of(path);
        let mutex = {
            let mut locks = self.locks.lock().expect("locks map");
            Arc::clone(locks.entry(key.clone()).or_default())
        };
        let guard = mutex.lock_owned().await;
        self.holders
            .lock()
            .expect("holders map")
            .insert(key.clone(), operation.clone());
        WorktreeGuard {
            _guard: guard,
            key,
            holder: operation.clone(),
            holders: Arc::clone(&self.holders),
        }
    }

    /// The operation currently holding the worktree, if any (for diagnostics and cleanup reports).
    pub fn holder_of(&self, path: &WorktreePath) -> Option<OperationId> {
        self.holders
            .lock()
            .expect("holders map")
            .get(&key_of(path))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    use super::*;

    fn path(s: &str) -> WorktreePath {
        WorktreePath::new(s).unwrap()
    }
    fn op(s: &str) -> OperationId {
        OperationId::new(s).unwrap()
    }

    #[tokio::test]
    async fn two_holders_of_the_same_worktree_are_serialized() {
        let locks = Arc::new(OperationLocks::new());
        let inside = Arc::new(AtomicU32::new(0));
        let max_inside = Arc::new(AtomicU32::new(0));
        let mut tasks = Vec::new();
        for n in 0..4 {
            let (locks, inside, max_inside) = (locks.clone(), inside.clone(), max_inside.clone());
            tasks.push(tokio::spawn(async move {
                let _g = locks
                    .acquire(&path("C:/wt/a"), &op(&format!("op-{n}")))
                    .await;
                let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                max_inside.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(20)).await;
                inside.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for t in tasks {
            t.await.unwrap();
        }
        assert_eq!(max_inside.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_different_worktree_is_not_blocked() {
        let locks = OperationLocks::new();
        let _held = locks.acquire(&path("C:/wt/a"), &op("op-a")).await;
        let other = tokio::time::timeout(
            Duration::from_millis(200),
            locks.acquire(&path("C:/wt/b"), &op("op-b")),
        )
        .await;
        assert!(other.is_ok(), "an unrelated worktree must not wait");
    }

    #[tokio::test]
    async fn spellings_of_one_path_share_a_lock_and_the_holder_is_reported() {
        let locks = OperationLocks::new();
        let held = locks.acquire(&path("C:/WT/Repo"), &op("op-1")).await;
        assert_eq!(locks.holder_of(&path("c:\\wt\\repo\\")), Some(op("op-1")));
        let blocked = tokio::time::timeout(
            Duration::from_millis(100),
            locks.acquire(&path("c:/wt/repo"), &op("op-2")),
        )
        .await;
        assert!(blocked.is_err(), "the same directory must be serialized");
        drop(held);
        assert_eq!(locks.holder_of(&path("C:/WT/Repo")), None);
        assert!(
            tokio::time::timeout(
                Duration::from_millis(200),
                locks.acquire(&path("c:/wt/repo"), &op("op-2"))
            )
            .await
            .is_ok()
        );
    }
}
