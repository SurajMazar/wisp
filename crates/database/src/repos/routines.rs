//! Routines and their run history (spec §66).

use rusqlite::{OptionalExtension, Row, params};
use wisp_ipc::types::routines::{
    NotifyPolicy, RetryPolicy, Routine, RoutineRun, RoutineRunStatus, RoutineSaveParams,
};

use crate::{Database, DbResult};

fn status_str(s: RoutineRunStatus) -> &'static str {
    match s {
        RoutineRunStatus::Running => "running",
        RoutineRunStatus::Succeeded => "succeeded",
        RoutineRunStatus::Failed => "failed",
    }
}

fn parse_status(s: &str) -> RoutineRunStatus {
    match s {
        "running" => RoutineRunStatus::Running,
        "succeeded" => RoutineRunStatus::Succeeded,
        _ => RoutineRunStatus::Failed,
    }
}

fn routine_from(r: &Row<'_>) -> rusqlite::Result<Routine> {
    let last: Option<String> = r.get("last_status")?;
    Ok(Routine {
        id: r.get("id")?,
        name: r.get("name")?,
        schedule: r.get("schedule")?,
        schedule_text: String::new(),
        timezone: r.get("timezone")?,
        agent_id: r.get("agent_id")?,
        prompt: r.get("prompt")?,
        enabled: r.get::<_, i64>("enabled")? == 1,
        retry: serde_json::from_str::<RetryPolicy>(&r.get::<_, String>("retry_policy")?)
            .unwrap_or_default(),
        notify: serde_json::from_str::<NotifyPolicy>(&r.get::<_, String>("notify")?)
            .unwrap_or_default(),
        next_run_at_ms: r.get("next_run_at")?,
        last_status: last.as_deref().map(parse_status),
        last_run_at_ms: r.get("last_run")?,
        created_at_ms: r.get("created_at")?,
        updated_at_ms: r.get("updated_at")?,
    })
}

fn run_from(r: &Row<'_>) -> rusqlite::Result<RoutineRun> {
    Ok(RoutineRun {
        id: r.get("id")?,
        routine_id: r.get("routine_id")?,
        attempt: r.get("attempt")?,
        status: parse_status(&r.get::<_, String>("status")?),
        error: r.get("error")?,
        conversation_id: r.get("conversation_id")?,
        agent_run_id: r.get("agent_run_id")?,
        scheduled_for_ms: r.get("scheduled_for")?,
        started_at_ms: r.get("started_at")?,
        finished_at_ms: r.get("finished_at")?,
    })
}

const SELECT: &str = "SELECT r.*,
     (SELECT status FROM routine_runs x WHERE x.routine_id = r.id ORDER BY started_at DESC LIMIT 1) AS last_status,
     (SELECT MAX(started_at) FROM routine_runs x WHERE x.routine_id = r.id) AS last_run
     FROM routines r";

impl Database {
    pub fn list_routines(&self) -> DbResult<Vec<Routine>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY r.name COLLATE NOCASE"))?;
        Ok(stmt
            .query_map([], routine_from)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get_routine(&self, id: &str) -> DbResult<Option<Routine>> {
        Ok(self
            .lock()?
            .query_row(&format!("{SELECT} WHERE r.id = ?1"), [id], routine_from)
            .optional()?)
    }

    /// Insert or update; returns the id.
    pub fn save_routine(
        &self,
        p: &RoutineSaveParams,
        next_run_at: Option<i64>,
    ) -> DbResult<String> {
        let id = p.id.clone().unwrap_or_else(wisp_common::ids::new_id);
        let now = wisp_common::time::now_ms();
        let conn = self.lock()?;
        let agent: Option<String> = match &p.agent_id {
            Some(a) => conn
                .query_row("SELECT id FROM agents WHERE id = ?1", [a], |r| r.get(0))
                .optional()?,
            None => None,
        };
        conn.execute(
            "INSERT INTO routines (id, name, schedule, timezone, agent_id, prompt, enabled, retry_policy, notify, next_run_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, schedule = excluded.schedule, timezone = excluded.timezone,
               agent_id = excluded.agent_id, prompt = excluded.prompt, enabled = excluded.enabled,
               retry_policy = excluded.retry_policy, notify = excluded.notify, next_run_at = excluded.next_run_at,
               updated_at = excluded.updated_at",
            params![
                id,
                p.name,
                p.schedule,
                p.timezone,
                agent,
                p.prompt,
                p.enabled as i64,
                serde_json::to_string(&p.retry)?,
                serde_json::to_string(&p.notify)?,
                next_run_at,
                now
            ],
        )?;
        Ok(id)
    }

    pub fn delete_routine(&self, id: &str) -> DbResult<bool> {
        Ok(self
            .lock()?
            .execute("DELETE FROM routines WHERE id = ?1", [id])?
            > 0)
    }

    /// Enabled routines due at or before `now`.
    pub fn due_routines(&self, now: i64) -> DbResult<Vec<Routine>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(&format!(
            "{SELECT} WHERE r.enabled = 1 AND r.next_run_at IS NOT NULL AND r.next_run_at <= ?1 ORDER BY r.next_run_at"
        ))?;
        Ok(stmt
            .query_map([now], routine_from)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn set_next_run(&self, id: &str, next: Option<i64>) -> DbResult<()> {
        self.lock()?.execute(
            "UPDATE routines SET next_run_at = ?2 WHERE id = ?1",
            params![id, next],
        )?;
        Ok(())
    }

    pub fn insert_routine_run(&self, r: &RoutineRun) -> DbResult<()> {
        self.lock()?.execute(
            "INSERT INTO routine_runs (id, routine_id, attempt, status, scheduled_for, started_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![r.id, r.routine_id, r.attempt, status_str(r.status), r.scheduled_for_ms, r.started_at_ms],
        )?;
        Ok(())
    }

    pub fn finish_routine_run(&self, r: &RoutineRun) -> DbResult<()> {
        let conn = self.lock()?;
        let agent_run: Option<String> = match &r.agent_run_id {
            Some(a) => conn
                .query_row("SELECT id FROM agent_runs WHERE id = ?1", [a], |x| x.get(0))
                .optional()?,
            None => None,
        };
        conn.execute(
            "UPDATE routine_runs SET status = ?2, error = ?3, conversation_id = ?4, agent_run_id = ?5, finished_at = ?6 WHERE id = ?1",
            params![r.id, status_str(r.status), r.error, r.conversation_id, agent_run, r.finished_at_ms],
        )?;
        Ok(())
    }

    pub fn list_routine_runs(&self, routine_id: &str, limit: u32) -> DbResult<Vec<RoutineRun>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT * FROM routine_runs WHERE routine_id = ?1 ORDER BY started_at DESC, attempt DESC LIMIT ?2")?;
        Ok(stmt
            .query_map(params![routine_id, limit.min(200)], run_from)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Runs cut off by a quit are marked failed at startup.
    pub fn fail_unfinished_routine_runs(&self) -> DbResult<usize> {
        Ok(self.lock()?.execute(
            "UPDATE routine_runs SET status = 'failed', error = 'Wisp was closed during the run.', finished_at = ?1 WHERE status = 'running'",
            [wisp_common::time::now_ms()],
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routines_due_runs_and_history() {
        let dir = tempfile::tempdir().expect("tmp");
        let db = Database::open(&dir.path().join("t.sqlite3")).expect("open");
        let p = RoutineSaveParams {
            id: None,
            name: "Morning summary".into(),
            schedule: "0 9 * * *".into(),
            timezone: "UTC".into(),
            agent_id: Some("ghost".into()),
            prompt: "Summarize".into(),
            enabled: true,
            retry: RetryPolicy::default(),
            notify: NotifyPolicy::default(),
        };
        let id = db.save_routine(&p, Some(100)).expect("save");
        assert!(db.due_routines(99).expect("due").is_empty());
        let due = db.due_routines(100).expect("due");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].agent_id, None, "dangling agent link dropped");
        let mut run = RoutineRun {
            id: wisp_common::ids::new_id(),
            routine_id: id.clone(),
            attempt: 1,
            status: RoutineRunStatus::Running,
            error: None,
            conversation_id: None,
            agent_run_id: None,
            scheduled_for_ms: 100,
            started_at_ms: 101,
            finished_at_ms: None,
        };
        db.insert_routine_run(&run).expect("run");
        assert_eq!(db.fail_unfinished_routine_runs().expect("recover"), 1);
        run.status = RoutineRunStatus::Succeeded;
        run.finished_at_ms = Some(102);
        db.finish_routine_run(&run).expect("finish");
        let r = db.get_routine(&id).expect("get").expect("routine");
        assert_eq!(
            (r.last_status, r.last_run_at_ms),
            (Some(RoutineRunStatus::Succeeded), Some(101))
        );
        assert_eq!(db.list_routine_runs(&id, 10).expect("runs").len(), 1);
        db.set_next_run(&id, None).expect("next");
        assert!(db.due_routines(i64::MAX).expect("due").is_empty());
        assert!(db.delete_routine(&id).expect("delete"));
    }
}
