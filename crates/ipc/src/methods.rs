//! Method and event registry.
//!
//! Every IPC method is a marker type implementing [`Method`], declared once in
//! [`define_methods!`]. The same table drives Rust dispatch, JSON Schema generation and
//! therefore the generated Zod/Swift clients — adding a method here is the only way to
//! make it callable.

use schemars::JsonSchema;
use serde::{Serialize, de::DeserializeOwned};

use crate::types::{
    accessibility::*, agents::*, app::*, connections::*, conversations::*, cursor::*, files::*,
    focus::*, guidance::*, invocation::*, memory::*, models::*, release::*, routines::*,
    settings::*, skills::*, teacher::*, tools::*, voice::*,
};

/// Which side implements a method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    /// Implemented by `wisp-core`; callable by the host and the UI.
    Core,
    /// Implemented by the Swift host; callable by `wisp-core` and the UI.
    Host,
}

pub trait Method {
    const NAME: &'static str;
    const TARGET: Target;
    type Params: Serialize + DeserializeOwned + JsonSchema;
    type Result: Serialize + DeserializeOwned + JsonSchema;
}

pub trait EventKind {
    const NAME: &'static str;
    type Payload: Serialize + DeserializeOwned + JsonSchema;
}

/// Description of a method used by the schema generator.
pub struct MethodSpec {
    pub name: &'static str,
    pub target: Target,
    pub params: fn(&mut schemars::SchemaGenerator) -> schemars::Schema,
    pub result: fn(&mut schemars::SchemaGenerator) -> schemars::Schema,
}

pub struct EventSpec {
    pub name: &'static str,
    pub payload: fn(&mut schemars::SchemaGenerator) -> schemars::Schema,
}

macro_rules! define_methods {
    ($( $(#[$meta:meta])* $ty:ident = $name:literal, $target:ident, $params:ty => $result:ty; )*) => {
        $(
            $(#[$meta])*
            pub struct $ty;
            impl Method for $ty {
                const NAME: &'static str = $name;
                const TARGET: Target = Target::$target;
                type Params = $params;
                type Result = $result;
            }
        )*

        pub fn method_specs() -> Vec<MethodSpec> {
            vec![$(MethodSpec {
                name: $name,
                target: Target::$target,
                params: |g| g.subschema_for::<$params>(),
                result: |g| g.subschema_for::<$result>(),
            }),*]
        }
    };
}

macro_rules! define_events {
    ($( $(#[$meta:meta])* $ty:ident = $name:literal, $payload:ty; )*) => {
        $(
            $(#[$meta])*
            pub struct $ty;
            impl EventKind for $ty {
                const NAME: &'static str = $name;
                type Payload = $payload;
            }
        )*

        pub fn event_specs() -> Vec<EventSpec> {
            vec![$(EventSpec { name: $name, payload: |g| g.subschema_for::<$payload>() }),*]
        }
    };
}

define_methods! {
    /// Version and runtime information about the core.
    AppInfoMethod = "app.info", Core, Empty => AppInfo;
    /// Read all settings (defaults filled in).
    SettingsGet = "settings.get", Core, Empty => AppSettings;
    /// Replace one settings section and persist it.
    SettingsUpdate = "settings.update", Core, SettingsUpdateParams => AppSettings;

    /// Live state of each invocation channel (keyboard, mouse, trackpad, menu bar).
    HostInvocationStatus = "host.invocation.status", Host, Empty => InvocationStatus;
    /// Capture the next shortcut the user presses (Escape cancels).
    HostShortcutRecord = "host.shortcut.record", Host, ShortcutRecordParams => ShortcutRecordResult;
    /// Check a shortcut against macOS, common app shortcuts and Wisp's own bindings.
    HostShortcutCheck = "host.shortcut.check", Host, ShortcutCheckParams => ShortcutCheckResult;
    /// Open a System Settings privacy pane.
    HostOpenSettingsPane = "host.system.openSettingsPane", Host, OpenSettingsPaneParams => Empty;
    /// Quit and reopen Wisp (macOS applies some permissions only after a restart).
    HostAppRelaunch = "host.app.relaunch", Host, Empty => Empty;
    /// Show or hide the floating assistant.
    HostAssistantSetVisible = "host.assistant.setVisible", Host, AssistantVisibility => Empty;
    /// Current assistant state and whether the panel is expanded.
    HostAssistantState = "host.assistant.state", Host, Empty => AssistantStateChanged;
    /// Play states on the real buddy (theme preview in Settings).
    HostCursorPreview = "host.cursor.preview", Host, CursorPreviewParams => Empty;
    /// Begin a focus selection (draw, point, window or screen).
    HostFocusStart = "host.focus.start", Host, FocusStartParams => Empty;
    /// Cancel an in-progress selection and remove the overlay.
    HostFocusCancel = "host.focus.cancel", Host, Empty => Empty;
    /// Forget the current focus region (and delete its image).
    HostFocusClear = "host.focus.clear", Host, Empty => Empty;
    /// Screen Recording permission as macOS reports it.
    HostScreenPermission = "host.screen.permission", Host, Empty => ScreenPermissionStatus;
    /// Ask macOS for Screen Recording permission (shows the system prompt once).
    HostScreenRequestPermission = "host.screen.requestPermission", Host, Empty => ScreenPermissionStatus;

    /// Accessibility permission as macOS reports it.
    HostAxStatus = "host.ax.status", Host, Empty => AccessibilityStatus;
    /// Show the system Accessibility prompt (once) and open the settings pane.
    HostAxRequestPermission = "host.ax.requestPermission", Host, Empty => AccessibilityStatus;
    /// Read the element under a point and the elements in a region (bounded).
    HostAxSnapshot = "host.ax.snapshot", Host, AxSnapshotParams => AxSnapshot;
    /// Microphone permission as macOS reports it.
    HostMicPermission = "host.mic.permission", Host, Empty => MicrophoneStatus;
    /// Ask for microphone access (system prompt the first time).
    HostMicRequestPermission = "host.mic.requestPermission", Host, Empty => MicrophoneStatus;

    /// Apple Foundation Models availability on this Mac.
    HostAppleStatus = "host.apple.status", Host, Empty => AppleStatus;
    /// Generate with Apple Foundation Models; streams `apple.delta` events to the core.
    HostAppleGenerate = "host.apple.generate", Host, AppleGenerateParams => AppleGenerateResult;
    HostAppleCancel = "host.apple.cancel", Host, RequestIdParams => Empty;
    /// Store a secret in the macOS Keychain (UI → host). Never passes through the core's storage.
    HostKeychainSet = "host.keychain.set", Host, KeychainSetParams => Empty;
    HostKeychainDelete = "host.keychain.delete", Host, KeychainAccount => Empty;
    /// Read a secret for an outgoing request (core → host only).
    HostKeychainGet = "host.keychain.get", Host, KeychainAccount => KeychainSecret;
    /// Start a local Ollama installation that isn't running.
    HostOllamaStart = "host.ollama.start", Host, Empty => Empty;

    /// Providers with live status and every model they offer.
    ModelsListMethod = "models.list", Core, Empty => ModelsList;
    /// Download an Ollama model; progress arrives as `models.pullProgress`.
    ModelsPull = "models.pull", Core, ModelNameParams => Empty;
    ModelsCancelPull = "models.cancelPull", Core, ModelNameParams => Empty;
    ModelsDelete = "models.delete", Core, ModelNameParams => Empty;
    /// Ask a model a quick question and time it.
    ModelsTest = "models.test", Core, ModelTestParams => ModelTestResult;
    /// Which model would answer, and why (respects privacy mode).
    AiRoute = "ai.route", Core, RouteParams => RouteInfo;

    /// Recent conversations, newest first; `query` searches titles and messages.
    ConversationListMethod = "conversation.list", Core, ConversationListParams => ConversationList;
    ConversationGet = "conversation.get", Core, IdParams => Conversation;
    /// Ask a question. Returns immediately; the answer streams as `conversation.delta`.
    ConversationSend = "conversation.send", Core, SendParams => SendResult;
    /// Stop an answer that is still streaming (by assistant message id).
    ConversationCancel = "conversation.cancel", Core, IdParams => Empty;
    ConversationRename = "conversation.rename", Core, RenameParams => ConversationSummary;
    ConversationPin = "conversation.pin", Core, PinParams => ConversationSummary;
    ConversationDelete = "conversation.delete", Core, IdParams => Empty;
    /// Answer the same question again, replacing an assistant message.
    ConversationRegenerate = "conversation.regenerate", Core, RegenerateParams => Message;
    /// Database backups (automatic daily; the last 7 are kept).
    BackupListMethod = "backup.list", Core, Empty => BackupList;
    /// Make a backup now.
    BackupCreate = "backup.create", Core, Empty => BackupInfo;
    /// Export conversations, memories, skills, agents, routines and settings as JSON.
    DataExport = "data.export", Core, Empty => ExportResult;
    /// Look for a newer release (only when asked; nothing is installed automatically).
    UpdateCheck = "update.check", Core, Empty => UpdateInfo;
    /// Scheduled routines.
    RoutineListMethod = "routine.list", Core, Empty => RoutineList;
    /// Create or update a routine (schedule and timezone are validated).
    RoutineSave = "routine.save", Core, RoutineSaveParams => Routine;
    RoutineDelete = "routine.delete", Core, IdParams => Empty;
    /// Run a routine now (outside its schedule).
    RoutineRunNow = "routine.runNow", Core, IdParams => RoutineRun;
    /// A routine's run history, newest first.
    RoutineRuns = "routine.runs", Core, RoutineRunsParams => RoutineRunList;
    /// Imported files, newest first.
    FileListMethod = "file.list", Core, Empty => FileList;
    /// Pick files with the system picker and index them.
    FileImport = "file.import", Core, Empty => FileList;
    /// Import a dropped file's bytes (≤ 20 MB) and index it.
    FileImportDataMethod = "file.importData", Core, FileImportData => FileItem;
    FileDelete = "file.delete", Core, IdParams => Empty;
    /// Search indexed files.
    FileSearch = "file.search", Core, FileSearchParams => FileSearchResults;
    /// Open a file in its default app.
    FileOpen = "file.open", Core, IdParams => Empty;
    /// Saved memories, newest first.
    MemoryListMethod = "memory.list", Core, MemoryListParams => MemoryList;
    /// Connected services (MCP servers): state and tools of each.
    McpStatus = "mcp.status", Core, Empty => McpStatusList;
    /// Reconnect one server (after changing its settings or signing in).
    McpReconnect = "mcp.reconnect", Core, McpServerId => McpServerStatus;
    /// Add or edit a memory.
    MemorySave = "memory.save", Core, MemorySaveParams => MemoryItem;
    MemoryDelete = "memory.delete", Core, IdParams => Empty;
    /// Forget everything.
    MemoryClear = "memory.clear", Core, Empty => MemoryCleared;
    /// Installed skills.
    SkillListMethod = "skill.list", Core, Empty => SkillList;
    SkillGet = "skill.get", Core, IdParams => Skill;
    /// Create or edit a skill from its SKILL.md (each edit is a new version).
    SkillSave = "skill.save", Core, SkillSaveParams => Skill;
    SkillSetEnabled = "skill.setEnabled", Core, SkillEnableParams => SkillSummary;
    SkillDelete = "skill.delete", Core, IdParams => Empty;
    /// Import a SKILL.md. Imported skills start disabled and are never executed.
    SkillImport = "skill.import", Core, SkillImportParams => Skill;
    SkillExportMethod = "skill.export", Core, IdParams => SkillExport;
    SkillVersionsMethod = "skill.versions", Core, IdParams => SkillVersions;
    /// Make an earlier version current again (as a new version).
    SkillRestore = "skill.restore", Core, SkillRestoreParams => Skill;
    /// Saved agents.
    AgentListMethod = "agent.list", Core, Empty => AgentList;
    /// Create or update an agent.
    AgentSave = "agent.save", Core, AgentSaveParams => AgentDefinition;
    AgentDelete = "agent.delete", Core, IdParams => Empty;
    /// Start a run (returns at once; follow `agent.runUpdated`).
    AgentRunMethod = "agent.run", Core, AgentRunParams => AgentRun;
    /// Pause, resume, continue or stop a run.
    AgentControl = "agent.control", Core, RunControl => Empty;
    /// Recent runs, newest first.
    AgentRuns = "agent.runs", Core, AgentRunsParams => AgentRunList;
    /// Control the running lesson.
    TeachControl = "teach.control", Core, LessonControl => Empty;
    /// The running (or last) lesson, if any.
    TeachCurrent = "teach.current", Core, Empty => CurrentLesson;
    /// Computer-control tools and their default risk.
    ToolListMethod = "tool.list", Core, Empty => ToolList;
    /// Run a tool through validation, risk analysis, policy, approval, execution and verification.
    ToolRun = "tool.run", Core, ToolRunParams => ToolRunResult;
    /// Answer a pending approval request.
    ToolRespond = "tool.respond", Core, ApprovalResponse => Empty;
    /// Recent tool calls (audit log).
    ToolActivityMethod = "tool.activity", Core, ToolActivityParams => ToolActivity;
    /// Standing and session permissions.
    ToolGrantsMethod = "tool.grants", Core, Empty => ToolGrants;
    /// Remove a standing permission (or clear a session one by tool name).
    ToolRevoke = "tool.revoke", Core, IdParams => ToolGrants;
    /// Show an answer's guidance on screen again.
    ConversationShowGuidance = "conversation.showGuidance", Core, IdParams => Empty;
    /// Put text on the clipboard (Copy buttons).
    HostClipboardWrite = "host.clipboard.writeText", Host, ClipboardText => Empty;
    /// Draw guidance on screen and fly the buddy to the target (spec §54).
    HostGuidanceShow = "host.guidance.show", Host, GuidanceShow => Empty;
    /// Remove any guidance from the screen.
    HostGuidanceClear = "host.guidance.clear", Host, Empty => Empty;
    /// Post a macOS notification.
    HostNotify = "host.notify", Host, NotifyParams => Empty;
    /// Let the user pick files; the host copies them under `files/` in the data folder.
    HostFilesPick = "host.files.pick", Host, Empty => StagedFiles;
    /// Read text from a PDF, document or image under `files/` (PDFKit, document readers, Vision).
    HostFilesExtract = "host.files.extract", Host, FilePathParams => ExtractedText;
    /// Open a file under `files/` with its default app.
    HostFilesOpen = "host.files.open", Host, FilePathParams => Empty;
    /// On-device sentence embeddings (Apple NaturalLanguage) for semantic search.
    HostEmbed = "host.embed", Host, EmbedParams => Embeddings;
    /// Capture and read the frontmost window (not Wisp's), for lessons. Needs Screen Recording.
    HostScreenObserve = "host.screen.observe", Host, Empty => FocusRegion;
    /// Perform an approved action and observe whether it worked (needs Accessibility).
    HostInputPerform = "host.input.perform", Host, InputPerformParams => ToolExecution;

    /// Start listening (push-to-talk from the panel or a microphone test).
    HostVoiceStart = "host.voice.start", Host, HostVoiceStartParams => Empty;
    /// Stop listening and process what was said.
    HostVoiceStop = "host.voice.stop", Host, Empty => VoiceFinishResult;
    /// Stop speaking immediately.
    HostVoiceStopSpeaking = "host.voice.stopSpeaking", Host, Empty => Empty;
    /// Perform a background action through an app (Reminders, Calendar, Notes, Music) or the
    /// Trash. Core only; the call has already passed policy and approval.
    HostAppsPerform = "host.apps.perform", Host, AppsPerformParams => ToolExecution;
    /// The voices built into macOS.
    HostSpeechVoices = "host.speech.voices", Host, Empty => SystemVoices;
    /// Speak text with a macOS voice into a WAV file (core only; confined to `cache/tts`).
    HostSpeechSynthesize = "host.speech.synthesize", Host, SpeechSynthesizeParams => Empty;

    /// Speech model status and readiness of recognition and voices.
    VoiceStatusMethod = "voice.status", Core, Empty => VoiceStatus;
    /// Transcribe a finished recording and (unless testing) ask it as a question.
    VoiceFinish = "voice.finish", Core, VoiceFinishParams => VoiceFinishResult;
    VoiceCancel = "voice.cancel", Core, SessionParams => Empty;
    /// Speak text with the configured voice (Test Voice).
    VoiceSpeak = "voice.speak", Core, SpeakParams => Empty;
    VoiceStopSpeaking = "voice.stopSpeaking", Core, Empty => Empty;
    /// Download a speech model listed in the manifest (verified by SHA-256).
    VoiceModelDownload = "voice.models.download", Core, ModelIdParams => Empty;
    VoiceModelCancel = "voice.models.cancel", Core, ModelIdParams => Empty;
    VoiceModelRemove = "voice.models.remove", Core, ModelIdParams => Empty;

    /// Rank and trim an accessibility snapshot into model-ready context.
    AccessibilityRank = "accessibility.rank", Core, AxRankParams => AxContext;
    /// The most recent focus region the core accepted.
    FocusLatest = "focus.latest", Core, Empty => LatestFocus;
}

define_events! {
    /// Emitted once after migrations have run and the core accepts requests.
    CoreReady = "core.ready", AppInfo;
    /// Emitted after any settings section changes.
    SettingsChanged = "settings.changed", AppSettings;
    /// A connected service changed state (connecting, ready, error).
    McpStatusChanged = "mcp.statusChanged", McpServerStatus;
    /// The user summoned the assistant.
    InvocationStartedEvent = "invocation.started", InvocationStarted;
    /// The invocation finished (released, toggled off, dismissed, cancelled).
    InvocationEndedEvent = "invocation.ended", InvocationEnded;
    /// An invocation channel changed state (e.g. permission granted).
    InvocationStatusChanged = "invocation.statusChanged", InvocationStatus;
    /// The assistant changed state or the panel expanded/collapsed.
    AssistantStateChangedEvent = "assistant.stateChanged", AssistantStateChanged;
    /// A focus selection began.
    FocusStartedEvent = "focus.started", FocusStarted;
    /// A focus region was captured and validated.
    FocusCompleted = "focus.completed", FocusRegion;
    /// A selection ended without a region (cancelled, nothing selected, permission, failure).
    FocusEndedEvent = "focus.ended", FocusEnded;
    /// The current focus region was cleared.
    FocusCleared = "focus.cleared", Empty;
    /// Model download progress.
    ModelsPullProgress = "models.pullProgress", PullProgress;
    /// Streaming text from Apple Foundation Models (host → core).
    AppleDeltaEvent = "apple.delta", AppleDelta;
    /// Streaming text of an assistant message.
    ConversationDeltaEvent = "conversation.delta", MessageDelta;
    /// Streamed reasoning text for an assistant message.
    ReasoningDelta = "conversation.reasoningDelta", MessageDelta;
    /// A message was created or changed status (complete, cancelled, error).
    MessageUpdated = "conversation.messageUpdated", Message;
    /// A conversation's title, pin or recency changed.
    ConversationUpdated = "conversation.updated", ConversationSummary;
    /// A message was removed (e.g. replaced by a regenerated answer).
    MessageRemoved = "conversation.messageRemoved", Message;
    /// The assistant needs permission for an action.
    ToolApprovalRequested = "tool.approvalRequested", ApprovalRequest;
    /// An approval was answered (here or elsewhere) or expired.
    ToolApprovalResolved = "tool.approvalResolved", ApprovalResolved;
    /// A tool call changed state (activity log).
    ToolCallUpdated = "tool.callUpdated", ToolCallRecord;
    /// A lesson changed (new step, waiting, done…).
    TeachUpdated = "teach.updated", Lesson;
    /// An agent run changed (plan, step, status).
    AgentRunUpdated = "agent.runUpdated", AgentRun;
    /// A file's indexing status changed.
    FileUpdated = "file.updated", FileItem;
    /// A routine run started or finished.
    RoutineRunUpdated = "routine.runUpdated", RoutineRun;
    /// Captured microphone audio (host → core only).
    AudioChunkEvent = "audio.chunk", AudioChunk;
    /// Synthesized speech ready to play (core → host).
    SpeechChunkEvent = "voice.speechChunk", SpeechChunk;
    /// Speech model download progress.
    SpeechModelProgressEvent = "voice.modelProgress", SpeechModelProgress;
    /// Microphone level while listening (host → UI).
    VoiceLevelEvent = "voice.level", VoiceLevel;
    /// A spoken question was asked (host → UI).
    VoiceAskedEvent = "voice.asked", VoiceAsked;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn method_and_event_names_are_unique_and_namespaced() {
        let mut seen = HashSet::new();
        for m in method_specs() {
            assert!(m.name.contains('.'), "{} must be namespaced", m.name);
            assert!(seen.insert(m.name), "duplicate method {}", m.name);
            assert_eq!(
                m.name.starts_with("host."),
                m.target == Target::Host,
                "{}",
                m.name
            );
        }
        let mut seen = HashSet::new();
        for e in event_specs() {
            assert!(seen.insert(e.name), "duplicate event {}", e.name);
        }
    }
}
