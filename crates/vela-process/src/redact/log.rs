//! Redaction at the log ingestion point: a writer that redacts every record before it reaches the log file, and the
//! JSON `tracing` layer built on it.
//!
//! Records are redacted after formatting, so no field, span value or message can bypass the redactor, whatever code
//! emitted it. The formatted record is JSON, which the redactor treats structurally (`json`), so a redacted log file
//! is still one JSON object per line.

use std::io::{self, Write};

use tracing::Subscriber;
use tracing_subscriber::fmt::format::{Format, Json, JsonFields};
use tracing_subscriber::fmt::{self, MakeWriter};
use tracing_subscriber::registry::LookupSpan;

use super::{SharedRedactor, StreamRedactor};

/// Redacts everything written through it, line by line, before passing it to the inner writer.
///
/// A line reaches the inner writer only once it is complete; the unterminated remainder is written when the writer is
/// dropped. `flush` flushes the inner writer and does not force out an incomplete line, because redacting half a line
/// could expose a secret that the other half would have identified.
pub struct RedactingWriter<W: Write> {
    inner: W,
    lines: StreamRedactor,
}

impl<W: Write> RedactingWriter<W> {
    /// Wraps `inner`.
    pub fn new(inner: W, redactor: SharedRedactor) -> Self {
        Self {
            inner,
            lines: StreamRedactor::new(redactor),
        }
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let redacted = self.lines.push_bytes(buffer);
        if !redacted.is_empty() {
            self.inner.write_all(redacted.as_bytes())?;
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Write> Drop for RedactingWriter<W> {
    fn drop(&mut self) {
        let rest = self.lines.finish();
        if !rest.is_empty() {
            // Nothing can report a failure from `drop`; a log record that cannot be written is lost, never written raw.
            let _ = self.inner.write_all(rest.as_bytes());
        }
        let _ = self.inner.flush();
    }
}

/// A `tracing-subscriber` writer factory whose writers redact (see [`RedactingWriter`]).
pub struct RedactingMakeWriter<M> {
    inner: M,
    redactor: SharedRedactor,
}

impl<M> RedactingMakeWriter<M> {
    /// Wraps the writer factory `inner`.
    pub fn new(inner: M, redactor: SharedRedactor) -> Self {
        Self { inner, redactor }
    }
}

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for RedactingMakeWriter<M> {
    type Writer = RedactingWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter::new(self.inner.make_writer(), self.redactor.clone())
    }
}

/// The structured log layer: one JSON object per record, with the fields of the current span and of every enclosing
/// span (which is where the run, ticket, worker and operation correlation ids travel), redacted before it is written.
///
/// `writer` is where the log goes: the size-capped rotating file of `vela-core::diagnostics` in production.
pub fn json_layer<S, M>(
    writer: M,
    redactor: SharedRedactor,
) -> fmt::Layer<S, JsonFields, Format<Json>, RedactingMakeWriter<M>>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
    M: for<'writer> MakeWriter<'writer> + 'static,
{
    fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_writer(RedactingMakeWriter::new(writer, redactor))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tracing::Instrument;
    use tracing_subscriber::layer::SubscriberExt;

    use super::*;
    use crate::redact::corpus::corpus;
    use crate::redact::{PatternRedactor, REDACTED, Redactor};

    /// An in-memory log file.
    #[derive(Clone, Default)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl Buffer {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    impl Write for Buffer {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Buffer {
        type Writer = Buffer;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    fn redactor() -> SharedRedactor {
        Arc::new(PatternRedactor::new())
    }

    fn subscriber(buffer: &Buffer) -> impl Subscriber + Send + Sync + 'static {
        tracing_subscriber::registry().with(json_layer(buffer.clone(), redactor()))
    }

    /// Every line must still be one JSON object after redaction.
    fn assert_json_lines(log: &str, expected: usize) {
        let lines: Vec<&str> = log.lines().collect();
        assert_eq!(lines.len(), expected, "{log}");
        for line in lines {
            assert!(line.starts_with('{') && line.ends_with('}'), "{line}");
            assert!(line.contains("\"level\":\"INFO\""), "{line}");
            // A document the structural redactor accepts is well formed, and redaction left nothing more to remove.
            assert_eq!(PatternRedactor::new().redact(line), line);
        }
    }

    #[test]
    fn the_corpus_is_redacted_in_log_records() {
        for case in corpus() {
            let buffer = Buffer::default();
            tracing::subscriber::with_default(subscriber(&buffer), || {
                tracing::info!("{}", case.text);
                tracing::info!(detail = %case.text, "as a field");
                let span = tracing::info_span!("work", detail = %case.text);
                span.in_scope(|| tracing::info!("inside a span"));
            });
            let log = buffer.text();
            for secret in &case.secrets {
                assert!(
                    !log.contains(secret.as_str()),
                    "{}: `{secret}` survived in {log}",
                    case.name
                );
            }
            assert!(log.contains(REDACTED), "{}: {log}", case.name);
            assert_json_lines(&log, 3);
        }
    }

    #[test]
    fn records_carry_the_fields_of_every_enclosing_span() {
        let buffer = Buffer::default();
        tracing::subscriber::with_default(subscriber(&buffer), || {
            let run = tracing::info_span!("correlation", run_id = "run-1", ticket_id = "F07");
            let _run = run.enter();
            let worker =
                tracing::info_span!("correlation", worker_id = "w-1", operation_id = "op-9");
            let _worker = worker.enter();
            tracing::info!(password = "hunter2", "started");
        });
        let log = buffer.text();
        assert_json_lines(&log, 1);
        for expected in [
            "\"run_id\":\"run-1\"",
            "\"ticket_id\":\"F07\"",
            "\"worker_id\":\"w-1\"",
            "\"operation_id\":\"op-9\"",
            "\"password\":\"[REDACTED]\"",
            "\"message\":\"started\"",
        ] {
            assert!(log.contains(expected), "{expected} missing from {log}");
        }
    }

    #[tokio::test]
    async fn span_fields_follow_a_spawned_task_into_its_records() {
        let buffer = Buffer::default();
        let _default = tracing::subscriber::set_default(subscriber(&buffer));
        let span = tracing::info_span!("correlation", run_id = "run-1", worker_id = "w-1");
        async {
            let task = tokio::spawn(
                async {
                    tokio::task::yield_now().await;
                    tracing::info!("from the task");
                }
                .in_current_span(),
            );
            task.await.unwrap();
        }
        .instrument(span)
        .await;
        let log = buffer.text();
        assert_json_lines(&log, 1);
        assert!(
            log.contains("\"run_id\":\"run-1\"") && log.contains("\"worker_id\":\"w-1\""),
            "{log}"
        );
    }

    #[test]
    fn a_writer_holds_an_incomplete_line_until_it_is_dropped() {
        let buffer = Buffer::default();
        let mut writer = RedactingWriter::new(buffer.clone(), redactor());
        writer.write_all(b"one\nDB_PASSWORD=hun").unwrap();
        writer.flush().unwrap();
        assert_eq!(buffer.text(), "one\n");
        writer.write_all(b"ter2").unwrap();
        drop(writer);
        assert_eq!(buffer.text(), "one\nDB_PASSWORD=[REDACTED]");
    }
}
