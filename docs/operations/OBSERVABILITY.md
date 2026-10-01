# Observability and Diagnostics

## Event timeline

Every run and ticket exposes human-readable events backed by structured
journal records.

## Logs

Separate: - Vela core log, - worker/agent stream, - command
stdout/stderr, - Git operations, - tracker operations.

Logs use correlation IDs: run, ticket, worker, operation.

## Redaction

Redact: - tokens, - authorization headers, - credential-like environment
values, - cookies, - known secret patterns.

## Diagnostic bundle

User may export: - version/platform, - non-secret settings, - run
state, - event journal, - adapter capabilities, - redacted logs, - Git
ref summary.

Never include repository source by default in a diagnostic bundle.

# Approval Telemetry

Journal: - detection source, - normalized operation class, - policy
decision/rule ID, - delivery adapter, - verification result, -
repetition count, - elapsed blocked time.

Do not log secrets, full credentials, or sensitive screen contents.
