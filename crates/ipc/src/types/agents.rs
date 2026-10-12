//! Agents (spec §60–65): saved assistants that carry out multi-step tasks on the computer,
//! observing, acting through the tool pipeline and verifying — within hard limits and always
//! visible.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::models::ModelRef;

/// Hard limits for one run (spec §61).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct AgentLimits {
    /// 1–50.
    pub max_steps: u32,
    /// 1–120.
    pub max_minutes: u32,
    /// 1–100.
    pub max_tool_calls: u32,
    /// Rough prompt tokens across the run; 1 000–500 000.
    pub max_tokens: u32,
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            max_steps: 15,
            max_minutes: 10,
            max_tool_calls: 25,
            max_tokens: 60_000,
        }
    }
}

impl AgentLimits {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=50).contains(&self.max_steps)
            || !(1..=120).contains(&self.max_minutes)
            || !(1..=100).contains(&self.max_tool_calls)
            || !(1_000..=500_000).contains(&self.max_tokens)
        {
            return Err("limits are out of range".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    /// A single emoji.
    pub avatar: String,
    pub instructions: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    /// Tool names this agent may use (see `tool.list`). Empty = read-only.
    pub tools: Vec<String>,
    /// Skill ids attached to this agent.
    #[serde(default)]
    pub skills: Vec<String>,
    pub limits: AgentLimits,
    pub enabled: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSaveParams {
    /// Omit to create.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub avatar: String,
    #[serde(default)]
    pub instructions: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub limits: AgentLimits,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentList {
    pub agents: Vec<AgentDefinition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    Planning,
    Running,
    Paused,
    /// Waiting for the user to approve an action.
    WaitingPermission,
    /// Repeated itself or hit a dead end; Continue or Stop.
    Stuck,
    /// Asked the user a question.
    NeedsInput,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PlanItemStatus {
    Pending,
    Active,
    Done,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanItem {
    pub title: String,
    pub status: PlanItemStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StepKind {
    Observe,
    Plan,
    Act,
    Verify,
    Message,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum RunStepStatus {
    Running,
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunStep {
    pub seq: u32,
    pub kind: StepKind,
    pub title: String,
    pub status: RunStepStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRun {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    pub agent_name: String,
    pub avatar: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    pub goal: String,
    pub status: RunStatus,
    pub plan: Vec<PlanItem>,
    pub steps: Vec<RunStep>,
    /// Final summary, question for the user, or why it stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub steps_used: u32,
    pub tool_calls_used: u32,
    pub tokens_used: u32,
    pub limits: AgentLimits,
    pub started_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRunParams {
    /// Omit for a one-off task with default limits ("Do it" in the composer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    pub goal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    /// Work through apps and files directly (reminders, calendar, notes, music, files, web
    /// pages) without looking at or touching the screen — e.g. "Wisp agent, …" by voice.
    #[serde(default)]
    pub background: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum RunAction {
    Pause,
    Resume,
    /// Keep going after being stuck.
    Continue,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunControl {
    pub run_id: String,
    pub action: RunAction,
    /// Reply to a question the agent asked (with Continue).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRunsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// 1–200.
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRunList {
    pub runs: Vec<AgentRun>,
}
