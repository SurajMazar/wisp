//! Screen focus: telling the assistant exactly what to look at (spec §30–37).
//!
//! Geometry crossing IPC is in global **points**, top-left origin (CoreGraphics convention);
//! `scale` converts a display's points to pixels.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wisp_common::geometry::{Point, Rect};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FocusKind {
    /// The area around a single clicked point.
    Point,
    #[default]
    Rectangle,
    /// Freeform loop drawn around the target.
    Lasso,
    Circle,
    /// The frontmost window under the pointer.
    Window,
    /// The whole display under the pointer.
    Screen,
}

/// Default shape when dragging (spec §81 "Focus Style").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DrawStyle {
    #[default]
    Rectangle,
    Lasso,
    Circle,
}

impl From<DrawStyle> for FocusKind {
    fn from(s: DrawStyle) -> Self {
        match s {
            DrawStyle::Rectangle => FocusKind::Rectangle,
            DrawStyle::Lasso => FocusKind::Lasso,
            DrawStyle::Circle => FocusKind::Circle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct FocusSettings {
    pub draw_style: DrawStyle,
    /// Context kept around a selection, in points (0–200).
    pub margin: u32,
    /// Side of the square captured around a point focus, in points (120–1200).
    pub point_size: u32,
    /// Dim everything outside the selection while drawing.
    pub dim_surroundings: bool,
    /// What Wisp looks at when you ask out loud without drawing.
    pub ask_context: AskContext,
    /// Apps never captured unless you draw on them (bundle identifiers).
    pub excluded_apps: Vec<String>,
    /// When the picture itself is sent to the model, not just the text read from it.
    pub screenshots: ScreenshotMode,
}

/// Pictures make local models much slower (~10 s), so by default they're sent only when needed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ScreenshotMode {
    /// When there's little readable text, or the question is about how something looks.
    #[default]
    Auto,
    Always,
}

/// What a spoken question includes when nothing was drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AskContext {
    /// The window under the pointer.
    #[default]
    Window,
    /// The whole display under the pointer.
    Display,
    /// Nothing: only what you draw.
    Off,
}

/// Password managers: never captured without an explicit drawing.
pub const DEFAULT_EXCLUDED_APPS: [&str; 8] = [
    "com.1password.1password",
    "com.agilebits.onepassword7",
    "com.bitwarden.desktop",
    "com.lastpass.LastPass",
    "com.dashlane.dashlanephonefinal",
    "com.apple.keychainaccess",
    "com.apple.Passwords",
    "com.keepassxc.keepassxc",
];

impl Default for FocusSettings {
    fn default() -> Self {
        Self {
            draw_style: DrawStyle::Rectangle,
            margin: 24,
            point_size: 480,
            dim_surroundings: true,
            ask_context: AskContext::Window,
            excluded_apps: DEFAULT_EXCLUDED_APPS
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            screenshots: ScreenshotMode::Auto,
        }
    }
}

impl FocusSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.margin > 200 {
            return Err("margin must be 0–200".into());
        }
        if !(120..=1200).contains(&self.point_size) {
            return Err("pointSize must be 120–1200".into());
        }
        if self.excluded_apps.len() > 100
            || self
                .excluded_apps
                .iter()
                .any(|a| a.is_empty() || a.len() > 255)
        {
            return Err("excludedApps must be up to 100 bundle identifiers".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DisplayInfo {
    /// `CGDirectDisplayID`.
    pub id: u32,
    pub name: String,
    /// Display frame in global points (top-left origin).
    pub frame: Rect,
    /// Backing scale (2 on Retina).
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplicationInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    pub pid: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowInfo {
    pub id: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub frame: Rect,
}

/// A captured image of the focused area. Kept only in Wisp's cache; never logged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FocusImage {
    /// URL the UI can display (`wisp://focus/<id>.png`).
    pub url: String,
    pub width_px: u32,
    pub height_px: u32,
}

/// What the user pointed the assistant at (spec §34).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FocusRegion {
    pub id: String,
    pub kind: FocusKind,
    pub display: DisplayInfo,
    /// What the user selected, global points.
    pub selection: Rect,
    /// What was captured: the selection plus margin, clipped to the display, global points.
    pub crop: Rect,
    /// Lasso path or clicked point, global points.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<Point>,
    /// The user drew on the screen with the pen; their marks are drawn into the image.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ink: bool,
    pub cursor: Point,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<FocusImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application: Option<ApplicationInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowInfo>,
    /// Relevant UI elements in the region, when Accessibility is allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<super::accessibility::AxContext>,
    /// Text recognized on screen inside the capture (Apple Vision, on device).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<ScreenText>,
    pub created_at_ms: i64,
}

/// On-screen text, read on device. Lets text-only models understand the screen and lets
/// guidance find targets by their label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenText {
    /// Reading order: top to bottom, left to right.
    pub lines: Vec<TextLine>,
    pub elapsed_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextLine {
    pub text: String,
    /// Global points (top-left origin), like every other screen coordinate.
    pub frame: Rect,
    /// 0–1.
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FocusEndReason {
    Cancelled,
    NothingSelected,
    PermissionDenied,
    CaptureFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FocusStarted {
    pub kind: FocusKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FocusEnded {
    pub reason: FocusEndReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FocusStartParams {
    pub kind: FocusKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LatestFocus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<FocusRegion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PermissionState {
    Enabled,
    NeedsAttention,
    NotRequested,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MicrophoneStatus {
    pub state: PermissionState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenPermissionStatus {
    pub screen_recording: PermissionState,
}
