//! Internal event log: metadata only (spec §100). Never message content, audio or pixels.

use rusqlite::params;
use serde_json::Value;

use crate::{Database, DbResult};

/// Events kept for the activity log; older rows are pruned beyond this count.
const MAX_EVENTS: i64 = 10_000;

impl Database {
    pub fn record_event(&self, event_type: &str, payload: &Value) -> DbResult<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO events (id, type, payload, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                wisp_common::ids::new_id(),
                event_type,
                serde_json::to_string(payload)?,
                wisp_common::time::now_ms()
            ],
        )?;
        conn.execute(
            "DELETE FROM events WHERE id IN (SELECT id FROM events ORDER BY created_at DESC LIMIT -1 OFFSET ?1)",
            [MAX_EVENTS],
        )?;
        Ok(())
    }

    pub fn count_events(&self, event_type: &str) -> DbResult<i64> {
        Ok(self.lock()?.query_row(
            "SELECT COUNT(*) FROM events WHERE type = ?1",
            [event_type],
            |r| r.get(0),
        )?)
    }
}

impl Database {
    /// Local diagnostics (spec §105): provider, model, latency, tokens. Never prompt or answer text.
    #[allow(clippy::too_many_arguments)]
    pub fn record_diagnostic(
        &self,
        category: &str,
        name: &str,
        provider: Option<&str>,
        model: Option<&str>,
        latency_ms: Option<i64>,
        tokens: (Option<u32>, Option<u32>),
        data: Value,
    ) -> DbResult<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO diagnostics (id, category, name, provider, model, latency_ms, tokens_in, tokens_out, data, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                wisp_common::ids::new_id(),
                category,
                name,
                provider,
                model,
                latency_ms,
                tokens.0,
                tokens.1,
                serde_json::to_string(&data)?,
                wisp_common::time::now_ms()
            ],
        )?;
        conn.execute(
            "DELETE FROM diagnostics WHERE id IN (SELECT id FROM diagnostics ORDER BY created_at DESC LIMIT -1 OFFSET 5000)",
            [],
        )?;
        Ok(())
    }
}

impl Database {
    /// Average first-token latency per (provider, model) over recent answers (ms).
    pub fn recent_first_token_ms(&self) -> DbResult<Vec<(String, String, f64)>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT provider, model, AVG(json_extract(data, '$.firstTokenMs')) FROM (
               SELECT provider, model, data FROM diagnostics
               WHERE category = 'model' AND provider IS NOT NULL AND model IS NOT NULL
                 AND json_extract(data, '$.firstTokenMs') IS NOT NULL
               ORDER BY created_at DESC LIMIT 200)
             GROUP BY provider, model",
        )?;
        Ok(stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, f64>(2)?)))?
            .collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_events() {
        let db = Database::open_in_memory().expect("open");
        db.record_event(
            "invocation.started",
            &serde_json::json!({"source": "keyboard"}),
        )
        .expect("record");
        assert_eq!(db.count_events("invocation.started").expect("count"), 1);
    }
}
