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
| `corpus.rs` | The redaction corpus (test only): 37 cases covering the five classes of `OBSERVABILITY.md`. |

`crates/vela-core/src/diagnostics/**` (logging setup):

| File | Content |
|---|---|
| `correlation.rs` | `CorrelationIds` (run, ticket, worker, operation) carried by a `correlation` span and a task-local: `scope`, `in_scope`, `current`, `spawn_correlated`. |
| `rotation.rs` | `LogFile`, `RotationLimits`, `LogStream`: size-capped rotation per log stream. |
| `payload.rs` | `RedactionPort` (the redactor as the core sees it), `redact_payload` and `redact_payload_text` for journal payloads. |

## Acceptance criteria

1. **Corpus of secret patterns is redacted in logs, journal payloads, and process output.** One corpus (`redact/corpus.rs`) is run through each ingestion point with the production redactor:
   - logs: `log::tests::the_corpus_is_redacted_in_log_records` emits every case as a message, as an event field and as a span field through the real JSON layer and asserts no secret is in the log and every line is still one JSON object;
   - journal payloads: `pattern::tests::the_corpus_is_redacted_in_journal_payloads` embeds every case in a serialized payload (member value and array element) and asserts the secrets are gone and the document shape is intact. That call, `Redactor::redact` on the serialized payload, is exactly what the journal entry point does: `vela-core` `redact_payload` serializes the payload and passes the document to the port (`payload::tests::the_whole_payload_passes_through_the_redactor_as_one_document`), so there is one JSON walk and the corpus covers it;
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
| 38080782735 | `a976289` | PASS, both jobs: review iteration 1 head |
| 38082211247 | `c61d64b` | FAIL: clippy, one unused import in a test module |
| 38082331441 | `7876efc` | PASS, both jobs: code after review 1. `vela-process` 42 tests, `vela-core` 21 tests (including the two Windows tests that hold a log file open) |

The corpus now has 37 cases (seven added after review 1: argument vectors and credential objects in JSON, Debug-formatted, colour-coded and percent-encoded text, `=>` and multi-word environment values, a URL password with an unencoded slash). Local loop after review 1: 37 dependency-free tests under WASI, not gate evidence.

## Review

Authoritative reviews run in a fresh reviewer session with no implementer context, through the `/code-review` skill at effort `high` (the repository names no level; this is the session's level), against the fixed point `efc6ef3` with the three-dot diff. The reviewer supplies severities; blocking is decided by the Engineering exit policy (`REVIEW_PROTOCOL.md`). The iteration limit is 3.

### Iteration 1 (`F07-R1`, head `a976289`, CI run 38080782735 green)

Sixteen findings: 2 high, 8 medium, 6 low. All ten high and medium findings are in blocking classes and were fixed; none was reclassified.

| # | Finding | Reviewer severity | Class | Disposition |
|---|---|---|---|---|
| R1-1 | Tokens, bearer values and lowercase names survive after an escape letter (`\n` in Debug output), a colour sequence or a percent escape, because matching needed a word start | high, security | blocking | Fixed: self-identifying token prefixes and JSON Web Tokens are matched anywhere; a word may start after an escape, a percent escape or a colour sequence (names too). Tests `tokens_are_found_after_escapes_and_inside_words`; three corpus cases |
| R1-2 | The JSON walk cloned the member name into every nested array frame and per element: one crafted line could exhaust memory | high, security | blocking | Fixed: names are shared (`Rc<str>`), depth is bounded (128, then plain-text redaction). Test `hostile_nesting_is_cheap_and_bounded` |
| R1-3 | Quadratic scans: repeated JSON Web Token attempts over one long run; the line end recomputed for every named value | medium, performance | blocking | Fixed: a failed token attempt marks the bytes it examined; the line end is memoized; bounded shape and URL scans. Test `hostile_input_stays_linear` (megabyte inputs that cannot finish unless linear) |
| R1-4 | `redact_payload` (the journal entry point) was a second JSON walk without the name and value pair rule and was tested only with a stand-in; numeric pair values were skipped by both | medium, spec | blocking | Fixed: one walk. `redact_payload` serializes and passes the document through the port; the pair rule covers numbers. DR-F07-1 marked as needing an owner decision before O01 |
| R1-5 | The value after a credential flag in an argument array (`["--password", "x"]`) was not redacted | medium, security | blocking | Fixed in the walk; corpus case "json argument vector" |
| R1-6 | An object under a credential-like name was not redacted (`{"credentials": {"pass": "x"}}`) | medium, security | blocking | Fixed: everything inside a container stored under a credential-like name is redacted; corpus case |
| R1-7 | Streamed private keys: one line that was not plain base64 (a `git diff` or `grep` prefix) ended the swallowing and released the rest of the key | medium, security | blocking | Fixed: an open key swallows every line until its `END` marker, at most 256 lines. Tests `prefixed_key_lines_are_swallowed_until_the_end_marker`, `an_unclosed_private_key_swallows_a_bounded_number_of_lines` |
| R1-8 | `=>` was treated as an operator; an unquoted value after `=` was one word, so environment values with blanks leaked their tail | medium, security | blocking | Fixed: `=>` assigns; `==` and `=~` compare only with blanks on both sides; an assignment that starts its line takes the rest of the line. Corpus case |
| R1-9 | A failed rotation emptied the active file, discarding the newest records (a file held open on Windows) | medium, data loss | blocking | Fixed: old history gives way first; an active file that cannot be renamed is copied, then emptied. Two Windows tests hold a file open without delete sharing |
| R1-10 | `with_environment` registered `PWD`, paths and placeholders as global secrets, corrupting ordinary output | medium, correctness | blocking | Fixed: `PWD` and `OLDPWD` are not credential names; paths and placeholders are not registered |
| R1-11 | The cut of an over-long line could expose the head of an unfinished secret | low | non-blocking | Fixed: the word the cut falls in is dropped |
| R1-12 | A URL password containing an unencoded `/` was not found | low | non-blocking | Fixed: bounded look-ahead for the `@` when what follows the colon is not a port |
| R1-13 | UTF-16 process output matches nothing | low | non-blocking | Documented on `StreamRedactor` and in DR-F07-5: P01 transcodes before feeding |
| R1-14 | Output is released only at a line feed (prompts, carriage-return progress) | low | non-blocking | `flush_partial` added for P01; a bare carriage return as a terminator is not implemented (DR-F07-5) |
| R1-15 | Weak tests: JSON well-formedness asserted by first and last character; "linear" test measured nothing; no test for the rotation fallback or the cross-crate join | low | non-blocking | Fixed except the cross-crate join, which cannot be written inside F07's surfaces (recorded for W01 and X04) |
| R1-16 | Evidence table stopped before the review head; `payload.rs` claimed the payload shape never changes; `redact_payload_text` always re-serialized | low | non-blocking | Fixed: this table; the module documentation states what changes; unchanged text is returned as is |
