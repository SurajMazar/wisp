//! Memory service (spec §67, §96): explicit saving, inspection, deletion and retrieval.
//! Embeddings come from the host (Apple NaturalLanguage, on device); without them retrieval
//! falls back to keywords.

use wisp_ipc::{
    ErrorCode, IpcError,
    types::{
        app::Empty,
        memory::{
            EmbedParams, Embeddings, MemoryCategory, MemoryCleared, MemoryItem, MemoryLevel,
            MemoryList, MemoryRef, MemorySaveParams, MemorySource,
        },
    },
};
use wisp_memory::{Candidate, MAX_CONTENT, MAX_MEMORIES};

use crate::Core;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// How many memories one prompt may include.
const RECALL_LIMIT: usize = 4;

fn clean(content: &str) -> Result<String, IpcError> {
    let c: String = content
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect::<String>()
        .trim()
        .to_owned();
    if c.is_empty() || c.chars().count() > MAX_CONTENT {
        return Err(IpcError::invalid_params(format!(
            "a memory must be 1–{MAX_CONTENT} characters"
        )));
    }
    Ok(c)
}

impl Core {
    /// On-device embeddings for `texts`, if the host can make them.
    pub(crate) async fn embed(&self, texts: Vec<String>) -> Option<Embeddings> {
        let host = self.ai().host()?;
        let params = serde_json::to_value(EmbedParams { texts }).ok()?;
        let e: Embeddings =
            serde_json::from_value(host.call("host.embed", params).await.ok()?).ok()?;
        (e.dims > 0).then_some(e)
    }

    async fn embed_memory(&self, id: &str, content: &str) {
        if let Some(e) = self.embed(vec![content.to_owned()]).await
            && let Some(v) = e.vectors.first().filter(|v| !v.is_empty())
        {
            let (id, bytes, model) = (id.to_owned(), wisp_memory::encode(v), e.model.clone());
            let _ = self
                .blocking(move |db| db.save_memory_embedding(&id, &model, &bytes, e.dims))
                .await;
        }
    }

    pub(crate) async fn memory_list(
        &self,
        query: Option<String>,
        category: Option<MemoryCategory>,
    ) -> Result<MemoryList, IpcError> {
        let items = self
            .blocking(move |db| db.list_memories(query.as_deref(), category))
            .await?;
        Ok(MemoryList {
            items,
            limit: MAX_MEMORIES,
        })
    }

    /// Store a memory (deduplicated). Fails when memory is full.
    pub(crate) async fn add_memory(
        &self,
        content: &str,
        category: MemoryCategory,
        level: MemoryLevel,
        source: MemorySource,
        agent_id: Option<String>,
        conversation_id: Option<String>,
    ) -> Result<MemoryItem, IpcError> {
        let content = clean(content)?;
        let c = content.clone();
        if let Some(id) = self.blocking(move |db| db.find_memory(&c)).await? {
            let existing = self.blocking(move |db| db.get_memory(&id)).await?;
            if let Some(m) = existing {
                return Ok(m);
            }
        }
        if self.blocking(|db| db.count_memories()).await? >= MAX_MEMORIES {
            return Err(IpcError::new(
                ErrorCode::Unavailable,
                format!("Memory is full ({MAX_MEMORIES}). Delete some on the Memory page first."),
            ));
        }
        let now = wisp_common::time::now_ms();
        let m = MemoryItem {
            id: wisp_common::ids::new_id(),
            category,
            level,
            content,
            source,
            agent_id,
            conversation_id,
            importance: if category == MemoryCategory::UserProfile {
                0.8
            } else {
                0.5
            },
            access_count: 0,
            last_accessed_at_ms: None,
            expires_at_ms: (level == MemoryLevel::Temporary).then_some(now + DAY_MS),
            created_at_ms: now,
            updated_at_ms: now,
        };
        let stored = m.clone();
        self.blocking(move |db| db.insert_memory(&stored)).await?;
        self.embed_memory(&m.id, &m.content).await;
        tracing::info!(category = ?m.category, level = ?m.level, source = ?m.source, "memory saved");
        Ok(m)
    }

    pub(crate) async fn memory_save(&self, p: MemorySaveParams) -> Result<MemoryItem, IpcError> {
        match p.id {
            None => {
                self.add_memory(
                    &p.content,
                    p.category,
                    p.level,
                    MemorySource::User,
                    None,
                    None,
                )
                .await
            }
            Some(id) => {
                if !wisp_common::ids::is_valid_id(&id) {
                    return Err(IpcError::invalid_params("invalid id"));
                }
                let content = clean(&p.content)?;
                let expires = (p.level == MemoryLevel::Temporary)
                    .then(|| wisp_common::time::now_ms() + DAY_MS);
                let (i, c) = (id.clone(), content.clone());
                if !self
                    .blocking(move |db| db.update_memory(&i, &c, p.category, p.level, expires))
                    .await?
                {
                    return Err(IpcError::new(
                        ErrorCode::NotFound,
                        "That memory no longer exists.",
                    ));
                }
                self.embed_memory(&id, &content).await;
                self.blocking(move |db| db.get_memory(&id))
                    .await?
                    .ok_or_else(|| {
                        IpcError::new(ErrorCode::NotFound, "That memory no longer exists.")
                    })
            }
        }
    }

    pub(crate) async fn memory_delete(&self, id: String) -> Result<Empty, IpcError> {
        if !wisp_common::ids::is_valid_id(&id) {
            return Err(IpcError::invalid_params("invalid id"));
        }
        self.blocking(move |db| db.delete_memory(&id)).await?;
        Ok(Empty {})
    }

    pub(crate) async fn memory_clear(&self) -> Result<MemoryCleared, IpcError> {
        let deleted = self.blocking(|db| db.clear_memories()).await?;
        tracing::info!(deleted, "memory cleared");
        Ok(MemoryCleared { deleted })
    }

    /// Save what the user clearly asked to be remembered, and forget what they asked to forget.
    /// Returns what was saved (shown in the chat with Undo).
    pub(crate) async fn learn_from(&self, text: &str, conversation_id: &str) -> Vec<MemoryRef> {
        let Ok(settings) = self.blocking(|db| db.load_settings()).await else {
            return vec![];
        };
        if !settings.memory.learn_from_chats {
            return vec![];
        }
        if let Some(target) = wisp_memory::forget_request(text) {
            let t = target.clone();
            if let Ok(all) = self.blocking(|db| db.list_memories(None, None)).await {
                for m in all
                    .into_iter()
                    .filter(|m| wisp_memory::lexical(&t, &m.content) >= 0.6)
                {
                    let id = m.id.clone();
                    let _ = self.blocking(move |db| db.delete_memory(&id)).await;
                    tracing::info!("memory forgotten on request");
                }
            }
            return vec![];
        }
        let mut saved = vec![];
        for e in wisp_memory::extract(text) {
            if let Ok(m) = self
                .add_memory(
                    &e.content,
                    e.category,
                    e.level,
                    MemorySource::Extracted,
                    None,
                    Some(conversation_id.to_owned()),
                )
                .await
            {
                saved.push(MemoryRef {
                    id: m.id,
                    content: m.content,
                });
            }
        }
        saved
    }

    /// The memories most relevant to `query` (an agent also sees its own).
    pub(crate) async fn recall(&self, query: &str, agent_id: Option<&str>) -> Vec<MemoryItem> {
        let Ok(settings) = self.blocking(|db| db.load_settings()).await else {
            return vec![];
        };
        if !settings.memory.enabled {
            return vec![];
        }
        let Ok(all) = self.blocking(|db| db.list_memories(None, None)).await else {
            return vec![];
        };
        let all: Vec<MemoryItem> = all
            .into_iter()
            .filter(|m| m.agent_id.is_none() || m.agent_id.as_deref() == agent_id)
            .collect();
        if all.is_empty() {
            return vec![];
        }
        // Embed the query and any memories not yet embedded with the same model.
        let mut vectors = std::collections::HashMap::new();
        let mut query_vec = None;
        if let Some(q) = self.embed(vec![query.chars().take(2000).collect()]).await {
            query_vec = q.vectors.into_iter().next().filter(|v| !v.is_empty());
            let model = q.model.clone();
            let m2 = model.clone();
            if let Ok(rows) = self.blocking(move |db| db.memory_embeddings(&m2)).await {
                vectors.extend(
                    rows.into_iter()
                        .map(|(id, b)| (id, wisp_memory::decode(&b))),
                );
            }
            let missing: Vec<&MemoryItem> = all
                .iter()
                .filter(|m| !vectors.contains_key(&m.id))
                .take(64)
                .collect();
            if !missing.is_empty()
                && let Some(e) = self
                    .embed(missing.iter().map(|m| m.content.clone()).collect())
                    .await
                && e.model == model
            {
                for (m, v) in missing.iter().zip(e.vectors) {
                    if v.is_empty() {
                        continue;
                    }
                    let (id, bytes, mdl, dims) = (
                        m.id.clone(),
                        wisp_memory::encode(&v),
                        e.model.clone(),
                        e.dims,
                    );
                    let _ = self
                        .blocking(move |db| db.save_memory_embedding(&id, &mdl, &bytes, dims))
                        .await;
                    vectors.insert(m.id.clone(), v);
                }
            }
        }
        let mut scored: Vec<(f32, MemoryItem)> = all
            .into_iter()
            .map(|m| {
                let s = wisp_memory::score(
                    query,
                    query_vec.as_deref(),
                    &Candidate {
                        content: &m.content,
                        category: m.category,
                        importance: m.importance,
                        embedding: vectors.get(&m.id).map(Vec::as_slice),
                    },
                );
                (s, m)
            })
            .filter(|(s, _)| *s >= wisp_memory::THRESHOLD)
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        let picked: Vec<MemoryItem> = scored
            .into_iter()
            .take(RECALL_LIMIT)
            .map(|(_, m)| m)
            .collect();
        let ids: Vec<String> = picked.iter().map(|m| m.id.clone()).collect();
        let _ = self.blocking(move |db| db.touch_memories(&ids)).await;
        picked
    }
}

/// The prompt section for recalled memories.
pub(crate) fn memory_section(items: &[MemoryItem]) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    let mut s = String::from(
        "What you know about the user (from memories they asked you to keep — use them when relevant, don't recite them):",
    );
    for m in items {
        s.push_str("\n- ");
        s.push_str(&m.content);
    }
    Some(s)
}
