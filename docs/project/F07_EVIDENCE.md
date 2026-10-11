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
| `corpus.rs` | The redaction corpus (test only): 38 cases covering the five classes of `OBSERVABILITY.md`. |

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
| 38082727692 | `047ce1a` | PASS, both jobs: review iteration 2 head |
| 38144894875 | `cdf107c` | PASS, both jobs: code after review 2. `vela-process` 46 tests, `vela-core` 24 tests (three Windows tests hold a log file open) |

Commits after `cdf107c` change only this document and `F07_DECISION_REQUESTS.md`. A commit cannot record its own CI run, so the run of the final documentation head is recorded in `CURRENT_STATE.md` at integration.

The corpus has 38 cases: 30 at the first review, seven added after review 1 (argument vectors and credential objects in JSON, Debug-formatted, colour-coded and percent-encoded text, `=>` and multi-word environment values, a URL password with an unencoded slash) and one after review 2 (environment listings behind markers and in columns, a `.netrc` entry). Local loop after review 2: 41 dependency-free tests under WASI, not gate evidence.

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

### Iteration 2 (`F07-R2`, head `047ce1a`, CI run 38082727692 green)

The reviewer found no high-severity finding, no crash, hang, quadratic path or boundary error, and confirmed twelve of the sixteen R1 findings resolved or acceptably documented (R1-7, R1-8, R1-15 and R1-16 partly). Sixteen new findings: 4 medium, 12 low. The four mediums are in blocking classes and were fixed; none was reclassified.

| # | Finding | Reviewer severity | Class | Disposition |
|---|---|---|---|---|
| R2-1 | A `BEGIN` line followed by anything but blanks (a colour reset, a quote, markup) did not open the key, so the following key lines were written out | medium, security | blocking | Fixed: the key opens unless the line also holds its `END` marker or the key itself. Test `a_begin_marker_followed_by_colour_codes_or_markup_still_opens_the_key` |
| R2-2 | "Starts its line" used a 16-byte window and a fixed keyword list, so list items, indented diff lines, deep indents and timestamp prefixes leaked the tail of a multi-word value | medium, security | blocking | Fixed: the scan tracks the real line start; anything that is not a word may precede the name (64-byte bound). Table rows and a corpus case |
| R2-3 | `APP_KEY`, `HMAC_KEY`, `SESSION_KEY` and similar were not credential names, in text or for environment registration | medium, security | blocking | Fixed: an upper-case variable ending in `KEY` or `KEYS` is a credential name; more qualifiers for other cases |
| R2-4 | A name followed only by blanks was never a named value: PowerShell's two-column environment listing and `.netrc` | medium, security | blocking | Fixed: a name at the start of a line followed by a wide gap takes the rest of the line; `password value` after `login` or `machine`. A `.netrc` password on a line of its own is a recorded limit |
| R2-5 | `===` was treated as an assignment | low | non-blocking | Fixed |
| R2-6 | Lines swallowed as a private key vanished without a trace | low | non-blocking | Fixed: each is replaced by the marker |
| R2-7 | The pair rule needed the name member before the value member; serializers that sort names put `value` first | low | non-blocking | Fixed: a held value is redacted when its name arrives. Test `a_pair_is_recognized_whichever_member_comes_first` |
| R2-8 | Two member names redacted to the same text collapsed into one member when the payload was re-read | low | non-blocking | Fixed: such a payload becomes the redacted document as one string; nothing is lost |
| R2-9 | `--password=x` in an argument array also claimed the next element | low | non-blocking | Fixed |
| R2-10 | Structural rules do not apply to pretty-printed multi-line JSON or to documents the walk rejects | low | non-blocking | A lone surrogate no longer stops the walk; the rest is recorded as a limit ("Structure across lines") |
| R2-11 | A string payload was redacted as a quoted literal | low | non-blocking | Fixed: redacted as text |
| R2-12 | A base64 secret that looks like a rooted path was not registered | low | non-blocking | Fixed: a rooted path is skipped only under a name that says it holds a location |
| R2-13 | An escaped quote inside an escaped-quote value ended it; the URL look-ahead over-redacted IPv6 and template hosts | low | non-blocking | Fixed |
| R2-14 | Start-up clean-up of log files failed the whole stream on one undeletable file; two opens of one stream each count separately | low | non-blocking | Fixed (best effort, Windows test); single ownership documented |
| R2-15 | The evidence table stopped before the review head | low | non-blocking | Fixed: code and record are separate commits, see the note under the CI table |
| R2-16 | Every payload is serialized and walked; floats of a redacted payload are re-read | low | non-blocking | Recorded as a limit |

### Iteration 3 (`F07-R3`, head `863045b`, CI run 38145194924 green; code identical to `cdf107c`, run 38144894875) -- the last permitted

The reviewer found **no high and no medium finding**: no surviving secret in the corpus classes, no panic, out-of-bounds index, off-boundary slice, hang or quadratic path, and no defect introduced by the review 2 fixes. All sixteen R2 findings are resolved (fourteen) or accepted as documented (R2-10, R2-16). All three acceptance criteria are judged met and demonstrated by tests. Nine findings, all low by the reviewer's own severity; none is in a blocking class. **No code was changed after this review**, so the merged code is the reviewed code; the nine are follow-ups.

| # | Finding (all low) | Disposition |
|---|---|---|
| R3-1 | Whether a `BEGIN` line opens a key is decided on the raw line, so in a JSON-lines stream it depends on unrelated members: it can swallow up to 256 following events, or fail to open | Follow-up, **before P01 and A02 consume redacted streams**: decide on the string that holds the marker when the line is JSON |
| R3-2 | Pair rule gaps: a `value` that is an array; a second pair-name member resets the pair | Follow-up (`credential_pair \|=`, inherit into containers) |
| R3-3 | A file name such as `token.rs:42:9: error...` is taken as a name and the diagnostic after it is redacted | Follow-up, **before P01 and A02**: do not treat `:` as an assignment after a file extension followed by a line number |
| R3-4 | The column rule redacts CLI help rows (`token       Print the current token`) | Follow-up: limit to names shaped like environment variables. Recorded trade-off (DR-F07-3) |
| R3-5 | Names with a trailing digit (`API_TOKEN2`) are not credential names | Follow-up: strip trailing digits from a segment |
| R3-6 | A multi-word value behind a worded prefix (`web_1  \| DB_PASSWORD=a b`, `INFO ...`) keeps its tail | Follow-up: accept a prefix ending in a separator. Residual of R2-2 |
| R3-7 | If `<stem>.1.log` is held without write and delete sharing, the copy fails and the active file is emptied | Follow-up: try another slot before emptying; recorded trade-off (DR-F07-6) |
| R3-8 | A task that spawns its successor with `spawn_correlated` adds one span per generation to every record | Follow-up: make the spawned span a root |
| R3-9 | `RotationLimits::max_files` has no upper bound | Follow-up: validate when S02 feeds it from settings |

Owner of the follow-ups: P01 (the first consumer of `StreamRedactor`) for R3-1 and R3-3 at the latest; the others with X04 (security regression suite) or the next change to these modules. They need a ticket or an explicit assignment; F07 does not reopen for them.

## Exit policy verdict

**Engineering exit policy: PASS** at review iteration 3 (the limit), on the reviewer's own severities: zero blocking findings, zero high, zero medium; tests, clippy `-D warnings`, build and structure checks green in CI on the reviewed code. Three authoritative reviews were run and no further one was requested or authorized. Open decisions are in `F07_DECISION_REQUESTS.md`; DR-F07-1 (one `Redactor` trait in `vela-domain::ports` instead of two joined at the composition root) is wanted before O01.
