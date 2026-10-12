//! Application settings. Stored one row per section in the `settings` table.
//!
//! Every field has a default so older databases load cleanly when new fields are added.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    cursor::CursorSettings, focus::FocusSettings, invocation::InvocationSettings,
    models::ModelSettings, voice::VoiceSettings,
};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct AppSettings {
    pub general: GeneralSettings,
    pub appearance: AppearanceSettings,
    pub privacy: PrivacySettings,
    pub onboarding: OnboardingSettings,
    pub invocation: InvocationSettings,
    pub cursor: CursorSettings,
    pub focus: FocusSettings,
    pub models: ModelSettings,
    pub voice: VoiceSettings,
    pub computer: ComputerSettings,
    pub memory: super::memory::MemorySettings,
    pub connections: super::connections::ConnectionsSettings,
    pub dictation: super::invocation::DictationSettings,
}

/// Computer control (spec §55–58). Off until the user turns it on.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct ComputerSettings {
    pub enabled: bool,
    /// Ask before every action, even ones allowed for the session or always.
    pub confirm_every_action: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct GeneralSettings {
    pub launch_at_login: bool,
    pub show_menu_bar_icon: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            show_menu_bar_icon: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

/// Accent / assistant colors (spec §29). `Custom` uses `custom_color`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AccentColor {
    #[default]
    Default,
    Blue,
    Purple,
    Cyan,
    Green,
    Orange,
    Red,
    Pink,
    Monochrome,
    Custom,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct AppearanceSettings {
    pub theme: ThemePreference,
    pub accent: AccentColor,
    /// `#RRGGBB`, used when `accent` is `custom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_color: Option<String>,
}

/// Spec §14.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PrivacyMode {
    /// Nothing leaves the Mac. Cloud providers are never eligible, including as fallback.
    #[default]
    LocalOnly,
    /// Local whenever possible; ask before each cloud use.
    LocalPreferred,
    /// Configured cloud providers may be used.
    CloudAllowed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct PrivacySettings {
    pub mode: PrivacyMode,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct OnboardingSettings {
    pub completed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at_ms: Option<i64>,
}

/// One section replaced atomically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "section", content = "value", rename_all = "camelCase")]
pub enum SettingsSection {
    General(GeneralSettings),
    Appearance(AppearanceSettings),
    Privacy(PrivacySettings),
    Onboarding(OnboardingSettings),
    Invocation(InvocationSettings),
    Cursor(CursorSettings),
    Focus(FocusSettings),
    Models(ModelSettings),
    Voice(VoiceSettings),
    Computer(ComputerSettings),
    Memory(super::memory::MemorySettings),
    Connections(super::connections::ConnectionsSettings),
    Dictation(super::invocation::DictationSettings),
}

impl SettingsSection {
    /// Storage key for this section.
    pub fn key(&self) -> &'static str {
        match self {
            Self::General(_) => "general",
            Self::Appearance(_) => "appearance",
            Self::Privacy(_) => "privacy",
            Self::Onboarding(_) => "onboarding",
            Self::Invocation(_) => "invocation",
            Self::Cursor(_) => "cursor",
            Self::Focus(_) => "focus",
            Self::Models(_) => "models",
            Self::Voice(_) => "voice",
            Self::Computer(_) => "computer",
            Self::Memory(_) => "memory",
            Self::Connections(_) => "connections",
            Self::Dictation(_) => "dictation",
        }
    }

    /// Semantic validation beyond what the type system enforces.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Invocation(i) => i.validate(),
            Self::Cursor(c) => c.validate(),
            Self::Focus(f) => f.validate(),
            Self::Models(m) => m.validate(),
            Self::Voice(v) => v.validate(),
            Self::Connections(c) => c.validate(),
            Self::Dictation(d) => d.validate(),
            Self::Appearance(AppearanceSettings {
                custom_color: Some(c),
                ..
            }) if !is_hex_color(c) => Err(format!("customColor must be #RRGGBB, got {c:?}")),
            _ => Ok(()),
        }
    }
}

/// `#RRGGBB`.
pub fn is_hex_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsUpdateParams {
    pub update: SettingsSection,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_privacy_first() {
        let s = AppSettings::default();
        assert_eq!(s.privacy.mode, PrivacyMode::LocalOnly);
        assert!(!s.onboarding.completed);
    }

    #[test]
    fn partial_json_fills_defaults() {
        let s: AppSettings =
            serde_json::from_str(r#"{"appearance":{"theme":"dark"}}"#).expect("parse");
        assert_eq!(s.appearance.theme, ThemePreference::Dark);
        assert!(s.general.show_menu_bar_icon);
    }

    #[test]
    fn validates_custom_color() {
        let bad = SettingsSection::Appearance(AppearanceSettings {
            custom_color: Some("red".into()),
            ..Default::default()
        });
        assert!(bad.validate().is_err());
        let good = SettingsSection::Appearance(AppearanceSettings {
            custom_color: Some("#12abEF".into()),
            ..Default::default()
        });
        assert!(good.validate().is_ok());
    }
}
