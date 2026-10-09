//! Helpers to turn byte streams into line-delimited records with cancellation.

use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wisp_ai::{AiError, Chunk, ChunkStream};

/// Spawn a task that reads `resp` line by line, maps each line with `parse`, and forwards chunks
/// until done, error, or cancellation. Returns the chunk stream.
pub fn lines_to_chunks<F>(
    resp: reqwest::Response,
    cancel: CancellationToken,
    mut parse: F,
) -> ChunkStream
where
    F: FnMut(&str) -> Result<Vec<Chunk>, AiError> + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<Result<Chunk, AiError>>(64);
    tokio::spawn(async move {
        let mut bytes = resp.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();
        loop {
            let next = tokio::select! {
                _ = cancel.cancelled() => { let _ = tx.send(Err(AiError::Cancelled)).await; return; }
                n = bytes.next() => n,
            };
            match next {
                Some(Ok(b)) => {
                    buf.extend_from_slice(&b);
                    while let Some(pos) = buf.iter().position(|&c| c == b'\n') {
                        let line: Vec<u8> = buf.drain(..=pos).collect();
                        let line = String::from_utf8_lossy(&line);
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        match parse(line) {
                            Ok(chunks) => {
                                for c in chunks {
                                    let done = matches!(c, Chunk::Done(_));
                                    if tx.send(Ok(c)).await.is_err() || done {
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Err(e)).await;
                                return;
                            }
                        }
                    }
                }
                Some(Err(e)) => {
                    let _ = tx
                        .send(Err(AiError::unavailable(
                            "The connection to the model was interrupted.",
                            e.to_string(),
                        )))
                        .await;
                    return;
                }
                None => {
                    let _ = tx.send(Ok(Chunk::Done(Default::default()))).await;
                    return;
                }
            }
        }
    });
    Box::pin(futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|x| (x, rx))
    }))
}
