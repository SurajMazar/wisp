//! Process configuration from command-line arguments and environment.

use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub data_dir: Option<PathBuf>,
    /// When true, logs also go to stderr (development). stdout is reserved for IPC frames.
    pub log_stderr: bool,
}

impl Config {
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut cfg = Config::default();
        let mut it = args.into_iter().skip(1);
        while let Some(a) = it.next() {
            match a.as_str() {
                "--data-dir" => {
                    cfg.data_dir = Some(it.next().ok_or("--data-dir needs a value")?.into());
                }
                "--log-stderr" => cfg.log_stderr = true,
                "--version" => return Err(format!("wisp-core {}", env!("CARGO_PKG_VERSION"))),
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        Ok(cfg)
    }
}
