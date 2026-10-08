//! In-process event bus. Events are typed at the publish site via [`wisp_ipc::EventKind`] and
//! forwarded to the host (and from there to the UI) by the transport.

use tokio::sync::broadcast;
use wisp_ipc::{Envelope, EventKind};

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Envelope>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn publish<E: EventKind>(&self, payload: &E::Payload) {
        match serde_json::to_value(payload) {
            // No subscribers is not an error (e.g. during tests).
            Ok(v) => drop(self.tx.send(Envelope::event(E::NAME, v))),
            Err(e) => tracing::error!(event = E::NAME, error = %e, "failed to serialize event"),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Envelope> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
