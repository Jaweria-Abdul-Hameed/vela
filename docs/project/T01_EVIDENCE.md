# T01 Evidence (execution log)

Ticket: `docs/issues/graph/tickets/T01.md` (Local Markdown tracker: ticket parsing, blockers, and malformed-metadata handling).
Base (fixed point): `2a7858dfcf21baea9b277ccb401585b9feb785d7` (`main`). Branch `t01/local-tracker`, worktree `%LOCALAPPDATA%\Vela\worktrees\t01`.

## What T01 delivered

No dependency, manifest, lockfile, `lib.rs` or contract change. Every file is inside `crates/vela-adapters/src/tracker_local/**`, plus this document.

| File | Content |
|---|---|
| `parse.rs` | `parse_ticket(file_name, text) -> ParsedTicket`: pure, never fails. Typed `WarningKind` / `TicketWarning`. |
| `parse/tests.rs` | Parser tests (valid, template, None, malformed blockers, ids, fences, BOM/CRLF, hostile text, garbage). |
| `mod.rs` | `TicketCorpus` (`from_texts`, `load_dir`): tickets, authored `DependencyEdge`s, warnings. `LocalTracker`: `IssueTrackerAdapter` (list and read; write operations report `CapabilityMissing`). |
| `tests.rs` | Corpus, edge, duplicate, directory-loading, port tests, and the real `docs/issues/graph/tickets` corpus. |

## Format read

Graph-ticket layout (`# ID: Title`, `> Symbolic key: \`ID\``, `**Planned labels:**`, `## 7. Dependencies / blocked by` bullets) and the `ISSUE_AUTHORING.md` template layout (`# Title`, `## Dependencies / blocked by`; id from the file name). The symbolic key wins over a title id. A blocker is the first word of a bullet; `None` means none. Headings inside code fences are ignored; only the first blockers section counts.

## Acceptance criteria

1. **Valid tickets parse.** `parse::tests::a_valid_graph_ticket_parses_with_blockers_and_labels`, `a_template_ticket_parses_with_the_filename_as_id`, `none_means_no_blockers_without_a_warning`; and `tests::the_repository_graph_tickets_parse_without_warnings`: all 135 real tickets parse with zero warnings (corpus check done independently with a node script: 135 files, 277 blocker bullets, equal to `ISSUE_GRAPH.json` `reducedEdges`).
2. **Malformed ones produce typed warnings.** `WarningKind` variants: `NotUtf8`, `TooLarge`, `Unreadable`, `MissingTitle`, `TitleTruncated`, `InvalidTicketId`, `IdMismatch`, `MissingBlockersSection`, `UnrecognisedBlockersText`, `InvalidBlocker`, `SelfBlocker`, `DuplicateBlocker`, `DuplicateTicketId`, `UnknownBlocker`. Tests: `a_missing_blockers_section_is_flagged`, `invalid_self_and_duplicate_blockers_are_flagged_and_dropped`, `unrecognised_text_...`, `a_file_without_a_valid_id_is_not_a_ticket`, `the_symbolic_key_wins_...`, `a_missing_title_...`, `a_long_title_is_truncated`, `a_duplicate_id_keeps_the_first_file...`, `loading_a_directory_skips_non_markdown_oversized_and_non_utf8_files`. No crash: `arbitrary_garbage_never_panics`, `a_malformed_file_does_not_stop_the_others`, `a_missing_directory_is_an_error_not_a_panic`.
3. **Blockers become explicit edges.** `tests::blockers_become_explicit_authored_edges` (`DependencyEdge`, `EdgeOrigin::Authored`, blocker to blocked); a blocker outside the corpus yields `UnknownBlocker` and no edge (`a_blocker_outside_the_corpus_is_flagged_and_gets_no_edge`); the real corpus yields 277 edges.

Architectural constraints (ADR-013): ticket text is data only. `hostile_text_is_data_and_warnings_quote_it_safely` (control characters and length bounded in warnings, instructions in the body carried verbatim and inert); files are found by listing the one configured directory, never from an id or a name inside a ticket; symbolic links and directories are skipped; files over 1 MiB and non-UTF-8 files are skipped with a warning; blocker text such as `../../etc/passwd` is rejected as an id. ADR-017 (gh CLI) is not in scope: this adapter does no GitHub access.

## Decisions and limits

- Write operations (`update_status`, `comment_evidence`, `close_ticket`) return `CapabilityMissing` ("read-only"); T01 scope is parsing, blockers and warnings. Follow-up for whichever ticket needs local write-back.
- Cycles are not detected here (FR-006 belongs to the graph tickets); edges are emitted as authored.
- `read_ticket` reloads the directory (stateless, always current).
- Ids that differ only by case count as duplicates (NTFS).

## Validation

Local Rust build, clippy and test are blocked on this host (Smart App Control); CI on `windows-latest` is the evidence of record. Locally: `cargo fmt --all --check` exit 0; `npm run check:structure` exit 0 (crate dependency and module skeleton checks).

| CI run | Head | Result |
|---|---|---|
| 38149533417 | 2f2edfa | failure: clippy E0716 in a test (temporary dropped while borrowed) |
| 38149651988 | f5b8d6b | success (fmt, clippy -D warnings, build, tests, structure; TS verify) |
