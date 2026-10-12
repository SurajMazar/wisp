//! Computer-control tools (spec §55–58): typed calls, risk, approvals, results and audit.
//!
//! A tool call is data until the policy engine allows it. Geometry is global points, top-left.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wisp_common::geometry::Point;

use super::app::Empty;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    /// One step riskier (saturating).
    pub fn raise(self) -> Self {
        match self {
            Self::Safe => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            _ => Self::Critical,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PointerArgs {
    pub x: f64,
    pub y: f64,
    /// What's there, e.g. "Save" — shown in approvals, used to press the element directly and
    /// to judge risk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DragArgs {
    pub from: Point,
    pub to: Point,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScrollArgs {
    pub x: f64,
    pub y: f64,
    /// Lines; positive scrolls down / right. −50…50.
    pub dy: i32,
    #[serde(default)]
    pub dx: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextArgs {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyArgs {
    /// A named key: return, tab, escape, space, delete, forwardDelete, up, down, left, right,
    /// home, end, pageUp, pageDown, f1–f12.
    pub key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum KeyModifier {
    Command,
    Option,
    Control,
    Shift,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HotkeyArgs {
    /// A letter, digit or named key.
    pub key: String,
    pub modifiers: Vec<KeyModifier>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppArgs {
    /// Application name ("Safari") or bundle identifier ("com.apple.Safari").
    pub app: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UrlArgs {
    /// http(s) only.
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindArgs {
    pub label: String,
}

/// A reminder in Reminders. Dates are local time, "YYYY-MM-DD HH:MM".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReminderArgs {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
}

/// An event in Calendar (local time, "YYYY-MM-DD HH:MM").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventArgs {
    pub title: String,
    pub start: String,
    /// Length, 5–1440.
    pub minutes: u32,
}

/// A day, "YYYY-MM-DD".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DayArgs {
    pub day: String,
}

/// A note in Notes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NoteArgs {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MusicCommand {
    Play,
    Pause,
    Next,
    Previous,
}

/// Control music playback (Music, or Spotify when it's the one playing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MusicArgs {
    pub command: MusicCommand,
}

/// A file or folder in the user's home ("~/Desktop/report.pdf").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PathArgs {
    pub path: String,
}

/// Move a file or folder into a folder (both in the user's home).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveArgs {
    pub from: String,
    /// Destination folder (created if missing).
    pub to: String,
}

/// Every action Wisp can take on the computer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "tool", content = "args", rename_all = "camelCase")]
pub enum ToolCall {
    /// Describe the frontmost window's controls (read-only).
    ReadScreen(Empty),
    /// Find a control by its label (read-only).
    FindElement(FindArgs),
    Click(PointerArgs),
    DoubleClick(PointerArgs),
    RightClick(PointerArgs),
    MoveMouse(PointerArgs),
    Drag(DragArgs),
    Scroll(ScrollArgs),
    TypeText(TextArgs),
    Keypress(KeyArgs),
    Hotkey(HotkeyArgs),
    Copy(Empty),
    Paste(Empty),
    OpenApplication(AppArgs),
    SwitchApplication(AppArgs),
    CloseApplication(AppArgs),
    OpenUrl(UrlArgs),
    ReadClipboard(Empty),
    WriteClipboard(TextArgs),
    // Background tools: they act through apps and files directly, never the mouse or keyboard.
    CreateReminder(ReminderArgs),
    ListReminders(Empty),
    CreateEvent(EventArgs),
    ListEvents(DayArgs),
    CreateNote(NoteArgs),
    Music(MusicArgs),
    ListFiles(PathArgs),
    MoveFile(MoveArgs),
    TrashFile(PathArgs),
    /// Read a web page as text.
    FetchPage(UrlArgs),
}

impl ToolCall {
    /// Acts through apps and files directly (no screen, mouse or keyboard).
    pub fn is_background(&self) -> bool {
        matches!(
            self,
            Self::CreateReminder(_)
                | Self::ListReminders(_)
                | Self::CreateEvent(_)
                | Self::ListEvents(_)
                | Self::CreateNote(_)
                | Self::Music(_)
                | Self::ListFiles(_)
                | Self::MoveFile(_)
                | Self::TrashFile(_)
                | Self::FetchPage(_)
        )
    }
}

/// What a tool is and how risky it is by default (for UIs and agents).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDefinition {
    pub name: String,
    pub title: String,
    pub description: String,
    pub risk: RiskLevel,
    pub requires_permission: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolList {
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalDecision {
    AllowOnce,
    AllowSession,
    AlwaysAllow,
    Deny,
}

/// The assistant wants to do something and needs the user's OK (spec §58).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool: String,
    /// e.g. `Click “Save”`.
    pub title: String,
    /// Plain-language detail, e.g. which app and where.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub risk: RiskLevel,
    /// Why it's this risky, when it was raised above the tool's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_reason: Option<String>,
    /// The choices offered (riskier actions offer fewer).
    pub choices: Vec<ApprovalDecision>,
    pub expires_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalResponse {
    pub id: String,
    pub decision: ApprovalDecision,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalResolved {
    pub id: String,
    pub decision: ApprovalDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum VerificationStatus {
    Verified,
    Failed,
    /// Nothing observable either way (spec §62: never assume success).
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Verification {
    pub status: VerificationStatus,
    pub detail: String,
}

/// A background action for the host to perform through an app (Reminders, Calendar, Notes,
/// Music) or the Trash — after the same policy and approval as every tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppsPerformParams {
    pub call: ToolCall,
}

/// An approved action for the host, bound to what the user approved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputPerformParams {
    pub call: ToolCall,
    /// The approval this action was allowed by: the host refuses keyboard actions if the
    /// frontmost app changed since the request was shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    /// For pointer actions: the process that owned the element under the point when risk was
    /// judged; the host refuses if something else is there now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_pid: Option<i32>,
}

/// What the host reports after performing an action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolExecution {
    /// Text result for read-only tools (clipboard, screen description).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    pub verification: Verification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ToolCallStatus {
    WaitingForApproval,
    Running,
    Succeeded,
    Failed,
    Denied,
}

/// One audited tool call (spec §105: no private content — arguments are summarized).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCallRecord {
    pub id: String,
    pub tool: String,
    pub title: String,
    pub risk: RiskLevel,
    /// allowOnce | allowSession | alwaysAllow | deny | policyDeny | autoAllow | granted
    pub decision: String,
    pub status: ToolCallStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<Verification>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    pub created_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolRunParams {
    pub call: ToolCall,
    /// The answer this action came from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
}

/// Result of `tool.run`: the audit record plus any text output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolRunResult {
    pub record: ToolCallRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolActivityParams {
    /// 1–500.
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolActivity {
    pub items: Vec<ToolCallRecord>,
}

/// A standing "always allow" or "never allow" for a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolGrant {
    pub id: String,
    pub tool: String,
    pub decision: ApprovalDecision,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolGrants {
    pub grants: Vec<ToolGrant>,
    /// Tools allowed for the rest of this session.
    pub session: Vec<String>,
}
