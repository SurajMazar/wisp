//! Tool audit log and standing permissions (spec §57, §105).

use rusqlite::{OptionalExtension, params};
use wisp_ipc::types::tools::{
    ApprovalDecision, RiskLevel, ToolCallRecord, ToolCallStatus, ToolGrant, Verification,
};

use crate::{Database, DbResult};

fn status_str(s: ToolCallStatus) -> &'static str {
    match s {
        ToolCallStatus::WaitingForApproval => "waitingForApproval",
        ToolCallStatus::Running => "running",
        ToolCallStatus::Succeeded => "succeeded",
        ToolCallStatus::Failed => "failed",
        ToolCallStatus::Denied => "denied",
    }
}

fn parse_status(s: &str) -> ToolCallStatus {
    match s {
        "waitingForApproval" | "requested" => ToolCallStatus::WaitingForApproval,
        "running" => ToolCallStatus::Running,
        "succeeded" => ToolCallStatus::Succeeded,
        "denied" => ToolCallStatus::Denied,
        _ => ToolCallStatus::Failed,
    }
}

fn parse_risk(s: &str) -> RiskLevel {
    match s {
        "safe" => RiskLevel::Safe,
        "low" => RiskLevel::Low,
        "medium" => RiskLevel::Medium,
        "high" => RiskLevel::High,
        _ => RiskLevel::Critical,
    }
}

impl Database {
    /// Record a new tool call. `arguments` must already be redacted.
    pub fn insert_tool_call(
        &self,
        r: &ToolCallRecord,
        arguments: &serde_json::Value,
    ) -> DbResult<()> {
        let conn = self.lock()?;
        // Only link to a message that exists (the FK would reject anything else).
        let message_id: Option<String> = match &r.message_id {
            Some(id) => conn
                .query_row("SELECT id FROM messages WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .optional()?,
            None => None,
        };
        conn.execute(
            "INSERT INTO tool_calls (id, tool, risk, arguments, decision, status, result, error, message_id, created_at, finished_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                r.id,
                r.tool,
                r.risk.as_str(),
                arguments.to_string(),
                r.decision,
                status_str(r.status),
                serde_json::json!({ "title": r.title, "verification": r.verification }).to_string(),
                r.error,
                message_id,
                r.created_at_ms,
                r.finished_at_ms,
            ],
        )?;
        Ok(())
    }

    pub fn update_tool_call(&self, r: &ToolCallRecord) -> DbResult<()> {
        self.lock()?.execute(
            "UPDATE tool_calls SET decision = ?2, status = ?3, result = ?4, error = ?5, finished_at = ?6 WHERE id = ?1",
            params![
                r.id,
                r.decision,
                status_str(r.status),
                serde_json::json!({ "title": r.title, "verification": r.verification }).to_string(),
                r.error,
                r.finished_at_ms,
            ],
        )?;
        Ok(())
    }

    /// Most recent first.
    pub fn list_tool_calls(&self, limit: u32) -> DbResult<Vec<ToolCallRecord>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, tool, risk, decision, status, result, error, message_id, created_at, finished_at
             FROM tool_calls ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit.min(500)], |row| {
            let result: Option<String> = row.get(5)?;
            let meta: serde_json::Value = result
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            let tool: String = row.get(1)?;
            Ok(ToolCallRecord {
                id: row.get(0)?,
                title: meta["title"]
                    .as_str()
                    .map_or_else(|| tool.clone(), str::to_owned),
                tool,
                risk: parse_risk(&row.get::<_, String>(2)?),
                decision: row.get(3)?,
                status: parse_status(&row.get::<_, String>(4)?),
                verification: serde_json::from_value::<Verification>(meta["verification"].clone())
                    .ok(),
                error: row.get(6)?,
                message_id: row.get(7)?,
                created_at_ms: row.get(8)?,
                finished_at_ms: row.get(9)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Calls left unfinished by a crash are marked failed at startup.
    pub fn fail_unfinished_tool_calls(&self) -> DbResult<usize> {
        Ok(self.lock()?.execute(
            "UPDATE tool_calls SET status = 'failed', error = 'Interrupted', finished_at = ?1
             WHERE status IN ('waitingForApproval', 'running', 'requested')",
            [wisp_common::time::now_ms()],
        )?)
    }

    pub fn list_tool_grants(&self) -> DbResult<Vec<ToolGrant>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, tool, decision, created_at FROM tool_permissions
             WHERE revoked_at IS NULL AND agent_id IS NULL ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            let d: String = row.get(2)?;
            Ok(ToolGrant {
                id: row.get(0)?,
                tool: row.get(1)?,
                decision: if d == "deny" {
                    ApprovalDecision::Deny
                } else {
                    ApprovalDecision::AlwaysAllow
                },
                created_at_ms: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Record a standing decision for a tool, replacing any previous one.
    pub fn set_tool_grant(&self, tool: &str, decision: ApprovalDecision) -> DbResult<ToolGrant> {
        let now = wisp_common::time::now_ms();
        let conn = self.lock()?;
        conn.execute(
            "UPDATE tool_permissions SET revoked_at = ?2 WHERE tool = ?1 AND agent_id IS NULL AND revoked_at IS NULL",
            params![tool, now],
        )?;
        let id = wisp_common::ids::new_id();
        let d = if decision == ApprovalDecision::Deny {
            "deny"
        } else {
            "alwaysAllow"
        };
        conn.execute(
            "INSERT INTO tool_permissions (id, tool, decision, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, tool, d, now],
        )?;
        Ok(ToolGrant {
            id,
            tool: tool.into(),
            decision,
            created_at_ms: now,
        })
    }

    /// Returns whether a grant was revoked.
    pub fn revoke_tool_grant(&self, id: &str) -> DbResult<bool> {
        Ok(self.lock()?.execute(
            "UPDATE tool_permissions SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
            params![id, wisp_common::time::now_ms()],
        )? > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().expect("tmp");
        let db = Database::open(&dir.path().join("t.sqlite3")).expect("open");
        (dir, db)
    }

    fn record(id: &str) -> ToolCallRecord {
        ToolCallRecord {
            id: id.into(),
            tool: "click".into(),
            title: "Click “Save”".into(),
            risk: RiskLevel::Low,
            decision: "allowOnce".into(),
            status: ToolCallStatus::Running,
            verification: None,
            error: None,
            message_id: Some("no-such-message".into()),
            created_at_ms: 5,
            finished_at_ms: None,
        }
    }

    #[test]
    fn audits_calls_and_recovers_interrupted_ones() {
        let (_d, db) = db();
        let mut r = record(&wisp_common::ids::new_id());
        db.insert_tool_call(&r, &serde_json::json!({"tool": "click"}))
            .expect("insert");
        r.status = ToolCallStatus::Succeeded;
        r.verification = Some(Verification {
            status: wisp_ipc::types::tools::VerificationStatus::Verified,
            detail: "The window changed".into(),
        });
        db.update_tool_call(&r).expect("update");
        let list = db.list_tool_calls(10).expect("list");
        assert_eq!(list[0].title, "Click “Save”");
        assert_eq!(list[0].status, ToolCallStatus::Succeeded);
        assert_eq!(list[0].message_id, None, "dangling message link dropped");
        assert!(list[0].verification.is_some());

        let other = record(&wisp_common::ids::new_id());
        db.insert_tool_call(&other, &serde_json::json!({}))
            .expect("insert");
        assert_eq!(db.fail_unfinished_tool_calls().expect("recover"), 1);
    }

    #[test]
    fn grants_replace_and_revoke() {
        let (_d, db) = db();
        db.set_tool_grant("click", ApprovalDecision::AlwaysAllow)
            .expect("grant");
        let g = db
            .set_tool_grant("click", ApprovalDecision::Deny)
            .expect("grant");
        let list = db.list_tool_grants().expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].decision, ApprovalDecision::Deny);
        assert!(db.revoke_tool_grant(&g.id).expect("revoke"));
        assert!(db.list_tool_grants().expect("list").is_empty());
    }
}
