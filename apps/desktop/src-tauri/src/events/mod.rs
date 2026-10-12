//! The single event channel to the renderer (F08): committed journal events, batched so the renderer receives at most
//! [`MAX_EMISSIONS_PER_SECOND`] emissions a second and never loses an entry. Events reach this bridge only after the commit
//! that wrote them (the store publishes post-commit), and each batch carries events in strictly increasing `seq` order.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};
use vela_domain::events::EventEnvelope;

/// The one channel every journal event travels on. TypeScript: `EVENTS_CHANNEL` in `packages/contracts/src/commands/snapshot.ts`.
pub const EVENTS_CHANNEL: &str = "vela:events";

/// The emission rate ceiling.
pub const MAX_EMISSIONS_PER_SECOND: u64 = 30;

/// Minimum spacing between two emissions, in milliseconds.
pub const MIN_EMIT_INTERVAL_MS: u64 = 1000 / MAX_EMISSIONS_PER_SECOND + 1;

/// The payload of [`EVENTS_CHANNEL`]. TypeScript: `EventBatch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventBatch {
    /// Journal events in increasing `seq` order.
    pub events: Vec<EventEnvelope>,
}

/// Pure batching logic: collects events and releases them no more often than the minimum interval.
#[derive(Debug)]
pub struct Coalescer {
    interval_ms: u64,
    last_emit_ms: Option<u64>,
    buffer: Vec<EventEnvelope>,
}

impl Coalescer {
    /// A coalescer that releases at most one batch per `interval_ms`.
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            last_emit_ms: None,
            buffer: Vec::new(),
        }
    }

    /// Adds an event.
    pub fn push(&mut self, event: EventEnvelope) {
        self.buffer.push(event);
    }

    /// Milliseconds until a batch may be released (0 when due now); `None` when nothing is buffered.
    pub fn wait_ms(&self, now_ms: u64) -> Option<u64> {
        if self.buffer.is_empty() {
            return None;
        }
        Some(match self.last_emit_ms {
            None => 0,
            Some(last) => (last + self.interval_ms).saturating_sub(now_ms),
        })
    }

    /// Releases the buffered events as one batch when the interval has elapsed.
    pub fn take_due(&mut self, now_ms: u64) -> Option<EventBatch> {
        if self.wait_ms(now_ms) != Some(0) {
            return None;
        }
        self.last_emit_ms = Some(now_ms);
        Some(EventBatch {
            events: std::mem::take(&mut self.buffer),
        })
    }
}

/// Starts the thread that moves committed events from the store to the renderer.
pub fn spawn_bridge<R: Runtime>(
    app: AppHandle<R>,
    events: Receiver<EventEnvelope>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("vela-event-bridge".to_owned())
        .spawn(move || {
            let started = Instant::now();
            let now = || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            let mut coalescer = Coalescer::new(MIN_EMIT_INTERVAL_MS);
            loop {
                let timeout = coalescer
                    .wait_ms(now())
                    .map_or(Duration::from_secs(3600), Duration::from_millis);
                match events.recv_timeout(timeout) {
                    Ok(event) => coalescer.push(event),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        if let Some(batch) = coalescer.take_due(u64::MAX) {
                            let _ = app.emit(EVENTS_CHANNEL, batch);
                        }
                        return;
                    }
                }
                while let Ok(event) = events.try_recv() {
                    coalescer.push(event);
                }
                if let Some(batch) = coalescer.take_due(now())
                    && app.emit(EVENTS_CHANNEL, batch).is_err()
                {
                    return;
                }
            }
        })
        .expect("spawn the event bridge thread")
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_domain::events::{Event, SchemaVersion, TicketReadyPayload};

    fn envelope(seq: u64) -> EventEnvelope {
        EventEnvelope {
            seq,
            ts: 0,
            run_id: None,
            ticket_id: None,
            worker_id: None,
            event: Event::TicketReady(TicketReadyPayload { frontier_size: 1 }),
            schema_version: SchemaVersion::current(),
        }
    }

    #[test]
    fn a_burst_of_1000_events_a_second_is_coalesced_without_losing_any() {
        let mut c = Coalescer::new(MIN_EMIT_INTERVAL_MS);
        let mut emissions = 0;
        let mut delivered: Vec<u64> = Vec::new();
        for ms in 0..1000u64 {
            c.push(envelope(ms + 1));
            if let Some(batch) = c.take_due(ms) {
                emissions += 1;
                delivered.extend(batch.events.iter().map(|e| e.seq));
            }
        }
        if let Some(batch) = c.take_due(10_000) {
            emissions += 1;
            delivered.extend(batch.events.iter().map(|e| e.seq));
        }
        assert!(emissions <= 31, "{emissions} emissions in one second");
        assert_eq!(
            delivered,
            (1..=1000).collect::<Vec<u64>>(),
            "no event lost, none reordered"
        );
    }

    #[test]
    fn nothing_is_released_before_the_interval_and_an_empty_buffer_never_emits() {
        let mut c = Coalescer::new(34);
        assert_eq!(c.wait_ms(0), None);
        assert!(c.take_due(0).is_none());
        c.push(envelope(1));
        assert_eq!(c.wait_ms(0), Some(0));
        assert_eq!(c.take_due(0).unwrap().events.len(), 1);
        c.push(envelope(2));
        assert_eq!(c.wait_ms(10), Some(24));
        assert!(c.take_due(10).is_none());
        assert_eq!(c.take_due(34).unwrap().events[0].seq, 2);
    }

    #[test]
    fn the_wire_shape_is_a_batch_of_flat_envelopes() {
        let json = serde_json::to_value(EventBatch {
            events: vec![envelope(7)],
        })
        .unwrap();
        assert_eq!(json["events"][0]["seq"], 7);
        assert_eq!(json["events"][0]["kind"], "TicketReady");
        assert_eq!(EVENTS_CHANNEL, "vela:events");
    }
}
