//! Wisp's on-disk layout under the user data directory.

use std::path::{Path, PathBuf};

/// Environment variable overriding the data directory (tests, development).
pub const DATA_DIR_ENV: &str = "WISP_DATA_DIR";

/// Subdirectories created under the data root (spec §42.18).
pub const SUBDIRS: &[&str] = &[
    "database",
    "models",
    "files",
    "embeddings",
    "skills",
    "logs",
    "cache",
    "recordings",
    "exports",
];

#[derive(Debug, Clone)]
pub struct DataPaths {
    root: PathBuf,
}

impl DataPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolve the data root: explicit override, then `WISP_DATA_DIR`, then
    /// `~/Library/Application Support/Wisp`.
    pub fn resolve(explicit: Option<PathBuf>) -> Option<Self> {
        if let Some(p) = explicit {
            return Some(Self::new(p));
        }
        if let Some(p) = std::env::var_os(DATA_DIR_ENV) {
            return Some(Self::new(p));
        }
        let home = std::env::var_os("HOME")?;
        Some(Self::new(
            Path::new(&home).join("Library/Application Support/Wisp"),
        ))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn database_file(&self) -> PathBuf {
        self.root.join("database").join("wisp.sqlite3")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn models_dir(&self) -> PathBuf {
        self.root.join("models")
    }

    /// Create the root and all standard subdirectories (owner-only permissions).
    pub fn ensure(&self) -> std::io::Result<()> {
        for sub in SUBDIRS {
            std::fs::create_dir_all(self.root.join(sub))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.root, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
}
