//! Agents, their runs and run steps (spec §60–65).

use rusqlite::{OptionalExtension, Row, params};
use wisp_ipc::types::agents::{
    AgentDefinition, AgentLimits, AgentRun, AgentSaveParams, PlanItem, RunStatus, RunStep,
    RunStepStatus, StepKind,
};

use crate::{Database, DbResult};

fn enum_str<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn enum_from<T: serde::de::DeserializeOwned>(s: &str, fallback: T) -> T {
    serde_json::from_value(serde_json::Value::String(s.to_owned())).unwrap_or(fallback)
}

fn agent_from_row(r: &Row<'_>) -> rusqlite::Result<AgentDefinition> {
    let model: String = r.get("model_pref")?;
    let tools: String = r.get("tools")?;
    let limits: String = r.get("limits")?;
    let skills: Option<String> = r.get("skills")?;
    Ok(AgentDefinition {
        id: r.get("id")?,
        name: r.get("name")?,
        description: r.get("description")?,
        avatar: r
            .get::<_, Option<String>>("avatar")?
            .unwrap_or_else(|| "🤖".into()),
        instructions: r.get("instructions")?,
        model: serde_json::from_str(&model).ok(),
        tools: serde_json::from_str(&tools).unwrap_or_default(),
        skills: skills
            .map(|s| s.split(',').map(str::to_owned).collect())
            .unwrap_or_default(),
        limits: serde_json::from_str::<AgentLimits>(&limits).unwrap_or_default(),
        enabled: r.get::<_, i64>("enabled")? == 1,
        created_at_ms: r.get("created_at")?,
        updated_at_ms: r.get("updated_at")?,
        last_run_at_ms: r.get("last_run")?,
    })
}

const AGENT_SELECT: &str = "SELECT a.*, (SELECT MAX(started_at) FROM agent_runs r WHERE r.agent_id = a.id) AS last_run,
     (SELECT group_concat(skill_id) FROM agent_skills s WHERE s.agent_id = a.id) AS skills FROM agents a";

impl Database {
    pub fn list_agents(&self) -> DbResult<Vec<AgentDefinition>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(&format!("{AGENT_SELECT} ORDER BY a.name COLLATE NOCASE"))?;
        Ok(stmt
            .query_map([], agent_from_row)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get_agent(&self, id: &str) -> DbResult<Option<AgentDefinition>> {
        let conn = self.lock()?;
        Ok(conn
            .query_row(
                &format!("{AGENT_SELECT} WHERE a.id = ?1"),
                [id],
                agent_from_row,
            )
            .optional()?)
    }

    /// Create (no id) or update an agent. Unknown skill ids are ignored.
    pub fn save_agent(&self, p: &AgentSaveParams) -> DbResult<String> {
        let now = wisp_common::time::now_ms();
        let id = p.id.clone().unwrap_or_else(wisp_common::ids::new_id);
        let model = p.model.as_ref().map_or_else(
            || "{}".to_owned(),
            |m| serde_json::to_string(m).unwrap_or_else(|_| "{}".into()),
        );
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO agents (id, name, description, avatar, instructions, model_pref, tools, limits, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, description = excluded.description, avatar = excluded.avatar,
               instructions = excluded.instructions, model_pref = excluded.model_pref, tools = excluded.tools,
               limits = excluded.limits, enabled = excluded.enabled, updated_at = excluded.updated_at",
            params![
                id,
                p.name,
                p.description,
                p.avatar,
                p.instructions,
                model,
                serde_json::to_string(&p.tools)?,
                serde_json::to_string(&p.limits)?,
                p.enabled as i64,
                now
            ],
        )?;
        tx.execute("DELETE FROM agent_skills WHERE agent_id = ?1", [&id])?;
        for s in &p.skills {
            tx.execute(
                "INSERT OR IGNORE INTO agent_skills (agent_id, skill_id) SELECT ?1, id FROM skills WHERE id = ?2",
                params![id, s],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn delete_agent(&self, id: &str) -> DbResult<bool> {
        Ok(self
            .lock()?
            .execute("DELETE FROM agents WHERE id = ?1", [id])?
            > 0)
    }

    /// Insert or update a run's header (steps are stored separately).
    pub fn save_run(&self, r: &AgentRun) -> DbResult<()> {
        let meta = serde_json::json!({
            "items": r.plan, "agentName": r.agent_name, "avatar": r.avatar, "limits": r.limits,
        });
        let conn = self.lock()?;
        // Only link to rows that exist.
        let agent: Option<String> = match &r.agent_id {
            Some(a) => conn
                .query_row("SELECT id FROM agents WHERE id = ?1", [a], |row| row.get(0))
                .optional()?,
            None => None,
        };
        let conversation: Option<String> = match &r.conversation_id {
            Some(c) => conn
                .query_row("SELECT id FROM conversations WHERE id = ?1", [c], |row| {
                    row.get(0)
                })
                .optional()?,
            None => None,
        };
        conn.execute(
            "INSERT INTO agent_runs (id, agent_id, conversation_id, trigger, goal, status, plan, error, steps_used, tool_calls_used, tokens_used, started_at, finished_at)
             VALUES (?1, ?2, ?3, 'user', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET status = excluded.status, plan = excluded.plan, error = excluded.error,
               steps_used = excluded.steps_used, tool_calls_used = excluded.tool_calls_used, tokens_used = excluded.tokens_used,
               finished_at = excluded.finished_at",
            params![
                r.id,
                agent,
                conversation,
                r.goal,
                enum_str(&r.status),
                meta.to_string(),
                r.note,
                r.steps_used,
                r.tool_calls_used,
                r.tokens_used,
                r.started_at_ms,
                r.finished_at_ms
            ],
        )?;
        Ok(())
    }

    pub fn save_run_step(&self, run_id: &str, s: &RunStep) -> DbResult<()> {
        let now = wisp_common::time::now_ms();
        self.lock()?.execute(
            "INSERT INTO agent_steps (id, run_id, seq, kind, title, status, detail, started_at, finished_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(run_id, seq) DO UPDATE SET title = excluded.title, status = excluded.status,
               detail = excluded.detail, finished_at = excluded.finished_at",
            params![
                wisp_common::ids::new_id(),
                run_id,
                s.seq,
                enum_str(&s.kind),
                s.title,
                enum_str(&s.status),
                serde_json::json!({ "detail": s.detail }).to_string(),
                now,
                (s.status != RunStepStatus::Running).then_some(now),
            ],
        )?;
        Ok(())
    }

    /// Newest first, with steps.
    pub fn list_runs(&self, agent_id: Option<&str>, limit: u32) -> DbResult<Vec<AgentRun>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, conversation_id, goal, status, plan, error, steps_used, tool_calls_used, tokens_used, started_at, finished_at
             FROM agent_runs WHERE (?1 IS NULL OR agent_id = ?1) ORDER BY started_at DESC LIMIT ?2",
        )?;
        let mut runs = stmt
            .query_map(params![agent_id, limit.min(200)], |r| {
                let meta: serde_json::Value =
                    serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default();
                Ok(AgentRun {
                    id: r.get(0)?,
                    agent_id: r.get(1)?,
                    conversation_id: r.get(2)?,
                    goal: r.get(3)?,
                    status: enum_from(&r.get::<_, String>(4)?, RunStatus::Failed),
                    plan: serde_json::from_value::<Vec<PlanItem>>(meta["items"].clone())
                        .unwrap_or_default(),
                    agent_name: meta["agentName"].as_str().unwrap_or("Assistant").to_owned(),
                    avatar: meta["avatar"].as_str().unwrap_or("🤖").to_owned(),
                    limits: serde_json::from_value(meta["limits"].clone()).unwrap_or_default(),
                    note: r.get(6)?,
                    steps_used: r.get(7)?,
                    tool_calls_used: r.get(8)?,
                    tokens_used: r.get(9)?,
                    started_at_ms: r.get(10)?,
                    finished_at_ms: r.get(11)?,
                    steps: vec![],
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut steps = conn.prepare("SELECT seq, kind, title, status, detail FROM agent_steps WHERE run_id = ?1 ORDER BY seq")?;
        for run in &mut runs {
            run.steps = steps
                .query_map([&run.id], |r| {
                    let detail: serde_json::Value =
                        serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default();
                    Ok(RunStep {
                        seq: r.get(0)?,
                        kind: enum_from(&r.get::<_, String>(1)?, StepKind::Message),
                        title: r.get(2)?,
                        status: enum_from(&r.get::<_, String>(3)?, RunStepStatus::Failed),
                        detail: detail["detail"].as_str().map(str::to_owned),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
        }
        Ok(runs)
    }

    /// Runs interrupted by a quit or crash are marked cancelled at startup.
    pub fn recover_unfinished_runs(&self) -> DbResult<usize> {
        let now = wisp_common::time::now_ms();
        let conn = self.lock()?;
        conn.execute(
            "UPDATE agent_steps SET status = 'skipped', finished_at = ?1 WHERE status = 'running'",
            [now],
        )?;
        Ok(conn.execute(
            "UPDATE agent_runs SET status = 'cancelled', error = COALESCE(error, 'Wisp was closed during the run.'), finished_at = ?1
             WHERE status IN ('planning', 'running', 'paused', 'waitingPermission', 'stuck', 'needsInput')",
            [now],
        )?)
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

    fn save(db: &Database, id: Option<String>, name: &str) -> String {
        db.save_agent(&AgentSaveParams {
            id,
            name: name.into(),
            description: "Helps".into(),
            avatar: "🧭".into(),
            instructions: "Be careful".into(),
            model: None,
            tools: vec!["click".into()],
            skills: vec!["no-such-skill".into()],
            limits: AgentLimits::default(),
            enabled: true,
        })
        .expect("save")
    }

    #[test]
    fn agents_crud() {
        let (_d, db) = db();
        let id = save(&db, None, "Helper");
        save(&db, Some(id.clone()), "Desk Helper");
        let all = db.list_agents().expect("list");
        assert_eq!(all.len(), 1);
        assert_eq!(
            (all[0].name.as_str(), all[0].avatar.as_str()),
            ("Desk Helper", "🧭")
        );
        assert_eq!(all[0].tools, vec!["click"]);
        assert!(all[0].skills.is_empty(), "unknown skills ignored");
        assert!(db.delete_agent(&id).expect("delete"));
        assert!(db.get_agent(&id).expect("get").is_none());
    }

    #[test]
    fn runs_and_steps_round_trip_and_recover() {
        let (_d, db) = db();
        let agent = save(&db, None, "Helper");
        let mut run = AgentRun {
            id: wisp_common::ids::new_id(),
            agent_id: Some(agent.clone()),
            agent_name: "Helper".into(),
            avatar: "🧭".into(),
            conversation_id: Some("missing".into()),
            goal: "Save the file".into(),
            status: RunStatus::Running,
            plan: vec![PlanItem {
                title: "Press save".into(),
                status: wisp_ipc::types::agents::PlanItemStatus::Active,
            }],
            steps: vec![],
            note: None,
            steps_used: 1,
            tool_calls_used: 0,
            tokens_used: 900,
            limits: AgentLimits::default(),
            started_at_ms: 10,
            finished_at_ms: None,
        };
        db.save_run(&run).expect("run");
        let step = RunStep {
            seq: 1,
            kind: StepKind::Act,
            title: "Click “Save”".into(),
            status: RunStepStatus::Running,
            detail: None,
        };
        db.save_run_step(&run.id, &step).expect("step");
        db.save_run_step(
            &run.id,
            &RunStep {
                status: RunStepStatus::Succeeded,
                detail: Some("Saved".into()),
                ..step
            },
        )
        .expect("step");
        run.status = RunStatus::Completed;
        db.save_run(&run).expect("run");
        let runs = db.list_runs(Some(&agent), 10).expect("list");
        assert_eq!(runs[0].status, RunStatus::Completed);
        assert_eq!(runs[0].conversation_id, None);
        assert_eq!(runs[0].plan[0].title, "Press save");
        assert_eq!(runs[0].steps[0].detail.as_deref(), Some("Saved"));
        assert_eq!(
            db.get_agent(&agent)
                .expect("get")
                .expect("agent")
                .last_run_at_ms,
            Some(10)
        );

        run.id = wisp_common::ids::new_id();
        run.status = RunStatus::Paused;
        db.save_run(&run).expect("run");
        assert_eq!(db.recover_unfinished_runs().expect("recover"), 1);
    }
}
