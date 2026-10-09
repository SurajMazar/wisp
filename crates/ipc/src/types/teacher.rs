//! Teacher mode (spec §53): a guided lesson, one step at a time. The user stays in control —
//! Wisp observes, highlights, explains and waits; it never acts unless asked through the tool
//! pipeline.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::guidance::GuidanceTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LessonStatus {
    /// Looking at the screen and working out the next step.
    Thinking,
    /// Waiting for the user to do the current step.
    WaitingForUser,
    Done,
    /// It couldn't work out how to continue; the user can say more or stop.
    Stuck,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StepStatus {
    Current,
    Done,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LessonStep {
    pub instruction: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<GuidanceTarget>,
    pub status: StepStatus,
    /// How the step was confirmed, e.g. "The window changed".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed: Option<String>,
    /// The assistant message that holds this step.
    pub message_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Lesson {
    pub id: String,
    pub conversation_id: String,
    pub goal: String,
    pub status: LessonStatus,
    pub steps: Vec<LessonStep>,
    /// Whether Wisp can notice on its own that a step was done (needs Accessibility).
    pub watching: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub started_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LessonAction {
    /// "I did it" — look again and continue.
    Next,
    /// Highlight and say the current step again.
    Repeat,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LessonControl {
    pub lesson_id: String,
    pub action: LessonAction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentLesson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lesson: Option<Lesson>,
}
