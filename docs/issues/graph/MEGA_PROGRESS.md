# Mega-ticket progress

| Mega | Status | Merge SHA | Wall-clock | Notes |
|---|---|---|---|---|
| M01 | MERGED | 3017847 | 478s (about 7 min) | D02, D05, D06 already implemented on branches (merged unchanged, no review); D03, D04 written fresh (about 12 tests); CI green; local cargo check/clippy/test work |
| M02 | MERGED | 20c02db | 548s (about 9 min) | D06 already merged in M01; D07 (6 tests) and F09 (8 fakes tests, conformance suite) written; CI green |
| M03 | MERGED | bf7598b | 866s (about 14 min) | F06 (18 tests incl. kill-at-fault-point), P01 (6), P02 (3), P03 (3); SP03 verified by me and recorded in docs/research/SPIKE_S-PROC-JOB.md; CI green |
| M04 | MERGED | f359703 | 1411s (about 23 min) | T01 merged from its branch; T02 (7 tests, FakeGh in testkit), T03 (6 tests, migration 0170, port gained has_evidence_marker), X05 (cargo-deny pinned+checksum, own secret scanner, allowlist validator, SBOM; Security workflow green); CI green |
| M00 | not run | | | human/spike track |

Branches already implemented but not merged: F04 (f04/tauri-skeleton), T01 (t01/local-tracker). Fold into M05 and M04.
