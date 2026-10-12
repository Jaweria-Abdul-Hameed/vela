//! Approval domain (D07, FR-034, FR-035): requests, decisions, secret-free fingerprints and the loop guard that trips
//! `APPROVAL_STALLED` when the same request repeats without progress. Pure logic, no I/O. Policy decides; delivery is a
//! separate port (ADR-007), so nothing here approves anything.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::WorkerId;
use crate::policy::OperationClass;

/// Default number of identical requests without progress before the guard trips.
pub const DEFAULT_STALL_THRESHOLD: u32 = 3;

/// What the policy engine decided for a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Allow this single request (persistent options are never selectable).
    AllowOnce,
    /// Refuse the request.
    Deny,
    /// Ask the human.
    Ask,
}

/// A secret-free identity for "the same approval request", built only from classified, non-secret fields.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Fingerprint(String);

impl Fingerprint {
    /// Builds a fingerprint from the fields `FR-034` names. The command family is reduced to the program name only: no
    /// arguments, paths or values reach the fingerprint, so a secret in a command line cannot appear in it.
    pub fn new(
        provider: &str,
        worker: &WorkerId,
        class: OperationClass,
        command_family: &str,
        working_root: &str,
        target_category: &str,
    ) -> Self {
        let family = command_family_of(command_family);
        let class = format!("{class:?}");
        let mut h = Fnv128::new();
        for part in [
            provider,
            worker.as_str(),
            class.as_str(),
            family.as_str(),
            working_root,
            target_category,
        ] {
            h.write(&(part.len() as u64).to_le_bytes());
            h.write(part.as_bytes());
        }
        Self(h.hex())
    }

    /// The hex digest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The program name of a command line: the first token without directory or extension, lowercased. Everything after it is
/// dropped.
pub fn command_family_of(command: &str) -> String {
    let first = command.split_whitespace().next().unwrap_or("");
    let name = first.rsplit(['/', '\\']).next().unwrap_or("");
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    stem.to_ascii_lowercase()
}

/// One approval request as the broker sees it. The operation text is deliberately not part of the fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Request identifier, unique per source.
    pub id: String,
    /// The worker that asked.
    pub worker: WorkerId,
    /// The classified operation.
    pub class: OperationClass,
    /// The secret-free fingerprint.
    pub fingerprint: Fingerprint,
}

/// A signal that the worker is making progress. Any signal resets every repeat counter for that worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressSignal {
    /// The branch head advanced (a new commit).
    HeadAdvanced,
    /// The set of changed files or their content changed.
    DiffChanged,
    /// A validation result changed (a failing test now passes, or a new failure appeared).
    ValidationChanged,
    /// A turn finished.
    TurnFinished,
    /// An approved operation was observed to complete.
    OperationCompleted,
}

/// The guard's verdict after recording a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardVerdict {
    /// Keep going; `count` identical requests have been seen since the last progress.
    Continue {
        /// Repeats so far.
        count: u32,
    },
    /// Threshold reached: raise `APPROVAL_STALLED`.
    Stalled {
        /// The repeated fingerprint.
        fingerprint: Fingerprint,
        /// Repeats so far.
        count: u32,
    },
}

/// Counts identical fingerprints per worker since the last progress signal.
#[derive(Debug, Clone)]
pub struct LoopGuard {
    threshold: u32,
    counts: BTreeMap<(WorkerId, Fingerprint), u32>,
}

impl Default for LoopGuard {
    fn default() -> Self {
        Self::new(DEFAULT_STALL_THRESHOLD)
    }
}

impl LoopGuard {
    /// A guard that trips at `threshold` repeats; a threshold below 2 is raised to 2 (one request is never a loop).
    pub fn new(threshold: u32) -> Self {
        Self {
            threshold: threshold.max(2),
            counts: BTreeMap::new(),
        }
    }

    /// The configured threshold.
    pub fn threshold(&self) -> u32 {
        self.threshold
    }

    /// Records a request and reports whether the worker is looping.
    pub fn record_request(&mut self, request: &ApprovalRequest) -> GuardVerdict {
        let n = self
            .counts
            .entry((request.worker.clone(), request.fingerprint.clone()))
            .or_insert(0);
        *n += 1;
        if *n >= self.threshold {
            GuardVerdict::Stalled {
                fingerprint: request.fingerprint.clone(),
                count: *n,
            }
        } else {
            GuardVerdict::Continue { count: *n }
        }
    }

    /// Records progress for `worker`: every counter of that worker resets.
    pub fn record_progress(&mut self, worker: &WorkerId, _signal: ProgressSignal) {
        self.counts.retain(|(w, _), _| w != worker);
    }

    /// Forgets a worker (it finished or was cancelled).
    pub fn forget(&mut self, worker: &WorkerId) {
        self.counts.retain(|(w, _), _| w != worker);
    }
}

struct Fnv128 {
    a: u64,
    b: u64,
}

impl Fnv128 {
    fn new() -> Self {
        Self {
            a: 0xcbf2_9ce4_8422_2325,
            b: 0x8422_2325_cbf2_9ce4,
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.a = (self.a ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
            self.b = (self.b ^ u64::from(byte).rotate_left(3)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn hex(&self) -> String {
        format!("{:016x}{:016x}", self.a, self.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> WorkerId {
        WorkerId::new(s).unwrap()
    }

    fn req(worker: &str, command: &str, n: u32) -> ApprovalRequest {
        let worker = w(worker);
        ApprovalRequest {
            id: format!("r{n}"),
            fingerprint: Fingerprint::new(
                "antigravity",
                &worker,
                OperationClass::FileMutate,
                command,
                "worktree",
                "source",
            ),
            worker,
            class: OperationClass::FileMutate,
        }
    }

    #[test]
    fn repeats_without_progress_trip_the_threshold() {
        let mut g = LoopGuard::default();
        assert_eq!(
            g.record_request(&req("w1", "cp a b", 1)),
            GuardVerdict::Continue { count: 1 }
        );
        assert_eq!(
            g.record_request(&req("w1", "cp c d", 2)),
            GuardVerdict::Continue { count: 2 }
        );
        assert!(matches!(
            g.record_request(&req("w1", "cp e f", 3)),
            GuardVerdict::Stalled { count: 3, .. }
        ));
    }

    #[test]
    fn every_progress_signal_resets() {
        for signal in [
            ProgressSignal::HeadAdvanced,
            ProgressSignal::DiffChanged,
            ProgressSignal::ValidationChanged,
            ProgressSignal::TurnFinished,
            ProgressSignal::OperationCompleted,
        ] {
            let mut g = LoopGuard::default();
            g.record_request(&req("w1", "cp a b", 1));
            g.record_request(&req("w1", "cp a b", 2));
            g.record_progress(&w("w1"), signal);
            assert_eq!(
                g.record_request(&req("w1", "cp a b", 3)),
                GuardVerdict::Continue { count: 1 }
            );
        }
    }

    #[test]
    fn workers_and_families_are_counted_separately() {
        let mut g = LoopGuard::default();
        g.record_request(&req("w1", "cp a b", 1));
        g.record_request(&req("w1", "cp a b", 2));
        assert_eq!(
            g.record_request(&req("w2", "cp a b", 3)),
            GuardVerdict::Continue { count: 1 }
        );
        assert_eq!(
            g.record_request(&req("w1", "mv a b", 4)),
            GuardVerdict::Continue { count: 1 }
        );
        g.record_progress(&w("w2"), ProgressSignal::TurnFinished);
        assert!(matches!(
            g.record_request(&req("w1", "cp a b", 5)),
            GuardVerdict::Stalled { .. }
        ));
    }

    #[test]
    fn threshold_is_at_least_two() {
        let mut g = LoopGuard::new(0);
        assert_eq!(g.threshold(), 2);
        assert_eq!(
            g.record_request(&req("w1", "cp a b", 1)),
            GuardVerdict::Continue { count: 1 }
        );
    }

    #[test]
    fn fingerprints_hold_no_secrets() {
        let secret = "sk-live-4f9a8b7c6d5e";
        let a = req(
            "w1",
            &format!("curl -H 'Authorization: {secret}' https://x.test"),
            1,
        );
        let shown = format!("{:?} {}", a.fingerprint, a.fingerprint);
        assert!(!shown.contains(secret));
        assert!(!shown.contains("Authorization"));
        let b = req("w1", "curl --other-arg https://y.test", 2);
        assert_eq!(
            a.fingerprint, b.fingerprint,
            "only the family reaches the fingerprint"
        );
        assert_eq!(a.fingerprint.as_str().len(), 32);
    }

    #[test]
    fn command_family_drops_path_extension_and_arguments() {
        assert_eq!(command_family_of(r"C:\Tools\Git.EXE status"), "git");
        assert_eq!(command_family_of("/usr/bin/npm run x"), "npm");
        assert_eq!(command_family_of(""), "");
    }
}
