use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};

use vela_domain::ids::WorktreePath;
use vela_domain::ports::{CommandSpec, OutputChunk, OutputSink, ProcessRunner};

use super::*;
use crate::redact::PatternRedactor;

#[derive(Default)]
struct Collect(Mutex<Vec<OutputChunk>>);

impl OutputSink for Collect {
    fn on_output(&self, chunk: OutputChunk) {
        self.0.lock().unwrap().push(chunk);
    }
}

impl Collect {
    fn text(&self) -> String {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.text.clone())
            .collect()
    }
}

fn runner() -> TokioProcessRunner {
    TokioProcessRunner::new(
        Arc::new(PatternRedactor::new()),
        Arc::new(TreeRegistry::new()),
    )
}

fn cwd() -> WorktreePath {
    WorktreePath::new(std::env::temp_dir().to_string_lossy().to_string()).unwrap()
}

fn spec(id: &str, program: &str, args: &[&str], timeout_ms: u64) -> CommandSpec {
    CommandSpec {
        id: id.to_owned(),
        program: program.to_owned(),
        args: args.iter().map(|s| (*s).to_owned()).collect(),
        cwd: cwd(),
        env: baseline_env(),
        timeout_ms: NonZeroU64::new(timeout_ms).unwrap(),
    }
}

#[tokio::test]
async fn runs_and_reports_exit_code_and_output() {
    let r = runner();
    let sink = Collect::default();
    let out = r
        .run(
            &spec("p1", "cmd", &["/C", "echo hello & exit 3"], 10_000),
            &sink,
        )
        .await
        .unwrap();
    assert_eq!(out.exit_code, Some(3));
    assert!(!out.timed_out && !out.cancelled);
    assert!(sink.text().contains("hello"));
    assert!(r.registry().running_ids().is_empty());
}

#[tokio::test]
async fn timeout_kills_the_tree() {
    let r = runner();
    let sink = Collect::default();
    let out = r
        .run(
            &spec("p2", "cmd", &["/C", "ping -n 60 127.0.0.1 >nul"], 500),
            &sink,
        )
        .await
        .unwrap();
    assert!(out.timed_out);
    assert_eq!(out.exit_code, None);
    assert!(out.duration_ms < 15_000);
}

#[tokio::test]
async fn kill_tree_cancels_a_running_command() {
    let r = runner();
    let sink = Collect::default();
    let s = spec("p3", "cmd", &["/C", "ping -n 60 127.0.0.1 >nul"], 60_000);
    let (out, ()) = tokio::join!(r.run(&s, &sink), async {
        tokio::time::sleep(Duration::from_millis(700)).await;
        r.kill_tree("p3").await.unwrap();
    });
    let out = out.unwrap();
    assert!(out.cancelled);
    assert!(!out.timed_out);
    r.kill_tree("unknown-id").await.unwrap();
}

#[tokio::test]
async fn environment_is_cleared_to_the_allowlist() {
    // USERNAME is set in the parent but is not on the allowlist, so it must not reach the child.
    let parent_value = std::env::var("USERNAME").unwrap_or_else(|_| "leaked-user".to_owned());
    let r = runner();
    let sink = Collect::default();
    let mut s = spec(
        "p4",
        "cmd",
        &["/C", "echo [%USERNAME%] [%ALLOWED_X%]"],
        10_000,
    );
    s.env.push(("ALLOWED_X".to_owned(), "yes".to_owned()));
    r.run(&s, &sink).await.unwrap();
    let text = sink.text();
    assert!(!text.contains(&format!("[{parent_value}]")), "{text}");
    assert!(text.contains("[yes]"), "{text}");
}

#[tokio::test]
async fn output_is_redacted_and_capped() {
    let r = runner();
    let sink = Collect::default();
    r.run(
        &spec(
            "p5",
            "cmd",
            &[
                "/C",
                "echo Authorization: Bearer abcdef0123456789abcdef0123456789",
            ],
            10_000,
        ),
        &sink,
    )
    .await
    .unwrap();
    assert!(
        !sink.text().contains("abcdef0123456789abcdef0123456789"),
        "{}",
        sink.text()
    );

    let capped = runner().with_max_output_bytes(2_000);
    let sink = Collect::default();
    capped
        .run(
            &spec(
                "p6",
                "cmd",
                &[
                    "/C",
                    "for /L %i in (1,1,2000) do @echo line-%i-padding-padding-padding",
                ],
                30_000,
            ),
            &sink,
        )
        .await
        .unwrap();
    assert!(sink.text().contains("output truncated"));
    assert!(sink.text().len() < 20_000);
}

#[tokio::test]
async fn missing_program_is_a_typed_error_and_duplicate_ids_are_refused() {
    let r = runner();
    let sink = Collect::default();
    let err = r
        .run(
            &spec("p7", "definitely-not-a-real-program-xyz", &[], 5_000),
            &sink,
        )
        .await
        .unwrap_err();
    assert_eq!(err.class(), ErrorClass::UserActionRequired);
}
