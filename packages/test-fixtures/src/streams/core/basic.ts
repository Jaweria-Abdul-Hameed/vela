// A small recorded stream for the core surface: a snapshot taken at journal position 2, then five more events.
import type { SnapshotDto } from '@vela/contracts/commands/snapshot';
import type { EventEnvelope } from '@vela/contracts/generated/EventEnvelope';

const SHA = 'a'.repeat(40);

function envelope(seq: number, event: Pick<EventEnvelope, 'kind' | 'payload'>): EventEnvelope {
  return {
    seq,
    ts: 1_700_000_000_000 + seq * 1000,
    run_id: 'run-1',
    ticket_id: null,
    worker_id: null,
    schema_version: 1,
    ...event,
  } as EventEnvelope;
}

/** The whole stream, from the first event. */
export const sampleStream: EventEnvelope[] = [
  envelope(1, {
    kind: 'RunCreated',
    payload: { run_base_sha: SHA, integration_branch: 'vela/run-1', scope: 'all tickets' },
  }),
  envelope(2, {
    kind: 'PreflightCompleted',
    payload: { passed: true, blocking_findings: 0, warnings: 1 },
  }),
  envelope(3, {
    kind: 'GraphBuilt',
    payload: { graph_snapshot_id: 'g1', ticket_count: 4, edge_count: 3 },
  }),
  envelope(4, { kind: 'TicketReady', payload: { frontier_size: 2 } }),
  envelope(5, { kind: 'TicketReady', payload: { frontier_size: 1 } }),
  envelope(6, { kind: 'TicketReady', payload: { frontier_size: 0 } }),
];

/** A snapshot taken when the journal was at position 2. */
export const sampleSnapshot: SnapshotDto = {
  last_seq: 2,
  schema_version: 1,
  projects: [{ id: 'p1', name: 'demo', repo_path: 'C:/demo', last_opened_ms: 1_700_000_000_000 }],
};

/** The events a client still has to apply after [`sampleSnapshot`]. */
export const streamAfterSnapshot: EventEnvelope[] = sampleStream.filter(
  (e) => e.seq > sampleSnapshot.last_seq,
);
