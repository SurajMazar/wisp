//! Preparing the screen while the user talks.
//!
//! A local model spends most of its time reading the prompt before its first word (~10 s for a
//! screenshot on an M1 Pro). When a capture arrives at the start of a hold, the engine sends the
//! question-independent start of the prompt right away; local runtimes keep that work, so when the
//! question arrives a few seconds later only the question itself is left to read.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use wisp_ipc::types::focus::FocusRegion;

use crate::Core;

/// Longest a question waits for its screen to finish preparing.
const MAX_WAIT: Duration = Duration::from_secs(12);

#[derive(Default)]
pub struct Prefiller {
    current: Mutex<Option<Job>>,
}

struct Job {
    focus_id: String,
    cancel: CancellationToken,
    done: watch::Receiver<bool>,
}

impl Core {
    /// Start preparing `region` (replacing any earlier preparation).
    pub(crate) fn start_prefill(&self, region: FocusRegion) {
        let (tx, rx) = watch::channel(false);
        let cancel = CancellationToken::new();
        if let Ok(mut c) = self.prefiller().current.lock() {
            if let Some(old) = c.take() {
                old.cancel.cancel();
            }
            *c = Some(Job {
                focus_id: region.id.clone(),
                cancel: cancel.clone(),
                done: rx,
            });
        }
        let core = self.clone();
        tokio::spawn(async move {
            let started = Instant::now();
            match core.prepare_screen(&region, cancel).await {
                Ok(true) => {
                    tracing::info!(ms = started.elapsed().as_millis() as u64, "screen prepared")
                }
                Ok(false) => {}
                Err(e) => tracing::debug!(error = %e.message, "screen not prepared"),
            }
            let _ = tx.send(true);
        });
    }

    /// Wait (bounded) for the preparation of `focus_id`, if one is running.
    pub(crate) async fn await_prefill(&self, focus_id: &str) {
        let rx = self.prefiller().current.lock().ok().and_then(|c| {
            c.as_ref()
                .filter(|j| j.focus_id == focus_id)
                .map(|j| j.done.clone())
        });
        if let Some(mut rx) = rx {
            let _ = tokio::time::timeout(MAX_WAIT, rx.wait_for(|done| *done)).await;
        }
    }
}
