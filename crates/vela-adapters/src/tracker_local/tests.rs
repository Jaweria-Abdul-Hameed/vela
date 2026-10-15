use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

fn id(text: &str) -> TicketId {
    TicketId::new(text).expect("valid id")
}

fn ticket_text(key: &str, blockers: &[&str]) -> String {
    let mut text = format!(
        "# {key}: Title of {key}\n\n> Symbolic key: `{key}`\n\n## 7. Dependencies / blocked by\n"
    );
    if blockers.is_empty() {
        text.push_str("None (can start immediately).\n");
    }
    for blocker in blockers {
        text.push_str(&format!("- {blocker} Some title\n"));
    }
    text
}

/// A unique empty directory removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "vela-t01-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn blockers_become_explicit_authored_edges() {
    let a = ticket_text("A1", &[]);
    let b = ticket_text("B2", &["A1"]);
    let c = ticket_text("C3", &["A1", "B2"]);
    let corpus = TicketCorpus::from_texts([
        ("A1.md", a.as_str()),
        ("B2.md", b.as_str()),
        ("C3.md", c.as_str()),
    ]);
    assert!(corpus.warnings.is_empty(), "{:?}", corpus.warnings);
    assert_eq!(corpus.tickets.len(), 3);
    let expected = [("A1", "B2"), ("A1", "C3"), ("B2", "C3")].map(|(blocker, blocked)| {
        DependencyEdge::new(id(blocker), id(blocked), EdgeOrigin::Authored).expect("edge")
    });
    assert_eq!(corpus.edges, expected);
}

#[test]
fn a_blocker_outside_the_corpus_is_flagged_and_gets_no_edge() {
    let b = ticket_text("B2", &["Z9"]);
    let corpus = TicketCorpus::from_texts([("B2.md", b.as_str())]);
    assert!(corpus.edges.is_empty());
    assert_eq!(
        corpus.warnings,
        vec![TicketWarning {
            source: "B2.md".to_owned(),
            kind: WarningKind::UnknownBlocker {
                blocker: "Z9".to_owned()
            }
        }]
    );
    assert_eq!(corpus.tickets[0].blockers, vec![id("Z9")]);
}

#[test]
fn a_duplicate_id_keeps_the_first_file_and_flags_the_second() {
    let first = ticket_text("A1", &[]);
    let second = "# A1: Impostor\n\n> Symbolic key: `a1`\n\n## Dependencies / blocked by\nNone\n";
    let corpus = TicketCorpus::from_texts([("A1.md", first.as_str()), ("other.md", second)]);
    assert_eq!(corpus.tickets.len(), 1);
    assert_eq!(corpus.tickets[0].title, "Title of A1");
    assert!(
        corpus
            .warnings
            .iter()
            .any(|w| w.source == "other.md"
                && matches!(w.kind, WarningKind::DuplicateTicketId { .. }))
    );
}

#[test]
fn a_malformed_file_does_not_stop_the_others() {
    let good = ticket_text("A1", &[]);
    let corpus = TicketCorpus::from_texts([
        ("bad name.md", "no heading at all"),
        ("A1.md", good.as_str()),
    ]);
    assert_eq!(corpus.tickets.len(), 1);
    assert!(matches!(
        corpus.warnings.as_slice(),
        [TicketWarning {
            kind: WarningKind::InvalidTicketId { .. },
            ..
        }]
    ));
}

#[test]
fn loading_a_directory_skips_non_markdown_oversized_and_non_utf8_files() {
    let dir = TempDir::new();
    fs::write(dir.0.join("A1.md"), ticket_text("A1", &[])).expect("write");
    fs::write(dir.0.join("B2.MD"), ticket_text("B2", &["A1"])).expect("write");
    fs::write(dir.0.join("notes.txt"), "not a ticket").expect("write");
    fs::create_dir(dir.0.join("sub.md")).expect("dir named like a ticket");
    fs::write(dir.0.join("bad.md"), [0xff, 0xfe, 0xfd]).expect("write");
    fs::write(
        dir.0.join("huge.md"),
        vec![b'x'; usize::try_from(MAX_TICKET_BYTES).expect("fits") + 1],
    )
    .expect("write");

    let corpus = TicketCorpus::load_dir(&dir.0).expect("load");
    let ids: Vec<&str> = corpus.tickets.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["A1", "B2"]);
    assert_eq!(corpus.edges.len(), 1);
    let mut kinds: Vec<(String, WarningKind)> = corpus
        .warnings
        .into_iter()
        .map(|w| (w.source, w.kind))
        .collect();
    kinds.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        kinds,
        vec![
            ("bad.md".to_owned(), WarningKind::NotUtf8),
            (
                "huge.md".to_owned(),
                WarningKind::TooLarge {
                    max_bytes: MAX_TICKET_BYTES
                }
            ),
        ]
    );
}

#[test]
fn a_missing_directory_is_an_error_not_a_panic() {
    let dir = TempDir::new();
    assert!(TicketCorpus::load_dir(&dir.0.join("absent")).is_err());
}

#[tokio::test]
async fn the_port_lists_and_reads_tickets_and_is_read_only() {
    let dir = TempDir::new();
    fs::write(dir.0.join("A1.md"), ticket_text("A1", &[])).expect("write");
    fs::write(dir.0.join("B2.md"), ticket_text("B2", &["A1"])).expect("write");
    let tracker = LocalTracker::new(&dir.0);
    let port: &dyn IssueTrackerAdapter = &tracker;

    let listed = port.list_tickets().await.expect("list");
    assert_eq!(listed.len(), 2);
    let b2 = port.read_ticket(&id("B2")).await.expect("read");
    assert_eq!(b2.blockers, vec![id("A1")]);
    assert!(port.read_ticket(&id("Z9")).await.is_err());

    let error = port
        .update_status(&id("A1"), "done")
        .await
        .expect_err("read-only");
    assert_eq!(error.code(), Some(ErrorCode::CapabilityMissing));
    assert!(port.comment_evidence(&id("A1"), "x").await.is_err());
    assert!(port.close_ticket(&id("A1")).await.is_err());
}
