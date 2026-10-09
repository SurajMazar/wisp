//! Turns a focus region into compact, model-ready text (spec §35–36, §97).
//!
//! The screen is untrusted input: anything shown on it (a web page, an email) could contain text
//! that looks like instructions. Everything here is fenced as quoted screen content and control
//! characters are stripped, so the model treats it as data.

use wisp_ipc::types::focus::{FocusKind, FocusRegion};

/// What went into the description, for the "context used" disclosure.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Description {
    pub text: String,
    pub text_lines: u32,
    pub ui_elements: u32,
    pub selected_text: bool,
    /// Some on-screen text was left out to fit the budget.
    pub truncated: bool,
}

fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_owned()
}

fn what(kind: FocusKind) -> &'static str {
    match kind {
        FocusKind::Window => "a window",
        FocusKind::Screen => "the whole screen",
        FocusKind::Point => "the spot they clicked",
        _ => "an area they selected",
    }
}

/// Describe `region` in at most roughly `budget_chars` characters.
/// `with_image` says whether the screenshot itself is also attached.
pub fn describe(region: &FocusRegion, budget_chars: usize, with_image: bool) -> Description {
    let mut d = Description::default();
    let mut head = String::new();
    head.push_str(&format!(
        "The user is pointing at {} ({}×{} pt){}.\n",
        what(region.kind),
        region.selection.width.round(),
        region.selection.height.round(),
        if with_image {
            "; the attached image shows it"
        } else {
            "; no image is attached, so rely on the text below"
        }
    ));
    if let Some(app) = &region.application {
        head.push_str(&format!("App: {}\n", clean(&app.name)));
    }
    if let Some(t) = region.window.as_ref().and_then(|w| w.title.as_deref()) {
        head.push_str(&format!("Window: {}\n", clean(t)));
    }

    // Accessibility: already ranked and summarized by wisp-accessibility. Drop its app/window
    // lines (said above).
    let mut ax = String::new();
    if let Some(a) = &region.accessibility {
        d.ui_elements = a.nodes.len() as u32;
        for line in a.summary.lines() {
            if line.starts_with("app: ") || line.starts_with("window: ") {
                continue;
            }
            if line.starts_with("selected text: ") {
                d.selected_text = true;
            }
            ax.push_str(&clean(line));
            ax.push('\n');
        }
    }

    // Spend what's left of the budget on recognized text, in reading order.
    let mut ocr = String::new();
    if let Some(t) = &region.text {
        let room = budget_chars.saturating_sub(head.len() + ax.len()).max(400);
        for l in &t.lines {
            let line = clean(&l.text);
            if line.is_empty() {
                continue;
            }
            if ocr.len() + line.len() + 1 > room {
                d.truncated = true;
                break;
            }
            ocr.push_str(&line);
            ocr.push('\n');
            d.text_lines += 1;
        }
    }

    let mut out = String::from("<screen>\n");
    out.push_str(&head);
    if !ax.is_empty() {
        out.push_str("UI elements there:\n");
        out.push_str(&ax);
    }
    if !ocr.is_empty() {
        out.push_str("Text visible there (top to bottom):\n\"\"\"\n");
        out.push_str(&ocr);
        out.push_str("\"\"\"\n");
        if d.truncated {
            out.push_str("(more text not shown)\n");
        }
    }
    out.push_str("</screen>");
    d.text = out;
    d
}

/// True when the region has enough text that a text-only model can still help.
pub fn readable_without_image(region: &FocusRegion) -> bool {
    let ocr_chars: usize = region
        .text
        .as_ref()
        .map(|t| t.lines.iter().map(|l| l.text.len()).sum())
        .unwrap_or(0);
    let ax = region
        .accessibility
        .as_ref()
        .is_some_and(|a| !a.nodes.is_empty() || a.under_pointer.is_some());
    ocr_chars >= 3 || ax
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_common::geometry::{Point, Rect};
    use wisp_ipc::types::{
        accessibility::AxContext,
        focus::{ApplicationInfo, DisplayInfo, ScreenText, TextLine},
    };

    fn region(lines: &[&str]) -> FocusRegion {
        FocusRegion {
            id: wisp_common::ids::new_id(),
            kind: FocusKind::Rectangle,
            display: DisplayInfo {
                id: 1,
                name: "Built-in".into(),
                frame: Rect::new(0.0, 0.0, 1512.0, 982.0),
                scale: 2.0,
            },
            selection: Rect::new(10.0, 10.0, 320.0, 180.0),
            crop: Rect::new(0.0, 0.0, 340.0, 200.0),
            path: vec![],
            cursor: Point { x: 20.0, y: 20.0 },
            image: None,
            application: Some(ApplicationInfo {
                name: "Finder".into(),
                bundle_id: None,
                pid: 1,
            }),
            window: None,
            accessibility: Some(AxContext {
                under_pointer: None,
                focused: None,
                nodes: vec![],
                summary: "app: Finder\nunder pointer: button \"Delete\"\nselected text: \"x\"\n"
                    .into(),
            }),
            text: Some(ScreenText {
                lines: lines
                    .iter()
                    .map(|t| TextLine {
                        text: (*t).into(),
                        frame: Rect::new(0.0, 0.0, 10.0, 10.0),
                        confidence: 1.0,
                    })
                    .collect(),
                elapsed_ms: 5,
            }),
            created_at_ms: 0,
        }
    }

    #[test]
    fn fences_screen_text_and_reports_what_was_used() {
        let d = describe(&region(&["Delete 3 files?", "Cancel\u{7}"]), 4000, false);
        assert!(d.text.starts_with("<screen>\n") && d.text.ends_with("</screen>"));
        assert!(d.text.contains("App: Finder"));
        assert!(!d.text.contains("app: Finder"), "app line not repeated");
        assert!(d.text.contains("no image is attached"));
        assert!(d.text.contains("\"\"\"\nDelete 3 files?\nCancel\n\"\"\""));
        assert_eq!(
            (d.text_lines, d.selected_text, d.truncated),
            (2, true, false)
        );
    }

    #[test]
    fn keeps_to_the_budget() {
        let long: Vec<String> = (0..500).map(|i| format!("line number {i}")).collect();
        let refs: Vec<&str> = long.iter().map(String::as_str).collect();
        let d = describe(&region(&refs), 1500, true);
        assert!(d.truncated && d.text.len() < 1700, "{}", d.text.len());
        assert!(d.text.contains("(more text not shown)"));
    }

    #[test]
    fn knows_when_text_is_enough() {
        assert!(readable_without_image(&region(&["Save"])));
        let mut empty = region(&[]);
        empty.accessibility = None;
        assert!(!readable_without_image(&empty));
    }
}
