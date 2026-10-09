//! `wisp-core`: the Rust runtime process supervised by the Wisp macOS host.

use std::process::ExitCode;

use wisp_common::paths::DataPaths;
use wisp_core::{Config, Core, events::EventBus, logging, transport};
use wisp_database::Database;

fn main() -> ExitCode {
    let config = match Config::from_args(std::env::args()) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };

    let Some(paths) = DataPaths::resolve(config.data_dir.clone()) else {
        eprintln!("cannot determine data directory (HOME unset); pass --data-dir");
        return ExitCode::from(2);
    };
    if let Err(e) = paths.ensure() {
        eprintln!(
            "cannot create data directory {}: {e}",
            paths.root().display()
        );
        return ExitCode::from(1);
    }
    let _log_guard = logging::init(&paths.logs_dir(), config.log_stderr);
    tracing::info!(version = env!("CARGO_PKG_VERSION"), data_dir = %paths.root().display(), "wisp-core starting");

    let db = match Database::open(&paths.database_file()) {
        Ok(db) => db,
        Err(e) => {
            tracing::error!(error = %e, "failed to open database");
            eprintln!("failed to open database: {e}");
            return ExitCode::from(1);
        }
    };

    let core = match Core::new(db, &paths, EventBus::new()) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "failed to initialize core");
            return ExitCode::from(1);
        }
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to start async runtime: {e}");
            return ExitCode::from(1);
        }
    };
    // Routines run on their schedules for as long as the engine runs.
    {
        let _guard = runtime.enter();
        core.start_scheduler();
        core.backup_if_due();
    }
    match runtime.block_on(transport::serve(
        core,
        tokio::io::stdin(),
        tokio::io::stdout(),
    )) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "transport failed");
            ExitCode::from(1)
        }
    }
}
