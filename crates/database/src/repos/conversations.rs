//! Conversations and messages.

use rusqlite::{OptionalExtension, Row, params};
use wisp_ipc::types::{
    conversations::{
        Conversation, ConversationSummary, FocusAttachment, Message, MessageRole, MessageStatus,
    },
    models::{Locality, ModelRef},
};

use crate::{Database, DbError, DbResult};

fn role_str(r: MessageRole) -> &'static str {
    match r {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
    }
}

fn status_str(s: MessageStatus) -> &'static str {
    match s {
        MessageStatus::Streaming => "streaming",
        MessageStatus::Complete => "complete",
        MessageStatus::Cancelled => "cancelled",
        MessageStatus::Error => "error",
    }
}

fn message_from_row(r: &Row<'_>) -> rusqlite::Result<Message> {
    let role: String = r.get("role")?;
    let status: String = r.get("status")?;
    let locality: Option<String> = r.get("locality")?;
    let provider: Option<String> = r.get("provider_id")?;
    let model_key: Option<String> = r.get("model_key")?;
    let context: String = r.get("context")?;
    let ctx: serde_json::Value = serde_json::from_str(&context).unwrap_or_default();
    Ok(Message {
        id: r.get("id")?,
        conversation_id: r.get("conversation_id")?,
        role: if role == "user" {
            MessageRole::User
        } else {
            MessageRole::Assistant
        },
        content: r.get("content")?,
        status: match status.as_str() {
            "streaming" => MessageStatus::Streaming,
            "cancelled" => MessageStatus::Cancelled,
            "error" => MessageStatus::Error,
            _ => MessageStatus::Complete,
        },
        model: provider.zip(model_key).map(|(p, m)| ModelRef {
            provider_id: p,
            model: m,
        }),
        model_name: ctx
            .get("modelName")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        locality: locality.map(|l| {
            if l == "cloud" {
                Locality::Cloud
            } else {
                Locality::Local
            }
        }),
        focus: ctx
            .get("focus")
            .and_then(|f| serde_json::from_value::<FocusAttachment>(f.clone()).ok()),
        error: r.get("error")?,
        error_details: ctx
            .get("errorDetails")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        reasoning: ctx
            .get("reasoning")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        reasoning_ms: ctx
            .get("reasoningMs")
            .and_then(|v| v.as_u64())
            .map(|n| n.min(u32::MAX as u64) as u32),
        citations: ctx
            .get("citations")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        saved_memories: ctx
            .get("savedMemories")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        guidance: ctx
            .get("guidance")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        context: ctx
            .get("contextInfo")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        stage: None,
        latency_ms: r
            .get::<_, Option<i64>>("latency_ms")?
            .map(|v| v.clamp(0, u32::MAX as i64) as u32),
        created_at_ms: r.get("created_at")?,
    })
}

fn summary_from_row(r: &Row<'_>) -> rusqlite::Result<ConversationSummary> {
    Ok(ConversationSummary {
        id: r.get("id")?,
        title: r.get("title")?,
        pinned: r.get::<_, i64>("pinned")? != 0,
        message_count: r.get::<_, i64>("message_count")?.clamp(0, u32::MAX as i64) as u32,
        preview: r.get("preview")?,
        updated_at_ms: r.get("updated_at")?,
    })
}

const SUMMARY_SQL: &str = "SELECT c.id, c.title, c.pinned, c.updated_at,
        (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id) AS message_count,
        (SELECT substr(m.content, 1, 140) FROM messages m WHERE m.conversation_id = c.id ORDER BY m.created_at DESC, m.id DESC LIMIT 1) AS preview
    FROM conversations c";

impl Database {
    pub fn create_conversation(&self, title: &str) -> DbResult<String> {
        let id = wisp_common::ids::new_id();
        let now = wisp_common::time::now_ms();
        self.lock()?.execute(
            "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![id, title, now],
        )?;
        Ok(id)
    }

    pub fn conversation_exists(&self, id: &str) -> DbResult<bool> {
        Ok(self
            .lock()?
            .query_row(
                "SELECT 1 FROM conversations WHERE id = ?1",
                [id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    pub fn list_conversations(
        &self,
        query: Option<&str>,
        limit: u32,
    ) -> DbResult<Vec<ConversationSummary>> {
        let conn = self.lock()?;
        let q = query.map(str::trim).filter(|q| !q.is_empty());
        let rows = match q {
            Some(q) => {
                let like = format!(
                    "%{}%",
                    q.replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_")
                );
                let mut stmt = conn.prepare(&format!(
                    "{SUMMARY_SQL} WHERE c.archived = 0 AND (c.title LIKE ?1 ESCAPE '\\' OR EXISTS (SELECT 1 FROM messages m WHERE m.conversation_id = c.id AND m.content LIKE ?1 ESCAPE '\\'))
                     ORDER BY c.pinned DESC, c.updated_at DESC LIMIT ?2"
                ))?;
                stmt.query_map(params![like, limit], summary_from_row)?
                    .collect::<Result<Vec<_>, _>>()?
            }
            None => {
                let mut stmt = conn.prepare(&format!("{SUMMARY_SQL} WHERE c.archived = 0 ORDER BY c.pinned DESC, c.updated_at DESC LIMIT ?1"))?;
                stmt.query_map([limit], summary_from_row)?
                    .collect::<Result<Vec<_>, _>>()?
            }
        };
        Ok(rows)
    }

    pub fn conversation_summary(&self, id: &str) -> DbResult<Option<ConversationSummary>> {
        Ok(self
            .lock()?
            .query_row(
                &format!("{SUMMARY_SQL} WHERE c.id = ?1"),
                [id],
                summary_from_row,
            )
            .optional()?)
    }

    pub fn get_conversation(&self, id: &str) -> DbResult<Option<Conversation>> {
        let conn = self.lock()?;
        let head = conn
            .query_row(
                "SELECT id, title, pinned, created_at, updated_at FROM conversations WHERE id = ?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)? != 0,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((id, title, pinned, created, updated)) = head else {
            return Ok(None);
        };
        let mut stmt = conn
            .prepare("SELECT * FROM messages WHERE conversation_id = ?1 ORDER BY created_at, id")?;
        let messages = stmt
            .query_map([&id], message_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Conversation {
            id,
            title,
            pinned,
            messages,
            created_at_ms: created,
            updated_at_ms: updated,
        }))
    }

    /// Most recent finished messages, oldest first, for building model context.
    pub fn recent_messages(&self, conversation_id: &str, limit: u32) -> DbResult<Vec<Message>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM (SELECT * FROM messages WHERE conversation_id = ?1 AND status IN ('complete') ORDER BY created_at DESC, id DESC LIMIT ?2) ORDER BY created_at, id",
        )?;
        Ok(stmt
            .query_map(params![conversation_id, limit], message_from_row)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn insert_message(&self, m: &Message) -> DbResult<()> {
        let mut ctx = serde_json::Map::new();
        if let Some(f) = &m.focus {
            ctx.insert("focus".into(), serde_json::to_value(f)?);
        }
        if let Some(n) = &m.model_name {
            ctx.insert("modelName".into(), n.clone().into());
        }
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, status, provider_id, model_key, locality, context, error, latency_ms, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
            params![
                m.id,
                m.conversation_id,
                role_str(m.role),
                m.content,
                status_str(m.status),
                m.model.as_ref().map(|r| r.provider_id.clone()),
                m.model.as_ref().map(|r| r.model.clone()),
                m.locality.map(|l| if l == Locality::Cloud { "cloud" } else { "local" }),
                serde_json::Value::Object(ctx).to_string(),
                m.error,
                m.latency_ms,
                m.created_at_ms
            ],
        )?;
        conn.execute(
            "UPDATE conversations SET updated_at = ?2 WHERE id = ?1",
            params![m.conversation_id, wisp_common::time::now_ms()],
        )?;
        Ok(())
    }

    /// Persist an assistant message's current text and state.
    pub fn update_message(&self, m: &Message, tokens: (Option<u32>, Option<u32>)) -> DbResult<()> {
        let mut ctx = serde_json::Map::new();
        if let Some(n) = &m.model_name {
            ctx.insert("modelName".into(), n.clone().into());
        }
        if let Some(d) = &m.error_details {
            ctx.insert("errorDetails".into(), d.clone().into());
        }
        if let Some(r) = &m.reasoning {
            ctx.insert("reasoning".into(), r.clone().into());
        }
        if let Some(ms) = m.reasoning_ms {
            ctx.insert("reasoningMs".into(), ms.into());
        }
        if let Some(c) = &m.context {
            ctx.insert("contextInfo".into(), serde_json::to_value(c)?);
        }
        if !m.guidance.is_empty() {
            ctx.insert("guidance".into(), serde_json::to_value(&m.guidance)?);
        }
        if !m.citations.is_empty() {
            ctx.insert("citations".into(), serde_json::to_value(&m.citations)?);
        }
        if !m.saved_memories.is_empty() {
            ctx.insert(
                "savedMemories".into(),
                serde_json::to_value(&m.saved_memories)?,
            );
        }
        let n = self.lock()?.execute(
            "UPDATE messages SET content = ?2, status = ?3, provider_id = ?4, model_key = ?5, locality = ?6, context = ?7,
                 error = ?8, latency_ms = ?9, tokens_in = ?10, tokens_out = ?11, updated_at = ?12 WHERE id = ?1",
            params![
                m.id,
                m.content,
                status_str(m.status),
                m.model.as_ref().map(|r| r.provider_id.clone()),
                m.model.as_ref().map(|r| r.model.clone()),
                m.locality.map(|l| if l == Locality::Cloud { "cloud" } else { "local" }),
                serde_json::Value::Object(ctx).to_string(),
                m.error,
                m.latency_ms,
                tokens.0,
                tokens.1,
                wisp_common::time::now_ms()
            ],
        )?;
        if n == 0 {
            return Err(DbError::Sqlite(rusqlite::Error::QueryReturnedNoRows));
        }
        Ok(())
    }

    /// Mark answers left streaming by a crash or quit as cancelled (startup recovery).
    pub fn recover_streaming_messages(&self) -> DbResult<usize> {
        Ok(self.lock()?.execute(
            "UPDATE messages SET status = 'cancelled' WHERE status = 'streaming'",
            [],
        )?)
    }

    pub fn get_message(&self, id: &str) -> DbResult<Option<Message>> {
        Ok(self
            .lock()?
            .query_row(
                "SELECT * FROM messages WHERE id = ?1",
                [id],
                message_from_row,
            )
            .optional()?)
    }

    pub fn delete_message(&self, id: &str) -> DbResult<bool> {
        Ok(self
            .lock()?
            .execute("DELETE FROM messages WHERE id = ?1", [id])?
            > 0)
    }

    pub fn rename_conversation(&self, id: &str, title: &str) -> DbResult<bool> {
        Ok(self.lock()?.execute(
            "UPDATE conversations SET title = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, title, wisp_common::time::now_ms()],
        )? > 0)
    }

    pub fn pin_conversation(&self, id: &str, pinned: bool) -> DbResult<bool> {
        Ok(self.lock()?.execute(
            "UPDATE conversations SET pinned = ?2 WHERE id = ?1",
            params![id, pinned as i64],
        )? > 0)
    }

    pub fn delete_conversation(&self, id: &str) -> DbResult<bool> {
        Ok(self
            .lock()?
            .execute("DELETE FROM conversations WHERE id = ?1", [id])?
            > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(conv: &str, role: MessageRole, text: &str, at: i64) -> Message {
        Message {
            id: wisp_common::ids::new_id(),
            conversation_id: conv.into(),
            role,
            content: text.into(),
            status: MessageStatus::Complete,
            model: None,
            model_name: None,
            locality: None,
            focus: None,
            error: None,
            error_details: None,
            latency_ms: None,
            reasoning: None,
            reasoning_ms: None,
            context: None,
            guidance: vec![],
            saved_memories: vec![],
            citations: vec![],
            stage: None,
            created_at_ms: at,
        }
    }

    #[test]
    fn conversation_lifecycle() {
        let db = Database::open_in_memory().expect("db");
        let c = db.create_conversation("Docker help").expect("create");
        db.insert_message(&msg(&c, MessageRole::User, "How do I run a container?", 1))
            .expect("user");
        let mut a = msg(&c, MessageRole::Assistant, "", 2);
        a.status = MessageStatus::Streaming;
        db.insert_message(&a).expect("assistant");
        a.content = "Use docker run.".into();
        a.status = MessageStatus::Complete;
        a.model = Some(ModelRef {
            provider_id: "ollama".into(),
            model: "qwen3-vl:8b".into(),
        });
        a.model_name = Some("qwen3-vl:8b".into());
        a.locality = Some(Locality::Local);
        db.update_message(&a, (Some(10), Some(4))).expect("update");

        let conv = db.get_conversation(&c).expect("get").expect("exists");
        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.messages[1].content, "Use docker run.");
        assert_eq!(conv.messages[1].locality, Some(Locality::Local));
        assert_eq!(conv.messages[1].model_name.as_deref(), Some("qwen3-vl:8b"));

        let list = db.list_conversations(None, 10).expect("list");
        assert_eq!(list[0].message_count, 2);
        assert_eq!(list[0].preview.as_deref(), Some("Use docker run."));
        assert_eq!(
            db.list_conversations(Some("container"), 10)
                .expect("search")
                .len(),
            1
        );
        assert_eq!(
            db.list_conversations(Some("kubernetes"), 10)
                .expect("search")
                .len(),
            0
        );
        assert_eq!(
            db.list_conversations(Some("100%_"), 10)
                .expect("wildcards are literal")
                .len(),
            0
        );

        assert!(db.pin_conversation(&c, true).expect("pin"));
        assert!(db.rename_conversation(&c, "Containers").expect("rename"));
        assert!(db.delete_conversation(&c).expect("delete"));
        assert!(db.get_conversation(&c).expect("get").is_none());
    }

    #[test]
    fn recovers_interrupted_streams_and_excludes_them_from_context() {
        let db = Database::open_in_memory().expect("db");
        let c = db.create_conversation("x").expect("create");
        db.insert_message(&msg(&c, MessageRole::User, "q", 1))
            .expect("user");
        let mut a = msg(&c, MessageRole::Assistant, "partial", 2);
        a.status = MessageStatus::Streaming;
        db.insert_message(&a).expect("assistant");
        assert_eq!(db.recent_messages(&c, 10).expect("recent").len(), 1);
        assert_eq!(db.recover_streaming_messages().expect("recover"), 1);
        let conv = db.get_conversation(&c).expect("get").expect("exists");
        assert_eq!(conv.messages[1].status, MessageStatus::Cancelled);
    }
}
