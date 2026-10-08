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
