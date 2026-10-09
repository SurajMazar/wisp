//! Imported files and their indexed chunks (spec §68–69).

use rusqlite::{OptionalExtension, Row, params};
use wisp_ipc::types::files::{FileItem, FileStatus};

use crate::{Database, DbResult};

#[derive(Debug, Clone, PartialEq)]
pub struct NewChunk {
    pub seq: u32,
    pub content: String,
    /// JSON: {"page": n?, "lineStart": a, "lineEnd": b, "citation": "…"}
    pub locator: serde_json::Value,
    pub vector: Option<(String, u32, Vec<u8>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredChunk {
    pub file_id: String,
    pub file_name: String,
    pub content: String,
    pub citation: String,
    pub model: Option<String>,
    pub vector: Option<Vec<u8>>,
}

fn status_str(s: FileStatus) -> &'static str {
    match s {
        FileStatus::Pending => "pending",
        FileStatus::Indexing => "indexing",
        FileStatus::Indexed => "indexed",
        FileStatus::Failed => "failed",
    }
}

fn from_row(r: &Row<'_>) -> rusqlite::Result<FileItem> {
    let status: String = r.get("status")?;
    Ok(FileItem {
        id: r.get("id")?,
        name: r.get("display_name")?,
        mime: r.get("mime_type")?,
        size_bytes: r.get::<_, i64>("size_bytes")? as u64,
        status: match status.as_str() {
            "indexed" => FileStatus::Indexed,
            "indexing" => FileStatus::Indexing,
            "failed" => FileStatus::Failed,
            _ => FileStatus::Pending,
        },
        error: r.get("error")?,
        chunks: r.get("chunks")?,
        created_at_ms: r.get("created_at")?,
        indexed_at_ms: r.get("indexed_at")?,
    })
}

const SELECT: &str = "SELECT f.*, (SELECT COUNT(*) FROM file_chunks c WHERE c.file_id = f.id) AS chunks FROM files f";

impl Database {
    pub fn insert_file(
        &self,
        id: &str,
        name: &str,
        stored_path: &str,
        mime: &str,
        size: u64,
        sha256: &str,
    ) -> DbResult<()> {
        self.lock()?.execute(
            "INSERT INTO files (id, display_name, stored_path, mime_type, size_bytes, sha256, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7)",
            params![id, name, stored_path, mime, size as i64, sha256, wisp_common::time::now_ms()],
        )?;
        Ok(())
    }

    pub fn file_by_sha(&self, sha256: &str) -> DbResult<Option<String>> {
        Ok(self
            .lock()?
            .query_row("SELECT id FROM files WHERE sha256 = ?1", [sha256], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn get_file(&self, id: &str) -> DbResult<Option<FileItem>> {
        Ok(self
            .lock()?
            .query_row(&format!("{SELECT} WHERE f.id = ?1"), [id], from_row)
            .optional()?)
    }

    pub fn file_stored_path(&self, id: &str) -> DbResult<Option<String>> {
        Ok(self
            .lock()?
            .query_row("SELECT stored_path FROM files WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn list_files(&self) -> DbResult<Vec<FileItem>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY f.created_at DESC"))?;
        Ok(stmt
            .query_map([], from_row)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn set_file_status(
        &self,
        id: &str,
        status: FileStatus,
        error: Option<&str>,
    ) -> DbResult<()> {
        let indexed = (status == FileStatus::Indexed).then(wisp_common::time::now_ms);
        self.lock()?.execute(
            "UPDATE files SET status = ?2, error = ?3, indexed_at = COALESCE(?4, indexed_at) WHERE id = ?1",
            params![id, status_str(status), error, indexed],
        )?;
        Ok(())
    }

    /// Replace a file's chunks.
    pub fn replace_chunks(&self, file_id: &str, chunks: &[NewChunk]) -> DbResult<()> {
        let now = wisp_common::time::now_ms();
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM file_chunks WHERE file_id = ?1", [file_id])?;
        for c in chunks {
            let (model, dims, vector) = match &c.vector {
                Some((m, d, v)) => (Some(m.as_str()), Some(*d), Some(v.as_slice())),
                None => (None, None, None),
            };
            tx.execute(
                "INSERT INTO file_chunks (id, file_id, seq, content, locator, model, dims, vector, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![wisp_common::ids::new_id(), file_id, c.seq, c.content, c.locator.to_string(), model, dims, vector, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Every chunk of indexed files (for search). Bounded by what's been imported.
    pub fn searchable_chunks(&self) -> DbResult<Vec<StoredChunk>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT c.file_id, f.display_name, c.content, c.locator, c.model, c.vector FROM file_chunks c
             JOIN files f ON f.id = c.file_id WHERE f.status = 'indexed' ORDER BY c.file_id, c.seq",
        )?;
        Ok(stmt
            .query_map([], |r| {
                let locator: serde_json::Value =
                    serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default();
                Ok(StoredChunk {
                    file_id: r.get(0)?,
                    file_name: r.get(1)?,
                    content: r.get(2)?,
                    citation: locator["citation"].as_str().unwrap_or_default().to_owned(),
                    model: r.get(4)?,
                    vector: r.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn delete_file(&self, id: &str) -> DbResult<bool> {
        Ok(self
            .lock()?
            .execute("DELETE FROM files WHERE id = ?1", [id])?
            > 0)
    }

    /// Files left mid-index by a crash are indexed again.
    pub fn unfinished_files(&self) -> DbResult<Vec<String>> {
        let conn = self.lock()?;
        let mut stmt =
            conn.prepare("SELECT id FROM files WHERE status IN ('pending', 'indexing')")?;
        Ok(stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_chunks_and_search_rows() {
        let dir = tempfile::tempdir().expect("tmp");
        let db = Database::open(&dir.path().join("t.sqlite3")).expect("open");
        db.insert_file(
            "f1",
            "notes.md",
            "files/f1-notes.md",
            "text/markdown",
            10,
            "abc",
        )
        .expect("insert");
        assert_eq!(db.file_by_sha("abc").expect("sha").as_deref(), Some("f1"));
        assert_eq!(db.unfinished_files().expect("unfinished"), vec!["f1"]);
        let chunk = |seq| NewChunk {
            seq,
            content: format!("chunk {seq}"),
            locator: serde_json::json!({"citation": format!("notes.md, line {seq}")}),
            vector: Some(("m".into(), 1, vec![0, 0, 128, 63])),
        };
        db.replace_chunks("f1", &[chunk(1), chunk(2)])
            .expect("chunks");
        assert!(
            db.searchable_chunks().expect("rows").is_empty(),
            "not searchable until indexed"
        );
        db.set_file_status("f1", FileStatus::Indexed, None)
            .expect("status");
        let rows = db.searchable_chunks().expect("rows");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].citation, "notes.md, line 2");
        let f = db.get_file("f1").expect("get").expect("file");
        assert_eq!((f.status, f.chunks), (FileStatus::Indexed, 2));
        assert!(f.indexed_at_ms.is_some());
        assert!(db.delete_file("f1").expect("delete"));
        assert!(
            db.searchable_chunks().expect("rows").is_empty(),
            "chunks cascade"
        );
    }
}
