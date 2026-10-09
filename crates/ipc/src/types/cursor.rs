//! The assistant cursor ("buddy") and assistant states (spec §26–29, §81, §92).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::settings::AccentColor;

/// Everything the assistant can be doing, shown on the buddy and in the panel (spec §28, §92).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AssistantState {
    /// Resting beside the pointer.
    #[default]
    Idle,
    /// Summoned and attentive; tracks the pointer closely.
    Following,
    Listening,
    Selecting,
    Drawing,
    Analyzing,
    Thinking,
    Speaking,
    Executing,
    WaitingForPermission,
    Success,
    Error,
}

/// When the buddy is on screen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CursorVisibility {
    /// Always beside the pointer; click it to open the assistant.
    #[default]
    Always,
    /// Only while the assistant is summoned.
    WhenSummoned,
}

/// How lively the buddy's motion is. Reduced Motion in macOS always wins.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AnimationLevel {
    Minimal,
    #[default]
    Normal,
    Expressive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct CursorSettings {
    pub enabled: bool,
    pub visibility: CursorVisibility,
    pub theme: AccentColor,
    /// `#RRGGBB`, used when `theme` is `custom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_color: Option<String>,
    /// Scale, 0.6–1.6.
    pub size: f64,
    /// 0.3–1.0.
    pub opacity: f64,
    /// Glow strength, 0–1.
    pub glow: f64,
    pub animation: AnimationLevel,
    pub show_label: bool,
    pub label: String,
}

impl Default for CursorSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            visibility: CursorVisibility::Always,
            theme: AccentColor::Default,
            custom_color: None,
            size: 1.0,
            opacity: 0.95,
            glow: 0.6,
            animation: AnimationLevel::Normal,
            show_label: false,
            label: "Wisp".into(),
        }
    }
}

impl CursorSettings {
    pub fn validate(&self) -> Result<(), String> {
        let range = |name: &str, v: f64, lo: f64, hi: f64| {
            if v.is_finite() && (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(format!("{name} must be between {lo} and {hi}"))
            }
        };
        range("size", self.size, 0.6, 1.6)?;
        range("opacity", self.opacity, 0.3, 1.0)?;
        range("glow", self.glow, 0.0, 1.0)?;
        if self.label.trim().is_empty() || self.label.chars().count() > 24 {
            return Err("label must be 1–24 characters".into());
        }
        if let Some(c) = self
            .custom_color
            .as_deref()
            .filter(|c| !super::settings::is_hex_color(c))
        {
            return Err(format!("customColor must be #RRGGBB, got {c:?}"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssistantStateChanged {
    pub state: AssistantState,
    /// Whether the assistant panel is expanded from the buddy.
    pub expanded: bool,
}

/// Play the given states on the real on-screen buddy, for previewing themes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CursorPreviewParams {
    pub states: Vec<AssistantState>,
    /// Time per state, 300–5000 ms.
    pub step_ms: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_buddy_always_visible() {
        let c = CursorSettings::default();
        assert!(c.validate().is_ok());
        assert_eq!(c.visibility, CursorVisibility::Always);
    }

    #[test]
    fn rejects_out_of_range_values() {
        for bad in [
            CursorSettings {
                size: 3.0,
                ..Default::default()
            },
            CursorSettings {
                opacity: 0.0,
                ..Default::default()
            },
            CursorSettings {
                glow: f64::NAN,
                ..Default::default()
            },
            CursorSettings {
                label: String::new(),
                ..Default::default()
            },
            CursorSettings {
                custom_color: Some("blue".into()),
                ..Default::default()
            },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
    }
}
