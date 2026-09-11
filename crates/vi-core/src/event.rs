//! Progress and lifecycle events. One [`EventBus`] per process; the CLI
//! prints events, the server forwards them over WebSocket, bindings turn
//! them into iterators.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::ids::{JobId, VideoId};

/// Default capacity of the broadcast channel. Slow subscribers lose the
/// oldest events rather than stalling producers.
pub const DEFAULT_CAPACITY: usize = 1024;

/// Progress of one stage of a job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Progress {
    /// Job the progress belongs to.
    pub job: JobId,
    /// Video being processed, when known.
    pub video: Option<VideoId>,
    /// Operator or stage name.
    pub stage: String,
    /// 0.0 to 1.0.
    pub fraction: f64,
    /// Items processed so far (frames, chunks, calls).
    pub items_done: u64,
    /// Total items when known.
    pub items_total: Option<u64>,
    /// Cost so far in USD.
    pub cost_usd: f64,
    /// Estimated seconds remaining when computable.
    pub eta_secs: Option<f64>,
}

/// Events emitted by the pipeline and other subsystems.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Event {
    /// A job started.
    JobStarted {
        /// Job id.
        job: JobId,
        /// Video id when already known.
        video: Option<VideoId>,
        /// Source URI.
        source: String,
    },
    /// A stage (operator) began.
    StageStarted {
        /// Job id.
        job: JobId,
        /// Stage name.
        stage: String,
    },
    /// Progress within a stage.
    Progress(Progress),
    /// A stage finished.
    StageFinished {
        /// Job id.
        job: JobId,
        /// Stage name.
        stage: String,
        /// Items produced.
        items: u64,
        /// Wall-clock milliseconds.
        elapsed_ms: u64,
    },
    /// A stage failed; the job may continue.
    StageFailed {
        /// Job id.
        job: JobId,
        /// Stage name.
        stage: String,
        /// Error text.
        error: String,
    },
    /// A job finished, successfully or not.
    JobFinished {
        /// Job id.
        job: JobId,
        /// Whether every stage succeeded.
        ok: bool,
        /// Wall-clock milliseconds.
        elapsed_ms: u64,
        /// Summary text.
        summary: String,
    },
    /// A human-readable message.
    Log {
        /// `info`, `warn`, `error`.
        level: String,
        /// Message.
        message: String,
    },
}

/// Broadcast bus for [`Event`]s.
#[derive(Debug, Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl EventBus {
    /// A bus whose subscribers buffer up to `capacity` events each.
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity.max(1));
        Self { tx }
    }

    /// Publish. Events with no subscribers are dropped silently.
    pub fn emit(&self, event: Event) {
        let _ = self.tx.send(event);
    }

    /// Subscribe; the receiver sees events emitted after this call.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    /// Number of live subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subscribers_receive_events() {
        let bus = EventBus::default();
        let mut rx = bus.subscribe();
        let job = JobId::new();
        bus.emit(Event::Log {
            level: "info".into(),
            message: "hi".into(),
        });
        bus.emit(Event::StageStarted {
            job,
            stage: "sample".into(),
        });
        assert!(matches!(rx.recv().await.unwrap(), Event::Log { .. }));
        assert!(matches!(
            rx.recv().await.unwrap(),
            Event::StageStarted { .. }
        ));
    }

    #[test]
    fn events_serialise_with_type_tag() {
        let e = Event::Log {
            level: "info".into(),
            message: "x".into(),
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.starts_with(r#"{"type":"log""#));
    }
}
