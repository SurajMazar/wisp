//! Invocation: how the user summons Wisp (spec §15–18, §82).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Modifier keys, in canonical display order (⌃ ⌥ ⇧ ⌘ fn).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum Modifier {
    Control,
    Option,
    Shift,
    Command,
    Function,
}

/// A global shortcut: modifiers plus an optional key. No key means a modifier-only chord (⌘⌥).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Shortcut {
    pub modifiers: Vec<Modifier>,
    /// macOS virtual key code (`kVK_*`), absent for modifier-only chords.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_code: Option<u16>,
    /// Human-readable key label captured at record time, e.g. "Space", "K".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_label: Option<String>,
}

impl Shortcut {
    pub fn command_option() -> Self {
        Self {
            modifiers: vec![Modifier::Option, Modifier::Command],
            key_code: None,
            key_label: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut sorted = self.modifiers.clone();
        sorted.sort();
        sorted.dedup();
        if sorted.len() != self.modifiers.len() {
            return Err("duplicate modifiers".into());
        }
        match self.key_code {
            None if self.modifiers.len() < 2 => {
                Err("a modifier-only shortcut needs at least two modifiers".into())
            }
            Some(_) if self.modifiers.is_empty() => {
                Err("a shortcut with a key needs at least one modifier".into())
            }
            Some(code) if code > 0x7F => Err(format!("key code {code} is out of range")),
            _ => Ok(()),
        }
    }
}

/// What holding / pressing the shortcut does (spec §18).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum InvocationBehavior {
    /// Active while held; releasing finishes the interaction.
    #[default]
    Hold,
    /// A press turns the assistant on; the next press turns it off.
    Toggle,
    /// Listening while held; releasing sends what you said. Requires voice.
    PushToTalk,
}

/// Action bound to a secondary invocation source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum InvocationAction {
    #[default]
    Disabled,
    /// Same as the keyboard shortcut in toggle mode.
    ToggleAssistant,
    /// Assistant cursor appears ready to point or draw.
    FocusCursor,
    OpenMainWindow,
}

/// Mouse buttons macOS reports to event taps. Primary/secondary buttons are never bindable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MouseButton {
    #[default]
    None,
    Middle,
    Back,
    Forward,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct InvocationSettings {
    pub shortcut: Shortcut,
    pub behavior: InvocationBehavior,
    pub mouse_button: MouseButton,
    pub mouse_action: InvocationAction,
    /// Delay before a held modifier-only chord activates, so ⌘⌥-key app shortcuts don't flash Wisp.
    pub hold_delay_ms: u32,
}

impl Default for InvocationSettings {
    fn default() -> Self {
        Self {
            shortcut: Shortcut::command_option(),
            behavior: InvocationBehavior::Hold,
            mouse_button: MouseButton::None,
            mouse_action: InvocationAction::FocusCursor,
            hold_delay_ms: 150,
        }
    }
}

impl InvocationSettings {
    pub fn validate(&self) -> Result<(), String> {
        self.shortcut.validate()?;
        if !(50..=1000).contains(&self.hold_delay_ms) {
            return Err("holdDelayMs must be between 50 and 1000".into());
        }
        Ok(())
    }
}

/// Hold a shortcut to dictate: what you say is typed where your cursor is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct DictationSettings {
    pub enabled: bool,
    /// A modifier-only chord held while speaking (default ⌃⇧).
    pub shortcut: Shortcut,
    /// Fix punctuation and drop filler words with the language model before typing (slower).
    pub cleanup: bool,
}

impl Default for DictationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            shortcut: Shortcut {
                modifiers: vec![Modifier::Control, Modifier::Shift],
                key_code: None,
                key_label: None,
            },
            cleanup: false,
        }
    }
}

impl DictationSettings {
    pub fn validate(&self) -> Result<(), String> {
        self.shortcut.validate()?;
        if self.shortcut.key_code.is_some() || self.shortcut.modifiers.len() < 2 {
            return Err(
                "dictation uses a chord of two or more modifier keys, held while speaking".into(),
            );
        }
        Ok(())
    }
}

/// Where an invocation came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum InvocationSource {
    Keyboard,
    Mouse,
    MenuBar,
    Dock,
    CommandPalette,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum InvocationEndReason {
    Released,
    /// A quick press-and-release in hold mode (opens or closes the assistant panel).
    Tapped,
    Toggled,
    Dismissed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationStarted {
    pub source: InvocationSource,
    pub behavior: InvocationBehavior,
    pub at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationEnded {
    pub source: InvocationSource,
    pub reason: InvocationEndReason,
    pub duration_ms: i64,
}

/// Whether each invocation channel is actually live right now — never assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ChannelState {
    Active,
    NeedsPermission,
    Conflict,
    Disabled,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationStatus {
    pub keyboard: ChannelState,
    pub mouse: ChannelState,
    pub trackpad: ChannelState,
    pub menu_bar: ChannelState,
    /// Accessibility (or Input Monitoring) granted — required for modifier-only chords and mouse buttons.
    pub event_tap_permitted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConflictSeverity {
    /// The shortcut will not work (taken by macOS or another app).
    Blocking,
    /// It works, but shadows a common shortcut.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortcutConflict {
    pub severity: ConflictSeverity,
    /// What it conflicts with, e.g. "Spotlight", "Copy (most apps)".
    pub with: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortcutCheckParams {
    pub shortcut: Shortcut,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortcutCheckResult {
    pub conflicts: Vec<ShortcutConflict>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortcutRecordParams {
    /// Give up after this long (1–30 s).
    pub timeout_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortcutRecordResult {
    pub shortcut: Shortcut,
    pub conflicts: Vec<ShortcutConflict>,
}

/// System Settings panes Wisp may open for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SettingsPane {
    Accessibility,
    InputMonitoring,
    ScreenRecording,
    Microphone,
    Automation,
    Notifications,
    Keyboard,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenSettingsPaneParams {
    pub pane: SettingsPane,
}

/// Assistant surface requested state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssistantVisibility {
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<InvocationSource>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_command_option_hold() {
        let s = InvocationSettings::default();
        assert_eq!(s.shortcut, Shortcut::command_option());
        assert_eq!(s.behavior, InvocationBehavior::Hold);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn rejects_unusable_shortcuts() {
        let single = Shortcut {
            modifiers: vec![Modifier::Command],
            key_code: None,
            key_label: None,
        };
        assert!(
            single.validate().is_err(),
            "single modifier would fire constantly"
        );
        let bare_key = Shortcut {
            modifiers: vec![],
            key_code: Some(49),
            key_label: Some("Space".into()),
        };
        assert!(bare_key.validate().is_err());
        let dup = Shortcut {
            modifiers: vec![Modifier::Command, Modifier::Command],
            key_code: Some(40),
            key_label: None,
        };
        assert!(dup.validate().is_err());
        let ok = Shortcut {
            modifiers: vec![Modifier::Option],
            key_code: Some(49),
            key_label: Some("Space".into()),
        };
        assert!(ok.validate().is_ok());
    }
}
