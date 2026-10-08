//! Logging. Every line passes through `wisp_security::redact` before it reaches disk.
//! stdout is never used for logs: it carries IPC frames.

use std::io::Write;
use std::path::Path;

use tracing_subscriber::{EnvFilter, fmt::MakeWriter, prelude::*};

/// Writer adapter that redacts secrets from each formatted log record.
#[derive(Clone)]
struct Redacting<W>(W);

impl<'a, W: MakeWriter<'a>> MakeWriter<'a> for Redacting<W> {
    type Writer = RedactingWriter<W::Writer>;
    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter(self.0.make_writer())
    }
}

pub struct RedactingWriter<W>(W);

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        self.0
            .write_all(wisp_security::redact::redact(&text).as_bytes())?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

/// Initialize logging into `logs_dir/wisp-core.log.<date>`. Returns a guard that must be kept
/// alive for the process lifetime so buffered lines are flushed.
pub fn init(logs_dir: &Path, also_stderr: bool) -> tracing_appender::non_blocking::WorkerGuard {
    let filter = EnvFilter::try_from_env("WISP_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let file = tracing_appender::rolling::daily(logs_dir, "wisp-core.log");
    let (file, guard) = tracing_appender::non_blocking(file);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(Redacting(file));
    let stderr_layer = also_stderr
        .then(|| tracing_subscriber::fmt::layer().with_writer(Redacting(std::io::stderr)));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .try_init();
    guard
}
