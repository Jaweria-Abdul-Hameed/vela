# D06 Evidence (execution log)

Ticket: `docs/issues/graph/tickets/D06.md` (Review domain). Base `2a7858dfcf21baea9b277ccb401585b9feb785d7`. Branch `d06/review-domain`, worktree `%LOCALAPPDATA%\Vela\worktrees\d06`.

## Delivered (`crates/vela-domain/src/review/**`, pure, no dependency, no `lib.rs` change)

| File | Content |
|---|---|
| `fingerprint.rs` | `FindingId` (`f-` + 16 hex, validated on deserialize), `fingerprint(category, location, requirement)` (normalized, length-prefixed, hand-written FNV-1a 64, stable across platforms). |
| `finding.rs` | `Finding`, `ReviewAxis`, `ReviewCategory`, Engineering policy (`is_blocking`, `blocks_as_observation`), `Disposition` and guarded transitions (`Resolved` only through a verifier verdict; blocking findings are never deferred or accepted). |
| `verification.rs` | `Verdict`, `evaluate_verification`, `ReviewOutcome` (`Pass`, `FixRequired`, `Escalate`), `Escalation` (kind is always `REVIEW_STALLED`), `StallReason`. |
| `cycle.rs` | `ReviewHistory`: `begin`, `record_fix_set`, `complete_comprehensive`, `complete_verification`; `CycleStage`, `StartOrigin`, `FixSetKind`, `ReviewError`. |

Interpretations: `D06_DECISION_REQUESTS.md` (DR-D06-1 to 6).

## Self-audit against the acceptance criteria

1. Same id from review into verification: id is the fingerprint of category, location, requirement only (`finding::tests::id_ignores_severity_evidence_and_fix_wording`, `fingerprint::tests::*`); `cycle::tests::blocking_findings_require_a_fix_then_a_verification_that_passes_when_all_resolved` verifies by the ids the comprehensive review produced.
2. Exit policy matches REVIEW_PROTOCOL.md: `finding::tests::blocking_matches_the_engineering_policy_for_every_category_and_severity` (all 13 x 4 against an independent oracle), observation test, high-severity test.
3. Second comprehensive rejected without a user decision whatever the inputs: `cycle::tests::a_second_comprehensive_review_is_rejected_without_a_user_decision_whatever_the_origin` and `..._after_a_fix_set_a_stall_and_a_pass` (every origin: new, fixed-point advance, retry, recovery); in-flight rerun test; property test checks it over random sequences.
4. Every failed verification escalates `REVIEW_STALLED`; success unreachable with an unresolved blocking finding: `verification::tests::outcome_table_over_every_combination` (3 verdict states x 3 fix-diff x 3 observation), individual tests for NOT_RESOLVED, missing verdict, fix-diff defect, high observation, empty-evidence and contradictory verdicts; `cycle::tests::every_failed_verification_stalls_with_review_stalled`; proptests `pass_implies_every_blocking_finding_resolved` and `success_is_unreachable_with_an_unresolved_blocking_finding` (random op sequences with any cause, including user decisions, against an independent model). The evaluation function takes no cause, so no timeout or human decision can approve a finding; a user decision only enables another fix and verification.

## Validation

Local Rust build, clippy and test are blocked on this host (Smart App Control, os error 4551). Local: `cargo fmt --all --check` exit 0; `node scripts/check-crate-deps.mjs` passed. The authoritative gates are GitHub CI (run ids below).

CI: see the completion report / appended below.
