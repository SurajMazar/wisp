//! Accessibility (AX) context: the real UI elements under the pointer and in a focus region
//! (spec §35, §38). Raw trees never leave the host whole; the core ranks and trims them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wisp_common::geometry::{Point, Rect};

use super::focus::{ApplicationInfo, PermissionState, WindowInfo};

/// One normalized accessibility element. Secure text field values are never populated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AxNode {
    /// AXRole without the "AX" prefix, e.g. "Button", "TextField".
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subrole: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    /// Global points, top-left origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Rect>,
    pub enabled: bool,
    pub focused: bool,
    pub selected: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    /// Depth below the window (0 = window).
    pub depth: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AxSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application: Option<ApplicationInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under_pointer: Option<AxNode>,
    /// Parents of `under_pointer`, nearest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ancestors: Vec<AxNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused: Option<AxNode>,
    /// Elements inside the requested region (bounded traversal).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<AxNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_text: Option<String>,
    /// Traversal hit its node or time budget.
    pub truncated: bool,
    pub elapsed_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AxSnapshotParams {
    /// Element under this point (global points). Defaults to the pointer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point: Option<Point>,
    /// Collect elements intersecting this region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Rect>,
    /// 1–600.
    pub max_nodes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessibilityStatus {
    pub state: PermissionState,
}

/// Model-ready accessibility context: the few elements that matter, plus a compact text form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AxContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under_pointer: Option<AxNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused: Option<AxNode>,
    pub nodes: Vec<AxNode>,
    /// One line per element, e.g. `button "Submit" at (820,440) 120×32`.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AxRankParams {
    pub snapshot: AxSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Rect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Point>,
    /// 1–100.
    pub limit: u32,
}
