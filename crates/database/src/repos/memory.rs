//! Memories and their embeddings (spec §67).

use rusqlite::{OptionalExtension, Row, params};
use wisp_ipc::types::memory::{MemoryCategory, MemoryItem, MemoryLevel, MemorySource};

use crate::{Database, DbResult};

fn s<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn e<T: serde::de::DeserializeOwned>(v: &str, d: T) -> T {
    serde_json::from_value(serde_json::Value::String(v.to_owned())).unwrap_or(d)
}

fn from_row(r: &Row<'_>) -> rusqlite::Result<MemoryItem> {
    Ok(MemoryItem {
        id: r.get("id")?,
        category: e(&r.get::<_, String>("category")?, MemoryCategory::Fact),
        level: e(&r.get::<_, String>("level")?, MemoryLevel::Persistent),
        content: r.get("content")?,
        source: e(&r.get::<_, String>("source")?, MemorySource::User),
        agent_id: r.get("agent_id")?,
        conversation_id: r.get("conversation_id")?,
        importance: r.get::<_, f64>("importance")? as f32,
        access_count: r.get("access_count")?,
        last_accessed_at_ms: r.get("last_accessed_at")?,
        expires_at_ms: r.get("expires_at")?,
        created_at_ms: r.get("created_at")?,
        updated_at_ms: r.get("updated_at")?,
    })
}

const COLS: &str = "id, category, level, content, source, agent_id, conversation_id, importance, access_count, last_accessed_at, expires_at, created_at, updated_at";

impl Database {
    /// Newest first. `query` filters by words in the content.
    pub fn list_memories(
        &self,
        query: Option<&str>,
        category: Option<MemoryCategory>,
    ) -> DbResult<Vec<MemoryItem>> {
        let conn = self.lock()?;
        let like = query.map(|q| format!("%{}%", q.replace(['%', '_'], "")));
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLS} FROM memories WHERE (?1 IS NULL OR content LIKE ?1) AND (?2 IS NULL OR category = ?2)
             AND (expires_at IS NULL OR expires_at > ?3) ORDER BY updated_at DESC"
        ))?;
        Ok(stmt
            .query_map(
                params![like, category.map(|c| s(&c)), wisp_common::time::now_ms()],
                from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get_memory(&self, id: &str) -> DbResult<Option<MemoryItem>> {
        Ok(self
            .lock()?
            .query_row(
                &format!("SELECT {COLS} FROM memories WHERE id = ?1"),
                [id],
                from_row,
            )
            .optional()?)
    }

    pub fn count_memories(&self) -> DbResult<u32> {
        Ok(self
            .lock()?
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))?)
    }

    /// The memory with exactly this content (case-insensitive), if any.
    pub fn find_memory(&self, content: &str) -> DbResult<Option<String>> {
        Ok(self
            .lock()?
            .query_row(
                "SELECT id FROM memories WHERE lower(content) = lower(?1)",
                [content],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn insert_memory(&self, m: &MemoryItem) -> DbResult<()> {
        let conn = self.lock()?;
        let agent: Option<String> = match &m.agent_id {
            Some(a) => conn
                .query_row("SELECT id FROM agents WHERE id = ?1", [a], |r| r.get(0))
                .optional()?,
            None => None,
        };
        let conversation: Option<String> = match &m.conversation_id {
            Some(c) => conn
                .query_row("SELECT id FROM conversations WHERE id = ?1", [c], |r| {
                    r.get(0)
                })
                .optional()?,
            None => None,
        };
        conn.execute(
            &format!("INSERT INTO memories ({COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"),
            params![
                m.id,
                s(&m.category),
                s(&m.level),
                m.content,
                s(&m.source),
                agent,
                conversation,
                m.importance as f64,
                m.access_count,
                m.last_accessed_at_ms,
                m.expires_at_ms,
                m.created_at_ms,
                m.updated_at_ms
            ],
        )?;
        Ok(())
    }

    pub fn update_memory(
        &self,
        id: &str,
        content: &str,
        category: MemoryCategory,
        level: MemoryLevel,
        expires_at: Option<i64>,
    ) -> DbResult<bool> {
        let conn = self.lock()?;
        let n = conn.execute(
            "UPDATE memories SET content = ?2, category = ?3, level = ?4, expires_at = ?5, source = 'user', updated_at = ?6 WHERE id = ?1",
            params![id, content, s(&category), s(&level), expires_at, wisp_common::time::now_ms()],
        )?;
        conn.execute("DELETE FROM memory_embeddings WHERE memory_id = ?1", [id])?;
        Ok(n > 0)
    }

    pub fn delete_memory(&self, id: &str) -> DbResult<bool> {
        Ok(self
            .lock()?
            .execute("DELETE FROM memories WHERE id = ?1", [id])?
            > 0)
    }

    pub fn clear_memories(&self) -> DbResult<u32> {
        Ok(self.lock()?.execute("DELETE FROM memories", [])? as u32)
    }

    /// Drop expired memories and (at startup) session ones.
    pub fn prune_memories(&self, include_session: bool) -> DbResult<u32> {
        let conn = self.lock()?;
        let mut n = conn.execute(
            "DELETE FROM memories WHERE expires_at IS NOT NULL AND expires_at <= ?1",
            [wisp_common::time::now_ms()],
        )?;
        if include_session {
            n += conn.execute("DELETE FROM memories WHERE level = 'session'", [])?;
        }
        Ok(n as u32)
    }

    pub fn touch_memories(&self, ids: &[String]) -> DbResult<()> {
        let conn = self.lock()?;
        let now = wisp_common::time::now_ms();
        for id in ids {
            conn.execute(
                "UPDATE memories SET access_count = access_count + 1, last_accessed_at = ?2 WHERE id = ?1",
                params![id, now],
            )?;
        }
        Ok(())
    }

    pub fn save_memory_embedding(
        &self,
        id: &str,
        model: &str,
        vector: &[u8],
        dims: u32,
    ) -> DbResult<()> {
        self.lock()?.execute(
            "INSERT INTO memory_embeddings (memory_id, model, dims, vector, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(memory_id) DO UPDATE SET model = excluded.model, dims = excluded.dims, vector = excluded.vector",
            params![id, model, dims, vector, wisp_common::time::now_ms()],
        )?;
        Ok(())
    }

    /// (memory id, vector bytes) for one embedding model.
    pub fn memory_embeddings(&self, model: &str) -> DbResult<Vec<(String, Vec<u8>)>> {
        let conn = self.lock()?;
        let mut stmt =
            conn.prepare("SELECT memory_id, vector FROM memory_embeddings WHERE model = ?1")?;
        Ok(stmt
            .query_map([model], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(content: &str, level: MemoryLevel, expires: Option<i64>) -> MemoryItem {
        MemoryItem {
            id: wisp_common::ids::new_id(),
            category: MemoryCategory::Preference,
            level,
            content: content.into(),
            source: MemorySource::Extracted,
            agent_id: Some("ghost".into()),
            conversation_id: None,
            importance: 0.5,
            access_count: 0,
            last_accessed_at_ms: None,
            expires_at_ms: expires,
            created_at_ms: 1,
            updated_at_ms: 1,
        }
    }

    #[test]
    fn stores_filters_prunes_and_embeds() {
        let dir = tempfile::tempdir().expect("tmp");
        let db = Database::open(&dir.path().join("t.sqlite3")).expect("open");
        let tea = item("The user prefers green tea.", MemoryLevel::Persistent, None);
        db.insert_memory(&tea).expect("insert");
        db.insert_memory(&item("Out of office.", MemoryLevel::Temporary, Some(2)))
            .expect("insert");
        db.insert_memory(&item("Session note.", MemoryLevel::Session, None))
            .expect("insert");
        assert_eq!(db.list_memories(Some("tea"), None).expect("q").len(), 1);
        assert_eq!(
            db.get_memory(&tea.id).expect("get").expect("tea").agent_id,
            None,
            "dangling agent link dropped"
        );
        assert_eq!(
            db.find_memory("the user PREFERS green tea.").expect("find"),
            Some(tea.id.clone())
        );
        assert_eq!(db.prune_memories(true).expect("prune"), 2);
        db.save_memory_embedding(&tea.id, "m", &[0, 0, 128, 63], 1)
            .expect("embed");
        assert_eq!(db.memory_embeddings("m").expect("emb").len(), 1);
        db.update_memory(
            &tea.id,
            "Prefers oolong.",
            MemoryCategory::Preference,
            MemoryLevel::Persistent,
            None,
        )
        .expect("update");
        assert!(
            db.memory_embeddings("m").expect("emb").is_empty(),
            "edits drop the stale embedding"
        );
        db.touch_memories(std::slice::from_ref(&tea.id))
            .expect("touch");
        assert_eq!(
            db.get_memory(&tea.id)
                .expect("get")
                .expect("tea")
                .access_count,
            1
        );
        assert_eq!(db.clear_memories().expect("clear"), 1);
    }
}
