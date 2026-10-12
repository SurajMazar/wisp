//! Visual guidance (spec §54): what Wisp draws on screen to show the user where things are.
//!
//! Geometry is in global points, top-left origin, like focus regions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wisp_common::geometry::{Point, Rect};

/// Something on screen an answer pointed at, resolved to where it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuidanceTarget {
    /// What the model called it (e.g. "Save").
    pub label: String,
    pub rect: Rect,
    pub source: TargetSource,
    /// How to mark it on screen.
    #[serde(default)]
    pub mark: GuidanceMark,
}

/// How the model asked for a target to be marked: `[[point: …]]`, `[[circle: …]]`, …
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum GuidanceMark {
    /// Outlined and tinted.
    #[default]
    Point,
    Circle,
    /// An arrow pointing at it.
    Arrow,
    /// A line under it (for text).
    Underline,
    /// Everything else dimmed.
    Spotlight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TargetSource {
    /// Matched a UI element from Accessibility (exact frame).
    Element,
    /// Matched text read on screen.
    Text,
    /// Coordinates given by a vision model.
    Coordinates,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RectAnnotation {
    pub rect: Rect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CircleAnnotation {
    pub center: Point,
    pub radius: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArrowAnnotation {
    pub from: Point,
    pub to: Point,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabelAnnotation {
    pub at: Point,
    pub text: String,
}

/// One drawing on the guidance overlay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum Annotation {
    Arrow(ArrowAnnotation),
    Circle(CircleAnnotation),
    Rectangle(RectAnnotation),
    /// Tinted box with a glowing edge.
    Highlight(RectAnnotation),
    /// Dims everything except the rectangle.
    Spotlight(RectAnnotation),
    Label(LabelAnnotation),
    /// A pulsing cursor indicator at a point.
    Pointer(LabelAnnotation),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuidanceShow {
    pub id: String,
    pub annotations: Vec<Annotation>,
    /// Where the buddy flies to point (it returns to the pointer afterwards).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buddy_target: Option<Point>,
    /// Auto-dismiss after this long; Esc, a click or the next invocation dismiss sooner.
    pub duration_ms: u32,
}
