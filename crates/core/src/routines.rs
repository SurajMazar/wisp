//! Routines (spec §66): a scheduler that runs prompts or agents on a cron schedule in the user's
//! timezone, with retries, notifications and run history. A missed run (Wisp was closed) runs
//! once at the next start; there's never a backlog.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use wisp_agents::schedule::Schedule;
use wisp_ipc::{
    Envelope, ErrorCode, IpcError,
    methods::RoutineRunUpdated,
    types::{
        agents::{AgentRunParams, RunStatus},
        app::Empty,
        conversations::SendParams,
        routines::{
            NotifyParams, Routine, RoutineList, RoutineRun, RoutineRunStatus, RoutineSaveParams,
        },
    },
};

use crate::Core;

/// How often the scheduler looks for due routines.
const TICK: Duration = Duration::from_secs(15);
/// Longest a prompt routine may take to answer.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10 * 60);

pub struct RoutineClock {
    /// Milliseconds per "minute" of retry backoff (tests shorten it).
    retry_unit_ms: AtomicU64,
}

impl Default for RoutineClock {
    fn default() -> Self {
        Self {
            retry_unit_ms: AtomicU64::new(60_000),
        }
    }
}

fn invalid(m: impl Into<String>) -> IpcError {
    IpcError::new(ErrorCode::InvalidParams, m.into())
}

fn with_text(mut r: Routine) -> Routine {
    r.schedule_text = Schedule::describe(&r.schedule);
    r
}

impl Core {
    /// Tests: make one minute of retry backoff last `ms` milliseconds.
    pub fn set_retry_unit_ms(&self, ms: u64) {
        self.routine_clock()
            .retry_unit_ms
            .store(ms, Ordering::Relaxed);
    }

    pub(crate) async fn routine_list(&self) -> Result<RoutineList, IpcError> {
        let routines = self.blocking(|db| db.list_routines()).await?;
        Ok(RoutineList {
            routines: routines.into_iter().map(with_text).collect(),
        })
    }

    pub(crate) async fn routine_save(&self, mut p: RoutineSaveParams) -> Result<Routine, IpcError> {
        p.name = p.name.trim().to_owned();
        p.prompt = p.prompt.trim().to_owned();
        if p.name.is_empty() || p.name.chars().count() > 60 {
            return Err(invalid("A routine needs a name (up to 60 characters)."));
        }
        if p.prompt.is_empty() || p.prompt.chars().count() > 4000 {
            return Err(invalid("A routine needs a prompt (up to 4000 characters)."));
        }
        if !(1..=5).contains(&p.retry.max_attempts) || !(1..=60).contains(&p.retry.backoff_minutes)
        {
            return Err(invalid("Retries must be 1–5 attempts, 1–60 minutes apart."));
        }
        let schedule = Schedule::parse(&p.schedule, &p.timezone)
            .map_err(|e| invalid(format!("That schedule doesn't work: {e}.")))?;
        if let Some(id) = &p.id
            && !wisp_common::ids::is_valid_id(id)
        {
            return Err(invalid("invalid id"));
        }
        if let Some(a) = &p.agent_id {
            let a = a.clone();
            if self.blocking(move |db| db.get_agent(&a)).await?.is_none() {
                return Err(IpcError::new(
                    ErrorCode::NotFound,
                    "That agent no longer exists.",
                ));
            }
        }
        let next = if p.enabled {
            schedule.next_after(wisp_common::time::now_ms())
        } else {
            None
        };
        let id = self.blocking(move |db| db.save_routine(&p, next)).await?;
        self.blocking(move |db| db.get_routine(&id))
            .await?
            .map(with_text)
            .ok_or_else(|| IpcError::internal("routine vanished"))
    }

    pub(crate) async fn routine_delete(&self, id: String) -> Result<Empty, IpcError> {
        if !wisp_common::ids::is_valid_id(&id) {
            return Err(invalid("invalid id"));
        }
        self.blocking(move |db| db.delete_routine(&id)).await?;
        Ok(Empty {})
    }

    pub(crate) async fn routine_runs(
        &self,
        id: String,
        limit: u32,
    ) -> Result<Vec<RoutineRun>, IpcError> {
        if !wisp_common::ids::is_valid_id(&id) || !(1..=200).contains(&limit) {
            return Err(invalid("invalid id or limit"));
        }
        self.blocking(move |db| db.list_routine_runs(&id, limit))
            .await
    }

    pub(crate) async fn routine_run_now(&self, id: String) -> Result<RoutineRun, IpcError> {
        if !wisp_common::ids::is_valid_id(&id) {
            return Err(invalid("invalid id"));
        }
        let routine = self
            .blocking(move |db| db.get_routine(&id))
            .await?
            .ok_or_else(|| IpcError::new(ErrorCode::NotFound, "That routine no longer exists."))?;
        let first = self
            .start_attempt(&routine, wisp_common::time::now_ms(), 1)
            .await?;
        let core = self.clone();
        let run = first.clone();
        tokio::spawn(async move { core.execute_routine(routine, run).await });
        Ok(first)
    }

    /// Start the background scheduler (once, at startup).
    pub fn start_scheduler(&self) {
        let _ = self.db().fail_unfinished_routine_runs();
        let core = self.clone();
        tokio::spawn(async move {
            loop {
                core.tick(wisp_common::time::now_ms()).await;
                tokio::time::sleep(TICK).await;
            }
        });
    }

    /// Fire every routine due at `now`. Returns how many started.
    pub async fn tick(&self, now: i64) -> usize {
        let Ok(due) = self.blocking(move |db| db.due_routines(now)).await else {
            return 0;
        };
        let mut started = 0;
        for r in due {
            let scheduled_for = r.next_run_at_ms.unwrap_or(now);
            // Schedule the next run from now first, so a slow or missed run never piles up.
            let next = Schedule::parse(&r.schedule, &r.timezone)
                .ok()
                .and_then(|s| s.next_after(now));
            let id = r.id.clone();
            if self
                .blocking(move |db| db.set_next_run(&id, next))
                .await
                .is_err()
            {
                continue;
            }
            match self.start_attempt(&r, scheduled_for, 1).await {
                Ok(run) => {
                    started += 1;
                    let core = self.clone();
                    tokio::spawn(async move { core.execute_routine(r, run).await });
                }
                Err(e) => tracing::warn!(error = %e.message, "routine couldn't start"),
            }
        }
        started
    }

    async fn start_attempt(
        &self,
        r: &Routine,
        scheduled_for: i64,
        attempt: u32,
    ) -> Result<RoutineRun, IpcError> {
        let run = RoutineRun {
            id: wisp_common::ids::new_id(),
            routine_id: r.id.clone(),
            attempt,
            status: RoutineRunStatus::Running,
            error: None,
            conversation_id: None,
            agent_run_id: None,
            scheduled_for_ms: scheduled_for,
            started_at_ms: wisp_common::time::now_ms(),
            finished_at_ms: None,
        };
        let stored = run.clone();
        self.blocking(move |db| db.insert_routine_run(&stored))
            .await?;
        self.events().publish::<RoutineRunUpdated>(&run);
        Ok(run)
    }

    async fn execute_routine(&self, r: Routine, mut run: RoutineRun) {
        loop {
            let outcome = match &r.agent_id {
                Some(agent) => self.routine_agent(agent, &r.prompt, &mut run).await,
                None => self.routine_prompt(&r, &mut run).await,
            };
            run.finished_at_ms = Some(wisp_common::time::now_ms());
            match outcome {
                Ok(summary) => {
                    run.status = RoutineRunStatus::Succeeded;
                    self.save_routine_run(&run).await;
                    tracing::info!(routine = %r.id, attempt = run.attempt, "routine succeeded");
                    if r.notify.on_success {
                        self.notify(&r.name, &summary).await;
                    }
                    return;
                }
                Err(e) => {
                    run.status = RoutineRunStatus::Failed;
                    run.error = Some(e.clone());
                    self.save_routine_run(&run).await;
                    tracing::warn!(routine = %r.id, attempt = run.attempt, error = %e, "routine failed");
                    if run.attempt >= r.retry.max_attempts {
                        if r.notify.on_failure {
                            self.notify(&r.name, &format!("Didn't work: {e}")).await;
                        }
                        return;
                    }
                    let unit = self.routine_clock().retry_unit_ms.load(Ordering::Relaxed);
                    tokio::time::sleep(Duration::from_millis(
                        unit * u64::from(r.retry.backoff_minutes),
                    ))
                    .await;
                    match self
                        .start_attempt(&r, run.scheduled_for_ms, run.attempt + 1)
                        .await
                    {
                        Ok(next) => run = next,
                        Err(_) => return,
                    }
                }
            }
        }
    }

    async fn save_routine_run(&self, run: &RoutineRun) {
        let stored = run.clone();
        let _ = self
            .blocking(move |db| db.finish_routine_run(&stored))
            .await;
        self.events().publish::<RoutineRunUpdated>(run);
    }

    async fn notify(&self, title: &str, body: &str) {
        if let Some(host) = self.ai().host() {
            let params = NotifyParams {
                title: title.chars().take(80).collect(),
                body: body.chars().take(240).collect(),
            };
            let _ = host
                .call(
                    "host.notify",
                    serde_json::to_value(params).unwrap_or_default(),
                )
                .await;
        }
    }

    /// Answer the prompt in a fresh "Routine: …" conversation; returns the answer.
    async fn routine_prompt(&self, r: &Routine, run: &mut RoutineRun) -> Result<String, String> {
        let mut rx = self.events().subscribe();
        let sent = self
            .send(SendParams {
                conversation_id: None,
                text: r.prompt.clone(),
                include_focus: false,
                allow_cloud: false,
                model: None,
                teach: false,
                act: false,
            })
            .await
            .map_err(|e| e.message)?;
        run.conversation_id = Some(sent.conversation_id.clone());
        let title = format!("Routine: {}", r.name);
        let (cid, t) = (
            sent.conversation_id.clone(),
            title.chars().take(80).collect::<String>(),
        );
        let _ = self
            .blocking(move |db| db.rename_conversation(&cid, &t))
            .await;
        let id = sent.assistant_message.id.clone();
        let wait = async {
            loop {
                match rx.recv().await {
                    Ok(Envelope::Event { event, payload, .. })
                        if event == "conversation.messageUpdated"
                            && payload["id"] == id.as_str()
                            && payload["status"] != "streaming" =>
                    {
                        return payload;
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => return serde_json::Value::Null,
                }
            }
        };
        let done = tokio::time::timeout(ANSWER_TIMEOUT, wait)
            .await
            .map_err(|_| "The answer took too long.".to_owned())?;
        if done["status"] == "complete" {
            Ok(done["content"].as_str().unwrap_or_default().to_owned())
        } else {
            Err(done["error"]
                .as_str()
                .unwrap_or("The model couldn't answer.")
                .to_owned())
        }
    }

    /// Run the agent with the prompt as its goal; returns its summary.
    async fn routine_agent(
        &self,
        agent: &str,
        goal: &str,
        run: &mut RoutineRun,
    ) -> Result<String, String> {
        let mut rx = self.events().subscribe();
        let started = self
            .agent_start(
                AgentRunParams {
                    agent_id: Some(agent.to_owned()),
                    goal: goal.to_owned(),
                    conversation_id: None,
                },
                None,
            )
            .await
            .map_err(|e| e.message)?;
        run.agent_run_id = Some(started.id.clone());
        let limit = Duration::from_secs(u64::from(started.limits.max_minutes) * 60 + 60);
        let id = started.id.clone();
        let wait = async {
            loop {
                if let Ok(Envelope::Event { event, payload, .. }) = rx.recv().await
                    && event == "agent.runUpdated"
                    && payload["id"] == id.as_str()
                    && let Ok(status) =
                        serde_json::from_value::<RunStatus>(payload["status"].clone())
                    && matches!(
                        status,
                        RunStatus::Completed
                            | RunStatus::Failed
                            | RunStatus::Cancelled
                            | RunStatus::Stuck
                    )
                {
                    return (
                        status,
                        payload["note"].as_str().unwrap_or_default().to_owned(),
                    );
                }
            }
        };
        let (status, note) = tokio::time::timeout(limit, wait)
            .await
            .map_err(|_| "The agent ran out of time.".to_owned())?;
        if status == RunStatus::Completed {
            Ok(note)
        } else {
            Err(if note.is_empty() {
                format!("{status:?}")
            } else {
                note
            })
        }
    }
}
