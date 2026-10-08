//! Stdio transport: newline-delimited [`Envelope`] frames.
//!
//! * stdin  → requests from the host (and responses to core-initiated host calls)
//! * stdout → responses and events
//!
//! Each request runs on its own task so a slow model call never blocks settings reads.
//! EOF on stdin means the host went away; the core shuts down.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    sync::{Mutex, mpsc, oneshot},
};
use wisp_ipc::{Envelope, FrameError, IpcError, methods::CoreReady};

use crate::Core;

type Pending = Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value, IpcError>>>>>;

/// Handle for core-initiated calls into the host (`host.*` methods).
#[derive(Clone)]
pub struct HostClient {
    out: mpsc::Sender<Envelope>,
    pending: Pending,
    next: Arc<AtomicU64>,
}

impl HostClient {
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, IpcError> {
        let id = format!("c{}", self.next.fetch_add(1, Ordering::Relaxed));
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id.clone(), tx);
        self.out
            .send(Envelope::request(id, method, params))
            .await
            .map_err(|_| {
                IpcError::new(
                    wisp_ipc::ErrorCode::Unavailable,
                    "The Wisp app isn't connected.",
                )
            })?;
        rx.await.map_err(|_| {
            IpcError::new(wisp_ipc::ErrorCode::Cancelled, "The request was cancelled.")
        })?
    }
}

pub async fn serve<R, W>(core: Core, input: R, output: W) -> std::io::Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (out_tx, mut out_rx) = mpsc::channel::<Envelope>(256);
    let pending: Pending = Arc::default();
    let host = HostClient {
        out: out_tx.clone(),
        pending: pending.clone(),
        next: Arc::default(),
    };

    // Writer: the only task that touches stdout.
    let writer = tokio::spawn(async move {
        let mut output = output;
        while let Some(env) = out_rx.recv().await {
            match env.encode() {
                Ok(line) => {
                    output.write_all(line.as_bytes()).await?;
                    output.flush().await?;
                }
                Err(e) => tracing::error!(error = %e, "dropping unencodable frame"),
            }
        }
        Ok::<_, std::io::Error>(())
    });

    // Event forwarder.
    let mut events = core.events().subscribe();
    let ev_tx = out_tx.clone();
    let forwarder = tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(env) => {
                    if ev_tx.send(env).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(skipped = n, "event forwarder lagged");
                }
                Err(_) => break,
            }
        }
    });

    core.events().publish::<CoreReady>(core.info());

    let mut lines = BufReader::new(input).lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        match Envelope::decode(&line) {
            Ok(Envelope::Request {
                id, method, params, ..
            }) => {
                let core = core.clone();
                let tx = out_tx.clone();
                tokio::spawn(async move {
                    let started = std::time::Instant::now();
                    let reply = match core.dispatch(&method, params).await {
                        Ok(v) => Envelope::ok(id, v),
                        Err(e) => {
                            tracing::warn!(method, code = ?e.code, "request failed");
                            Envelope::err(id, e)
                        }
                    };
                    tracing::debug!(
                        method,
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "handled"
                    );
                    let _ = tx.send(reply).await;
                });
            }
            Ok(Envelope::Response {
                id, result, error, ..
            }) => {
                if let Some(waiter) = pending.lock().await.remove(&id) {
                    let outcome = match (result, error) {
                        (_, Some(e)) => Err(e),
                        (Some(v), None) => Ok(v),
                        (None, None) => Ok(Value::Null),
                    };
                    let _ = waiter.send(outcome);
                } else {
                    tracing::warn!(id, "response for unknown request id");
                }
            }
            Ok(Envelope::Event { event, payload, .. }) => {
                core.record_host_event(&event, payload).await;
            }
            Err(FrameError::Malformed(e)) => tracing::warn!(error = %e, "malformed frame ignored"),
            Err(e) => tracing::warn!(error = %e, "frame rejected"),
        }
    }

    tracing::info!("stdin closed; shutting down");
    forwarder.abort();
    drop(host);
    drop(out_tx);
    // Give in-flight responses a moment to drain, then stop.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), writer).await;
    Ok(())
}
