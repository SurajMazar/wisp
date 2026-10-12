//! Visual guidance targets (spec §54): the model marks what it's talking about with
//! `[[point: Save]]` (a label as shown on screen) or `[[point: 412,230]]` (0–1000 coordinates in
//! the attached image, as vision models like Qwen-VL produce). Labels are resolved against the
//! Accessibility elements and recognized text, which carry exact screen frames — far more reliable
//! than asking a small model for pixels.

use wisp_common::geometry::{Point, Rect};
use wisp_ipc::types::{
    focus::FocusRegion,
    guidance::{
        Annotation, ArrowAnnotation, CircleAnnotation, GuidanceMark, GuidanceShow, GuidanceTarget,
        LabelAnnotation, RectAnnotation, TargetSource,
    },
};

/// At most this many targets per answer.
pub const MAX_TARGETS: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub enum TargetRef {
    Label(String),
    /// 0–1000 relative to the image.
    Coordinates(f64, f64),
}

/// A marker in an answer: what to mark and how.
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub target: TargetRef,
    pub mark: GuidanceMark,
}

fn mark_kind(kind: &str) -> Option<GuidanceMark> {
    Some(match kind.trim().to_ascii_lowercase().as_str() {
        "point" | "highlight" => GuidanceMark::Point,
        "circle" => GuidanceMark::Circle,
        "arrow" => GuidanceMark::Arrow,
        "underline" => GuidanceMark::Underline,
        "spotlight" => GuidanceMark::Spotlight,
        _ => return None,
    })
}

/// Find complete `[[point: …]]`, `[[circle: …]]`, `[[arrow: …]]`, `[[underline: …]]` and
/// `[[spotlight: …]]` markers, in order.
pub fn parse_targets(text: &str) -> Vec<Marker> {
    let mut out = vec![];
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else { break };
        let inner = after[..end].trim();
        rest = &after[end + 2..];
        let Some((kind, body)) = inner.split_once(':') else {
            continue;
        };
        let Some(mark) = mark_kind(kind) else {
            continue;
        };
        let body = body
            .trim()
            .trim_matches(|c| c == '"' || c == '\'' || c == '“' || c == '”');
        if body.is_empty() || body.len() > 120 {
            continue;
        }
        let nums: Vec<f64> = body
            .trim_matches(|c| c == '(' || c == ')')
            .split(',')
            .map(|p| p.trim().parse::<f64>())
            .collect::<Result<_, _>>()
            .unwrap_or_default();
        let t = match nums[..] {
            [x, y] if (0.0..=1000.0).contains(&x) && (0.0..=1000.0).contains(&y) => {
                TargetRef::Coordinates(x, y)
            }
            _ => TargetRef::Label(body.to_owned()),
        };
        let m = Marker { target: t, mark };
        if !out.contains(&m) {
            out.push(m);
        }
        if out.len() == MAX_TARGETS {
            break;
        }
    }
    out
}

/// Remove markers for display and speech. A trailing unfinished `[[…` (mid-stream) is hidden too.
pub fn strip_markers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        match rest[start..].find("]]") {
            Some(end) => rest = &rest[start + end + 2..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    // Markers usually sit after a word: "Click Save [[point: Save]]." → "Click Save."
    out.replace(" .", ".").replace(" ,", ",").replace("  ", " ")
}

fn norm(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

struct Candidate {
    text: String,
    rect: Rect,
    source: TargetSource,
    actionable: bool,
}

fn candidates(region: &FocusRegion) -> Vec<Candidate> {
    let mut out = vec![];
    if let Some(ax) = &region.accessibility {
        for n in ax
            .under_pointer
            .iter()
            .chain(ax.focused.iter())
            .chain(ax.nodes.iter())
        {
            let Some(rect) = n.frame else { continue };
            for t in [&n.title, &n.description, &n.value].into_iter().flatten() {
                if !t.trim().is_empty() {
                    out.push(Candidate {
                        text: t.clone(),
                        rect,
                        source: TargetSource::Element,
                        actionable: !n.actions.is_empty(),
                    });
                }
            }
        }
    }
    if let Some(t) = &region.text {
        for l in &t.lines {
            out.push(Candidate {
                text: l.text.clone(),
                rect: l.frame,
                source: TargetSource::Text,
                actionable: false,
            });
        }
    }
    out
}

/// Narrow a text line's frame to the part containing `needle` (assumes roughly even glyph widths).
fn sub_rect(line: &Rect, haystack: &str, needle: &str) -> Rect {
    let (h, n) = (haystack.to_lowercase(), needle.to_lowercase());
    let Some(i) = h.find(&n) else { return *line };
    let total = h.chars().count().max(1) as f64;
    let start = h[..i].chars().count() as f64;
    let len = n.chars().count() as f64;
    Rect::new(
        line.x + line.width * start / total,
        line.y,
        (line.width * len / total).max(8.0),
        line.height,
    )
}

/// Resolve one target to where it is on screen.
pub fn resolve(t: &TargetRef, region: &FocusRegion) -> Option<GuidanceTarget> {
    match t {
        TargetRef::Coordinates(x, y) => {
            let c = &region.crop;
            let p = Point {
                x: c.x + c.width * x / 1000.0,
                y: c.y + c.height * y / 1000.0,
            };
            Some(GuidanceTarget {
                label: String::new(),
                rect: Rect::new(p.x - 14.0, p.y - 14.0, 28.0, 28.0),
                source: TargetSource::Coordinates,
                mark: GuidanceMark::Point,
            })
        }
        TargetRef::Label(label) => {
            let want = norm(label);
            if want.is_empty() {
                return None;
            }
            let want_words: Vec<&str> = want.split(' ').collect();
            let mut best: Option<(f64, &Candidate)> = None;
            let all = candidates(region);
            for c in &all {
                let have = norm(&c.text);
                if have.is_empty() {
                    continue;
                }
                let mut score = if have == want {
                    100.0
                } else if format!(" {have} ").contains(&format!(" {want} ")) {
                    // The label is part of a longer element/line; shorter containers win.
                    75.0 - (have.len() as f64 / 20.0).min(15.0)
                } else if want.len() >= 3 && format!(" {want} ").contains(&format!(" {have} ")) {
                    60.0
                } else {
                    let have_words: Vec<&str> = have.split(' ').collect();
                    let common =
                        want_words.iter().filter(|w| have_words.contains(w)).count() as f64;
                    let j = common / (want_words.len() + have_words.len()) as f64 * 2.0;
                    if j >= 0.5 { 50.0 * j } else { 0.0 }
                };
                if score == 0.0 {
                    continue;
                }
                if c.source == TargetSource::Element {
                    score += if c.actionable { 6.0 } else { 3.0 };
                }
                if best.as_ref().is_none_or(|(s, b)| {
                    score > *s
                        || (score == *s
                            && c.rect.width * c.rect.height < b.rect.width * b.rect.height)
                }) {
                    best = Some((score, c));
                }
            }
            best.map(|(_, c)| GuidanceTarget {
                label: label.clone(),
                rect: if c.source == TargetSource::Text {
                    sub_rect(&c.rect, &c.text, label)
                } else {
                    c.rect
                },
                source: c.source,
                mark: GuidanceMark::Point,
            })
        }
    }
}

fn padded(r: &Rect, pad: f64) -> Rect {
    Rect::new(
        r.x - pad,
        r.y - pad,
        r.width + pad * 2.0,
        r.height + pad * 2.0,
    )
}

/// An arrow coming in diagonally from the upper left (lower right near the screen's top-left),
/// stopping just short of the target's edge.
fn arrow_to(r: &Rect) -> (Point, Point) {
    let c = Point {
        x: r.x + r.width / 2.0,
        y: r.y + r.height / 2.0,
    };
    let (dx, dy) = if r.x < 140.0 || r.y < 110.0 {
        (1.0, 1.0)
    } else {
        (-1.0, -1.0)
    };
    // Where the line from the centre towards the arrow's tail leaves the (padded) target.
    let (hw, hh) = (r.width / 2.0 + 6.0, r.height / 2.0 + 6.0);
    let t = (hw / 0.8).min(hh / 0.6);
    let to = Point {
        x: c.x + dx * 0.8 * t,
        y: c.y + dy * 0.6 * t,
    };
    let from = Point {
        x: to.x + dx * 110.0,
        y: to.y + dy * 82.0,
    };
    (from, to)
}

/// What to draw for resolved targets, each as the model asked; the buddy points at the last one.
pub fn show(id: &str, targets: &[GuidanceTarget], duration_ms: u32) -> Option<GuidanceShow> {
    let last = targets.last()?;
    let annotations = targets
        .iter()
        .map(|t| {
            let label = (!t.label.is_empty()).then(|| t.label.clone());
            match t.mark {
                GuidanceMark::Point => Annotation::Highlight(RectAnnotation {
                    rect: padded(&t.rect, 4.0),
                    label,
                }),
                GuidanceMark::Circle => Annotation::Circle(CircleAnnotation {
                    center: Point {
                        x: t.rect.x + t.rect.width / 2.0,
                        y: t.rect.y + t.rect.height / 2.0,
                    },
                    radius: (t.rect.width.hypot(t.rect.height) / 2.0 + 8.0).max(18.0),
                    label,
                }),
                GuidanceMark::Arrow => {
                    let (from, to) = arrow_to(&t.rect);
                    Annotation::Arrow(ArrowAnnotation { from, to, label })
                }
                GuidanceMark::Underline => Annotation::Highlight(RectAnnotation {
                    rect: Rect::new(t.rect.x, t.rect.y + t.rect.height + 1.0, t.rect.width, 3.0),
                    label: None,
                }),
                GuidanceMark::Spotlight => Annotation::Spotlight(RectAnnotation {
                    rect: padded(&t.rect, 8.0),
                    label,
                }),
            }
        })
        .chain(
            targets
                .iter()
                .filter(|t| t.source == TargetSource::Coordinates)
                .map(|t| {
                    Annotation::Pointer(LabelAnnotation {
                        at: Point {
                            x: t.rect.x + t.rect.width / 2.0,
                            y: t.rect.y + t.rect.height / 2.0,
                        },
                        text: t.label.clone(),
                    })
                }),
        )
        .collect();
    Some(GuidanceShow {
        id: id.to_owned(),
        annotations,
        buddy_target: Some(Point {
            x: last.rect.x + last.rect.width / 2.0,
            y: last.rect.y + last.rect.height / 2.0,
        }),
        duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_ipc::types::{
        accessibility::{AxContext, AxNode},
        focus::{DisplayInfo, FocusKind, ScreenText, TextLine},
    };

    fn node(role: &str, title: &str, rect: Rect, press: bool) -> AxNode {
        AxNode {
            role: role.into(),
            subrole: None,
            title: Some(title.into()),
            description: None,
            value: None,
            identifier: None,
            frame: Some(rect),
            enabled: true,
            focused: false,
            selected: false,
            actions: if press {
                vec!["AXPress".into()]
            } else {
                vec![]
            },
            depth: 3,
        }
    }

    fn region() -> FocusRegion {
        FocusRegion {
            id: wisp_common::ids::new_id(),
            kind: FocusKind::Window,
            display: DisplayInfo {
                id: 1,
                name: "Built-in".into(),
                frame: Rect::new(0.0, 0.0, 1512.0, 982.0),
                scale: 2.0,
            },
            selection: Rect::new(100.0, 100.0, 600.0, 400.0),
            crop: Rect::new(100.0, 100.0, 600.0, 400.0),
            path: vec![],
            cursor: Point { x: 0.0, y: 0.0 },
            image: None,
            application: None,
            window: None,
            accessibility: Some(AxContext {
                under_pointer: None,
                focused: None,
                nodes: vec![
                    node(
                        "AXStaticText",
                        "Save your changes before closing?",
                        Rect::new(150.0, 150.0, 300.0, 20.0),
                        false,
                    ),
                    node(
                        "AXButton",
                        "Save",
                        Rect::new(560.0, 440.0, 80.0, 28.0),
                        true,
                    ),
                    node(
                        "AXButton",
                        "Don't Save",
                        Rect::new(380.0, 440.0, 100.0, 28.0),
                        true,
                    ),
                ],
                summary: String::new(),
            }),
            text: Some(ScreenText {
                lines: vec![TextLine {
                    text: "Version history: Restore earlier version".into(),
                    frame: Rect::new(150.0, 200.0, 400.0, 20.0),
                    confidence: 0.9,
                }],
                elapsed_ms: 1,
            }),
            created_at_ms: 0,
        }
    }

    #[test]
    fn parses_markers_and_strips_them_for_display() {
        let s = "Click Save [[point: Save]] to keep it, or [[point: \"Don't Save\"]]. Here: [[point: 412, 230]]";
        let point = |t| Marker {
            target: t,
            mark: GuidanceMark::Point,
        };
        assert_eq!(
            parse_targets(s),
            vec![
                point(TargetRef::Label("Save".into())),
                point(TargetRef::Label("Don't Save".into())),
                point(TargetRef::Coordinates(412.0, 230.0))
            ]
        );
        assert_eq!(strip_markers(s), "Click Save to keep it, or. Here: ");
        assert_eq!(strip_markers("Press Enter [[poi"), "Press Enter ");
        assert!(parse_targets("[[not a point]] [[point: ]]").is_empty());
    }

    #[test]
    fn marks_are_drawn_as_asked() {
        let m = parse_targets(
            "Use [[circle: Save]], then [[arrow: Export]] and read [[underline: Total]] [[SPOTLIGHT: Panel]] [[wiggle: x]]",
        );
        assert_eq!(
            m.iter().map(|m| m.mark).collect::<Vec<_>>(),
            [
                GuidanceMark::Circle,
                GuidanceMark::Arrow,
                GuidanceMark::Underline
            ],
            "unknown kinds are ignored; at most three"
        );
        let target = |mark| GuidanceTarget {
            label: "Save".into(),
            rect: Rect::new(400.0, 300.0, 80.0, 24.0),
            source: TargetSource::Element,
            mark,
        };
        let shown = show(
            "m",
            &[
                target(GuidanceMark::Circle),
                target(GuidanceMark::Arrow),
                target(GuidanceMark::Underline),
                target(GuidanceMark::Spotlight),
            ],
            5000,
        )
        .expect("show");
        assert!(
            matches!(&shown.annotations[0], Annotation::Circle(c) if c.center == Point { x: 440.0, y: 312.0 } && c.radius > 40.0)
        );
        match &shown.annotations[1] {
            Annotation::Arrow(a) => {
                assert!(
                    a.from.x < a.to.x && a.from.y < a.to.y,
                    "comes in from the upper left"
                );
                assert!(a.to.x < 400.0 || a.to.y < 300.0, "stops outside the target");
            }
            other => panic!("expected an arrow, got {other:?}"),
        }
        assert!(
            matches!(&shown.annotations[2], Annotation::Highlight(r) if r.rect.height == 3.0 && r.rect.y == 325.0)
        );
        assert!(matches!(&shown.annotations[3], Annotation::Spotlight(_)));
        // Near the top-left corner the arrow comes from the lower right instead.
        let corner = GuidanceTarget {
            rect: Rect::new(20.0, 30.0, 60.0, 20.0),
            ..target(GuidanceMark::Arrow)
        };
        let s = show("m", &[corner], 5000).expect("show");
        assert!(
            matches!(&s.annotations[0], Annotation::Arrow(a) if a.from.x > a.to.x && a.from.y > a.to.y)
        );
    }

    #[test]
    fn resolves_labels_to_the_exact_element_not_a_sentence_containing_it() {
        let r = region();
        let t = resolve(&TargetRef::Label("Save".into()), &r).expect("found");
        assert_eq!(
            (t.rect, t.source),
            (Rect::new(560.0, 440.0, 80.0, 28.0), TargetSource::Element)
        );
        let t = resolve(&TargetRef::Label("don't save".into()), &r).expect("found");
        assert_eq!(t.rect.x, 380.0);
        assert!(resolve(&TargetRef::Label("Print".into()), &r).is_none());
    }

    #[test]
    fn narrows_recognized_text_to_the_words() {
        let t = resolve(
            &TargetRef::Label("Restore earlier version".into()),
            &region(),
        )
        .expect("found");
        assert_eq!(t.source, TargetSource::Text);
        assert!(
            t.rect.x > 150.0 + 100.0 && t.rect.width < 400.0,
            "{:?}",
            t.rect
        );
    }

    #[test]
    fn maps_vision_coordinates_into_the_capture() {
        let t = resolve(&TargetRef::Coordinates(500.0, 500.0), &region()).expect("mapped");
        assert_eq!((t.rect.x + 14.0, t.rect.y + 14.0), (400.0, 300.0));
        let s = show("g", &[t], 8000).expect("show");
        assert_eq!(s.buddy_target, Some(Point { x: 400.0, y: 300.0 }));
        assert_eq!(s.annotations.len(), 2);
    }
}
