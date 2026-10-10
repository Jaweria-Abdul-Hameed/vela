# F07 Evidence (Prompt 10, execution log)

Ticket: `docs/issues/graph/tickets/F07.md` (Structured logging, correlation ids, and the central redaction layer).
Base (fixed point): `efc6ef3cd0ddc5fb0c05ccf054382e613c4645f8` (`main` equal to `origin/main`, clean; F01, F02, F03, D01 integrated; F07's only blocker F02 done).
Worker: branch `f07/logging-redaction`, worktree `%LOCALAPPDATA%\Vela\worktrees\f07`.

## Pre-start checks

| Check | Result |
|---|---|
| F07 is READY | Its only published blocker is F02 (`ISSUE_GRAPH.json`), merged at `fd15065`. F07 is in the recomputed frontier recorded in `CURRENT_STATE.md` after D01. |
| `main` | `git fetch origin`: `main` equals `origin/main` at `efc6ef3`; post-merge CI of D01 (run 38076363143) and of the record commit (run 38076561117) both success. |
| Conflicts with D02 and D05 (running elsewhere) | Write surfaces are disjoint: D02 `crates/vela-domain/src/graph/**`, D05 `crates/vela-domain/src/policy/**`, F07 `crates/vela-process/src/redact/**` and `crates/vela-core/src/diagnostics/**` (different crates). None of the three holds a resource key (`SHARED_SURFACE_PROTOCOL.md` section 8). D05 lists "Redaction layer" only as a contract it may consume; F07 does not call the policy engine. F07 changes no manifest, lockfile, `lib.rs`, generated contract or migration, so the merge lane has no shared file to reconcile. No `d02/*` or `d05/*` branch existed on `origin` when F07 started. |
| Open decision requests | DR-F02-1 to 3, DR-F03-1 and DR-D01-1 to 14 were read; none concerns logging, redaction, `vela-process` or `vela-core::diagnostics`. |

## What F07 delivered

No dependency, manifest, lockfile or `lib.rs` change. Every file is inside the two write surfaces, plus this document and `F07_DECISION_REQUESTS.md`.

`crates/vela-process/src/redact/**` (the redaction layer):

| File | Content |
|---|---|
| `mod.rs` | `Redactor` trait (`redact`, `redact_value`), `SharedRedactor` (`Arc<dyn Redactor>`, the injectable trait object for P01, X03 and O01), the `[REDACTED]` marker. |
| `names.rs` | `is_credential_name`: which environment variables, headers, JSON members and flags hold credentials (all naming styles; counters and descriptive names exempt). |
| `pattern.rs` | `PatternRedactor`: named values, bearer tokens, known token shapes (GitHub, GitLab, Google, AWS, Slack, Stripe, npm, `sk-`, JSON Web Tokens), URL user information, PEM private keys, registered secrets (`with_secret`, `with_environment`). Hand-written single-pass scanner, no regular expressions. |
| `json.rs` | Structure-preserving redaction of serialized JSON (journal payloads, JSON log records, stream-json): strings are unescaped, redacted and re-escaped; a redacted document is still JSON of the same shape. |
| `stream.rs` | `StreamRedactor`: chunked process output redacted line by line (a secret split across reads is still found), multi-line private keys, a line limit. |
| `log.rs` | `RedactingWriter`, `RedactingMakeWriter` and `json_layer`: the JSON `tracing` layer whose records, with the fields of every enclosing span, are redacted before they are written. |
| `corpus.rs` | The redaction corpus (test only): 30 cases covering the five classes of `OBSERVABILITY.md`. |

`crates/vela-core/src/diagnostics/**` (logging setup):

| File | Content |
|---|---|
| `correlation.rs` | `CorrelationIds` (run, ticket, worker, operation) carried by a `correlation` span and a task-local: `scope`, `in_scope`, `current`, `spawn_correlated`. |
| `rotation.rs` | `LogFile`, `RotationLimits`, `LogStream`: size-capped rotation per log stream. |
| `payload.rs` | `RedactionPort` (the redactor as the core sees it), `redact_payload` and `redact_payload_text` for journal payloads. |

## Acceptance criteria

1. **Corpus of secret patterns is redacted in logs, journal payloads, and process output.** One corpus (`redact/corpus.rs`) is run through each ingestion point with the production redactor:
   - logs: `log::tests::the_corpus_is_redacted_in_log_records` emits every case as a message, as an event field and as a span field through the real JSON layer and asserts no secret is in the log and every line is still one JSON object;
   - journal payloads: `pattern::tests::the_corpus_is_redacted_in_journal_payloads` embeds every case in a serialized payload (member value and array element) and asserts the secrets are gone and the document shape is intact; `vela-core` `payload::tests` covers the typed (`serde_json::Value`) path the Orchestrator will use;
   - process output: `stream::tests::the_corpus_is_redacted_however_the_output_is_chunked` feeds every case in chunks of 1, 2, 3, 5, 7, 16 and 64 bytes and asserts the result equals the unchunked result;
   - plain text and idempotence: `pattern::tests::the_corpus_is_redacted_as_plain_text`, `redaction_is_idempotent`; false positives: `ordinary_text_is_returned_unchanged_and_borrowed`.
2. **Log rotation caps total size.** `rotation::tests::rotation_caps_the_total_size` writes more than twenty times the cap and checks the bytes on disk after every record; `a_record_larger_than_a_file_is_split_and_still_capped`, `keeping_no_rotated_files_keeps_only_the_active_file`, `reopening_appends_and_enforces_smaller_limits`, `files_rotate_in_order`, `clones_share_one_file_and_records_are_not_interleaved`.
3. **Correlation ids propagate across async tasks (test).** `correlation::tests::correlation_ids_propagate_across_async_tasks` (parent, spawned child, grandchild, after `.await`), `concurrent_tasks_keep_their_own_ids`, `nested_scopes_add_ids_and_a_plain_spawn_loses_them`, `the_ids_in_force_can_be_read_back`; and through the real JSON layer in `vela-process`: `log::tests::records_carry_the_fields_of_every_enclosing_span`, `span_fields_follow_a_spawned_task_into_its_records`.

Architectural constraints: no repository, issue or `AGENTS.md` text is read or interpreted by this code (ADR-013); the redactor has no configuration that such text could change.

## Decisions

Seven interpretations and the known limits are recorded in `F07_DECISION_REQUESTS.md` (DR-F07-1 to DR-F07-7). The two structural ones: the `Redactor` trait cannot be named by `vela-core`, so the core takes it behind `RedactionPort` and the composition root joins them (DR-F07-1); the subscriber is installed by the composition root from `json_layer` and `LogFile`, because no single crate inside F07's surfaces may depend on both (DR-F07-2).

## Validation

Local Rust build, clippy and test remain blocked on this host (Smart App Control), as for every earlier ticket; CI on `windows-latest` is the evidence of record. Locally, `cargo fmt --all --check`, `check-crate-deps` and `check-module-skeleton` pass. As a faster loop only, the dependency-free modules (`names`, `pattern`, `json`, `stream`, corpus) were compiled with `rustc --target wasm32-wasip1`, linted with `clippy-driver` and run under Node's WASI (29 tests passed); this needed `rustup target add wasm32-wasip1` on the host and is not evidence for the gate.

| CI run | Head | Result |
|---|---|---|
| 38080064902 | `3030e90` | FAIL: compile error in a `vela-core` test helper (`tracing::span::Current` is not re-exported) |
| 38080304978 | `3b47e76` | PASS, both jobs (Rust: format, structure, clippy `-D warnings`, build `--all-targets`, `cargo test --workspace`; TypeScript). `vela-process` 34 tests, `vela-core` 18 tests |

## Review

(recorded per iteration below)
