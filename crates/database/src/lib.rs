//! SQLite access for Wisp. All SQL in the application lives in this crate.

mod migrations;
pub mod repos;

use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use rusqlite::Connection;

pub use migrations::{AppliedMigration, latest_version};

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration {version} ({name}) was modified after being applied (checksum mismatch)")]
    MigrationTampered { version: u32, name: String },
    #[error("database schema version {found} is newer than this build supports ({supported})")]
    SchemaTooNew { found: u32, supported: u32 },
    #[error("stored value for {key} is invalid: {source}")]
    InvalidStoredValue {
        key: String,
        source: serde_json::Error,
    },
    #[error("serialization: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("database lock poisoned")]
    Poisoned,
}

pub type DbResult<T> = Result<T, DbError>;

/// Shared handle to the application database. Cheap to clone.
///
/// Calls are synchronous; async callers must use `spawn_blocking`.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open (creating if needed) the database file and apply pending migrations.
    pub fn open(path: &Path) -> DbResult<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// In-memory database for tests.
    pub fn open_in_memory() -> DbResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> DbResult<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrations::apply(&mut conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub(crate) fn lock(&self) -> DbResult<MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| DbError::Poisoned)
    }

    /// Highest applied migration version.
    pub fn schema_version(&self) -> DbResult<u32> {
        let conn = self.lock()?;
        migrations::current_version(&conn)
    }

    pub fn applied_migrations(&self) -> DbResult<Vec<AppliedMigration>> {
        let conn = self.lock()?;
        migrations::applied(&conn)
    }
}
