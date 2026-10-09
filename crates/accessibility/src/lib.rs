//! Relevance filtering for accessibility context (spec §36, §38: "Never dump the entire
//! accessibility tree into the prompt").
//!
//! The host collects a bounded set of nodes; this crate keeps the ones that matter — interactive,
//! labelled, inside the focus region, near the pointer, focused — in reading order, and renders a
//! compact one-line-per-element summary for small local models.

use std::collections::HashSet;

use wisp_common::geometry::{Point, Rect};
use wisp_ipc::types::accessibility::{AxContext, AxNode, AxSnapshot};

const MAX_LABEL: usize = 120;

fn interactive(role: &str) -> bool {
    matches!(
        role,
        "Button"
            | "Link"
            | "TextField"
            | "TextArea"
            | "SearchField"
            | "CheckBox"
            | "RadioButton"
            | "PopUpButton"
            | "ComboBox"
            | "MenuItem"
            | "MenuButton"
            | "MenuBarItem"
            | "Tab"
            | "Slider"
            | "Switch"
            | "Toggle"
            | "Cell"
            | "Row"
            | "DisclosureTriangle"
            | "Incrementor"
            | "ColorWell"
            | "SecureTextField"
    )
}

fn informative(role: &str) -> bool {
    matches!(
        role,
        "StaticText"
            | "Heading"
            | "Image"
            | "Window"
            | "Sheet"
            | "Dialog"
            | "Table"
            | "List"
            | "Outline"
    )
}

fn label(n: &AxNode) -> Option<&str> {
    [&n.title, &n.description, &n.value]
        .into_iter()
        .flatten()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
}

fn area(r: &Rect) -> f64 {
    r.width.max(0.0) * r.height.max(0.0)
}

fn overlap_fraction(frame: &Rect, region: &Rect) -> f64 {
    let a = area(frame);
    if a <= 0.0 {
        return 0.0;
    }
    frame
        .intersection(region)
        .map(|i| area(&i) / a)
        .unwrap_or(0.0)
}

fn center(r: &Rect) -> Point {
    Point {
        x: r.x + r.width / 2.0,
        y: r.y + r.height / 2.0,
    }
}

/// Score an element's relevance. Higher is more relevant; `None` means drop it.
pub fn score(n: &AxNode, region: Option<&Rect>, cursor: Option<Point>) -> Option<f64> {
    let frame = n
        .frame
        .as_ref()
        .filter(|f| f.width >= 1.0 && f.height >= 1.0)?;
    let overlap = match region {
        Some(r) => {
            let o = overlap_fraction(frame, r);
            if o <= 0.0 {
                return None;
            }
            o
        }
        None => 1.0,
    };
    let has_label = label(n).is_some();
    let mut s = 0.0;
    if interactive(&n.role) {
        s += 3.0;
    } else if informative(&n.role) && has_label {
        s += 2.0;
    } else if !has_label && !n.focused {
        return None; // unlabeled containers carry no meaning for the model
    }
    if has_label {
        s += 2.0;
    }
    s += 3.0 * overlap;
    if n.focused {
        s += 3.0;
    }
    if n.selected {
        s += 1.0;
    }
    if !n.enabled {
        s -= 0.5;
    }
    if let Some(c) = cursor {
        let m = center(frame);
        let d = ((m.x - c.x).powi(2) + (m.y - c.y).powi(2)).sqrt();
        s += 2.0 / (1.0 + d / 200.0);
        if frame.contains(c) {
            s += 1.5;
        }
    }
    Some(s)
}

/// Remove anything that must never reach a model or a log.
pub fn sanitize(mut n: AxNode) -> AxNode {
    if n.role == "SecureTextField" || n.subrole.as_deref() == Some("SecureTextField") {
        n.value = None;
    }
    let clip = |s: Option<String>| {
        s.map(|v| {
            let t = v.trim();
            if t.chars().count() > MAX_LABEL {
                let mut c: String = t.chars().take(MAX_LABEL).collect();
                c.push('…');
                c
            } else {
                t.to_owned()
            }
        })
        .filter(|v| !v.is_empty())
    };
    n.title = clip(n.title);
    n.description = clip(n.description);
    n.value = clip(n.value);
    n
}

fn line(n: &AxNode) -> String {
    let mut s = n.role.to_lowercase();
    if let Some(l) = label(n) {
        s.push_str(&format!(" \"{l}\""));
    }
    if let Some(f) = &n.frame {
        s.push_str(&format!(
            " at ({:.0},{:.0}) {:.0}×{:.0}",
            f.x, f.y, f.width, f.height
        ));
    }
    let mut flags = vec![];
    if n.focused {
        flags.push("focused");
    }
    if n.selected {
        flags.push("selected");
    }
    if !n.enabled {
        flags.push("disabled");
    }
    if !flags.is_empty() {
        s.push_str(&format!(" [{}]", flags.join(", ")));
    }
    s
}

/// Rank, deduplicate and trim a snapshot to at most `limit` elements in reading order.
pub fn rank(
    snapshot: AxSnapshot,
    region: Option<Rect>,
    cursor: Option<Point>,
    limit: usize,
) -> AxContext {
    let mut seen = HashSet::new();
    let mut scored: Vec<(f64, AxNode)> = snapshot
        .nodes
        .into_iter()
        .chain(snapshot.under_pointer.clone())
        .chain(snapshot.focused.clone())
        .map(sanitize)
        .filter_map(|n| score(&n, region.as_ref(), cursor).map(|s| (s, n)))
        .filter(|(_, n)| {
            let key = (
                n.role.clone(),
                label(n).map(str::to_owned),
                n.frame
                    .map(|f| (f.x as i64, f.y as i64, f.width as i64, f.height as i64)),
            );
            seen.insert(key)
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.truncate(limit.max(1));
    let mut nodes: Vec<AxNode> = scored.into_iter().map(|(_, n)| n).collect();
    nodes.sort_by(|a, b| {
        let (fa, fb) = (
            a.frame.unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)),
            b.frame.unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)),
        );
        // Reading order: rows (16 pt bands), then left to right.
        ((fa.y / 16.0).floor(), fa.x)
            .partial_cmp(&((fb.y / 16.0).floor(), fb.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut summary = String::new();
    if let Some(app) = &snapshot.application {
        summary.push_str(&format!("app: {}\n", app.name));
    }
    if let Some(w) = snapshot.window.as_ref().and_then(|w| w.title.as_deref()) {
        summary.push_str(&format!("window: {w}\n"));
    }
    if let Some(u) = snapshot.under_pointer.clone().map(sanitize) {
        summary.push_str(&format!("under pointer: {}\n", line(&u)));
    }
    if let Some(f) = snapshot.focused.clone().map(sanitize) {
        summary.push_str(&format!("keyboard focus: {}\n", line(&f)));
    }
    if let Some(t) = snapshot
        .selected_text
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let t: String = t.chars().take(400).collect();
        summary.push_str(&format!("selected text: \"{t}\"\n"));
    }
    if !nodes.is_empty() {
        summary.push_str("elements:\n");
        for n in &nodes {
            summary.push_str("- ");
            summary.push_str(&line(n));
            summary.push('\n');
        }
    }

    AxContext {
        under_pointer: snapshot.under_pointer.map(sanitize),
        focused: snapshot.focused.map(sanitize),
        nodes,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(role: &str, title: &str, frame: (f64, f64, f64, f64)) -> AxNode {
        AxNode {
            role: role.into(),
            subrole: None,
            title: (!title.is_empty()).then(|| title.into()),
            description: None,
            value: None,
            identifier: None,
            frame: Some(Rect::new(frame.0, frame.1, frame.2, frame.3)),
            enabled: true,
            focused: false,
            selected: false,
            actions: vec![],
            depth: 3,
        }
    }

    fn snapshot(nodes: Vec<AxNode>) -> AxSnapshot {
        AxSnapshot {
            application: None,
            window: None,
            under_pointer: None,
            ancestors: vec![],
            focused: None,
            nodes,
            selected_text: None,
            truncated: false,
            elapsed_ms: 1,
        }
    }

    #[test]
    fn keeps_relevant_elements_inside_the_region_in_reading_order() {
        let nodes = vec![
            node("Group", "", (0.0, 0.0, 1000.0, 800.0)),
            node("Button", "Cancel", (700.0, 440.0, 90.0, 30.0)),
            node("Button", "Submit", (820.0, 440.0, 120.0, 32.0)),
            node("StaticText", "Your order", (700.0, 380.0, 200.0, 20.0)),
            node("Button", "Far away", (10.0, 10.0, 80.0, 30.0)),
        ];
        let region = Rect::new(680.0, 360.0, 300.0, 140.0);
        let ctx = rank(
            snapshot(nodes),
            Some(region),
            Some(Point { x: 870.0, y: 455.0 }),
            10,
        );
        let titles: Vec<_> = ctx
            .nodes
            .iter()
            .filter_map(|n| n.title.as_deref())
            .collect();
        assert_eq!(titles, vec!["Your order", "Cancel", "Submit"]);
        assert!(
            ctx.summary
                .contains("button \"Submit\" at (820,440) 120×32")
        );
        assert!(!ctx.summary.contains("Far away"));
    }

    #[test]
    fn limit_prefers_interactive_near_cursor() {
        let mut nodes: Vec<AxNode> = (0..50)
            .map(|i| {
                node(
                    "StaticText",
                    &format!("t{i}"),
                    (0.0, i as f64 * 20.0, 100.0, 18.0),
                )
            })
            .collect();
        nodes.push(node("Button", "Target", (500.0, 500.0, 80.0, 30.0)));
        let ctx = rank(snapshot(nodes), None, Some(Point { x: 540.0, y: 515.0 }), 5);
        assert_eq!(ctx.nodes.len(), 5);
        assert!(
            ctx.nodes
                .iter()
                .any(|n| n.title.as_deref() == Some("Target"))
        );
    }

    #[test]
    fn never_exposes_secure_field_values_and_clips_long_text() {
        let mut pw = node("SecureTextField", "Password", (0.0, 0.0, 200.0, 24.0));
        pw.value = Some("hunter2".into());
        let mut long = node("StaticText", "", (0.0, 40.0, 200.0, 24.0));
        long.value = Some("x".repeat(500));
        let ctx = rank(snapshot(vec![pw, long]), None, None, 10);
        assert!(!ctx.summary.contains("hunter2"));
        assert!(ctx.nodes.iter().all(|n| {
            n.value
                .as_ref()
                .is_none_or(|v| v.chars().count() <= MAX_LABEL + 1)
        }));
    }

    #[test]
    fn deduplicates_and_drops_frameless_or_unlabeled_containers() {
        let a = node("Button", "OK", (10.0, 10.0, 50.0, 20.0));
        let mut frameless = node("Button", "Ghost", (0.0, 0.0, 0.0, 0.0));
        frameless.frame = None;
        let ctx = rank(
            snapshot(vec![
                a.clone(),
                a,
                frameless,
                node("Group", "", (0.0, 0.0, 100.0, 100.0)),
            ]),
            None,
            None,
            10,
        );
        assert_eq!(ctx.nodes.len(), 1);
    }
}
