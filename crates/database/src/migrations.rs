//! Forward-only, checksummed migrations embedded at compile time.
//!
//! Rules: never edit an applied migration — add a new one. A checksum mismatch on an applied
//! migration is a hard error rather than silent drift.

use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::{DbError, DbResult};

include!(concat!(env!("OUT_DIR"), "/migrations.rs"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    pub version: u32,
    pub name: String,
    pub checksum: String,
    pub applied_at: i64,
}

pub fn latest_version() -> u32 {
    MIGRATIONS.last().map(|m| m.0).unwrap_or(0)
}

fn checksum(sql: &str) -> String {
    hex::encode(Sha256::digest(sql.as_bytes()))
}

pub(crate) fn apply(conn: &mut Connection) -> DbResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY NOT NULL,
            name       TEXT NOT NULL,
            checksum   TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let current = current_version(conn)?;
    if current > latest_version() {
        return Err(DbError::SchemaTooNew {
            found: current,
            supported: latest_version(),
        });
    }

    for &(version, name, sql) in MIGRATIONS {
        let sum = checksum(sql);
        let existing: Option<String> = conn
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version = ?1",
                [version],
                |r| r.get(0),
            )
            .optional()?;
        match existing {
            Some(stored) if stored == sum => continue,
            Some(_) => {
                return Err(DbError::MigrationTampered {
                    version,
                    name: name.to_owned(),
                });
            }
            None => {
                let tx = conn.transaction()?;
                tx.execute_batch(sql)?;
                tx.execute(
                    "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
                    params![version, name, sum, wisp_common::time::now_ms()],
                )?;
                tx.commit()?;
                tracing::info!(version, name, "applied migration");
            }
        }
    }
    Ok(())
}

pub(crate) fn current_version(conn: &Connection) -> DbResult<u32> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?)
}

pub(crate) fn applied(conn: &Connection) -> DbResult<Vec<AppliedMigration>> {
    let mut stmt = conn.prepare(
        "SELECT version, name, checksum, applied_at FROM schema_migrations ORDER BY version",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(AppliedMigration {
            version: r.get(0)?,
            name: r.get(1)?,
            checksum: r.get(2)?,
            applied_at: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    #[test]
    fn applies_all_migrations_in_order_and_is_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("t.sqlite3");
        let db = Database::open(&path).expect("open");
        let applied = db.applied_migrations().expect("applied");
        assert_eq!(applied.len(), MIGRATIONS.len());
        assert!(applied.windows(2).all(|w| w[0].version < w[1].version));
        assert_eq!(db.schema_version().expect("v"), latest_version());
        drop(db);
        // Re-opening applies nothing new and doesn't error.
        let db = Database::open(&path).expect("reopen");
        assert_eq!(
            db.applied_migrations().expect("applied").len(),
            MIGRATIONS.len()
        );
    }

    #[test]
    fn detects_tampered_migration() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("t.sqlite3");
        drop(Database::open(&path).expect("open"));
        let conn = Connection::open(&path).expect("raw open");
        conn.execute(
            "UPDATE schema_migrations SET checksum = 'x' WHERE version = 1",
            [],
        )
        .expect("tamper");
        drop(conn);
        assert!(matches!(
            Database::open(&path),
            Err(DbError::MigrationTampered { version: 1, .. })
        ));
    }

    #[test]
    fn spec_tables_exist() {
        let db = Database::open_in_memory().expect("open");
        let conn = db.lock().expect("lock");
        for t in [
            "settings",
            "providers",
            "models",
            "conversations",
            "messages",
            "attachments",
            "projects",
            "agents",
            "agent_runs",
            "agent_steps",
            "skills",
            "skill_versions",
            "memories",
            "memory_embeddings",
            "routines",
            "routine_runs",
            "permissions",
            "tool_permissions",
            "tool_calls",
            "files",
            "file_chunks",
            "events",
            "diagnostics",
        ] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [t],
                    |r| r.get(0),
                )
                .expect("query");
            assert_eq!(n, 1, "missing table {t}");
        }
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .expect("pragma");
        assert_eq!(fk, 1);
    }
}
