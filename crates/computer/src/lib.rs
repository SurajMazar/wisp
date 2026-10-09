//! Computer-control tools (spec §55–56): definitions, argument validation, risk assessment and
//! plain-language titles. Pure — execution happens in the host after the policy engine
//! (`wisp-permissions`) allows it.

use wisp_ipc::types::tools::{KeyModifier, RiskLevel, ToolCall, ToolDefinition};

pub const MAX_TYPE_CHARS: usize = 2_000;
pub const MAX_CLIPBOARD_CHARS: usize = 100_000;
const MAX_COORD: f64 = 100_000.0;

pub const NAMED_KEYS: &[&str] = &[
    "return",
    "tab",
    "escape",
    "space",
    "delete",
    "forwardDelete",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "pageUp",
    "pageDown",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
];

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ToolError {
    #[error("{0}")]
    Invalid(String),
}

fn invalid(s: impl Into<String>) -> ToolError {
    ToolError::Invalid(s.into())
}

pub fn name(call: &ToolCall) -> &'static str {
    match call {
        ToolCall::ReadScreen(_) => "readScreen",
        ToolCall::FindElement(_) => "findElement",
        ToolCall::Click(_) => "click",
        ToolCall::DoubleClick(_) => "doubleClick",
        ToolCall::RightClick(_) => "rightClick",
        ToolCall::MoveMouse(_) => "moveMouse",
        ToolCall::Drag(_) => "drag",
        ToolCall::Scroll(_) => "scroll",
        ToolCall::TypeText(_) => "typeText",
        ToolCall::Keypress(_) => "keypress",
        ToolCall::Hotkey(_) => "hotkey",
        ToolCall::Copy(_) => "copy",
        ToolCall::Paste(_) => "paste",
        ToolCall::OpenApplication(_) => "openApplication",
        ToolCall::SwitchApplication(_) => "switchApplication",
        ToolCall::CloseApplication(_) => "closeApplication",
        ToolCall::OpenUrl(_) => "openUrl",
        ToolCall::ReadClipboard(_) => "readClipboard",
        ToolCall::WriteClipboard(_) => "writeClipboard",
    }
}

/// (name, title, description, default risk)
const DEFINITIONS: &[(&str, &str, &str, RiskLevel)] = &[
    (
        "readScreen",
        "Read the screen",
        "Describe the controls in the frontmost window",
        RiskLevel::Safe,
    ),
    (
        "findElement",
        "Find a control",
        "Locate a button or field by its label",
        RiskLevel::Safe,
    ),
    (
        "moveMouse",
        "Move the pointer",
        "Move the mouse pointer to a spot",
        RiskLevel::Safe,
    ),
    (
        "click",
        "Click",
        "Click a button or spot on screen",
        RiskLevel::Low,
    ),
    (
        "doubleClick",
        "Double-click",
        "Double-click a spot on screen",
        RiskLevel::Low,
    ),
    (
        "rightClick",
        "Right-click",
        "Open the context menu at a spot",
        RiskLevel::Low,
    ),
    ("scroll", "Scroll", "Scroll at a spot", RiskLevel::Low),
    (
        "keypress",
        "Press a key",
        "Press a single key such as Return or Tab",
        RiskLevel::Low,
    ),
    ("copy", "Copy", "Copy the current selection", RiskLevel::Low),
    (
        "openApplication",
        "Open an app",
        "Launch an application",
        RiskLevel::Low,
    ),
    (
        "switchApplication",
        "Switch apps",
        "Bring a running application to the front",
        RiskLevel::Low,
    ),
    (
        "writeClipboard",
        "Set the clipboard",
        "Put text on the clipboard",
        RiskLevel::Low,
    ),
    (
        "drag",
        "Drag",
        "Drag from one spot to another",
        RiskLevel::Medium,
    ),
    (
        "typeText",
        "Type text",
        "Type text into the focused field",
        RiskLevel::Medium,
    ),
    (
        "hotkey",
        "Keyboard shortcut",
        "Press a keyboard shortcut",
        RiskLevel::Medium,
    ),
    ("paste", "Paste", "Paste the clipboard", RiskLevel::Medium),
    (
        "closeApplication",
        "Quit an app",
        "Ask an application to quit",
        RiskLevel::Medium,
    ),
    (
        "openUrl",
        "Open a link",
        "Open a web address in the browser",
        RiskLevel::Medium,
    ),
    (
        "readClipboard",
        "Read the clipboard",
        "Read text on the clipboard",
        RiskLevel::Medium,
    ),
];

pub fn definitions() -> Vec<ToolDefinition> {
    DEFINITIONS
        .iter()
        .map(|(name, title, description, risk)| ToolDefinition {
            name: (*name).into(),
            title: (*title).into(),
            description: (*description).into(),
            risk: *risk,
            requires_permission: *risk != RiskLevel::Safe,
        })
        .collect()
}

fn base_risk(call: &ToolCall) -> RiskLevel {
    let n = name(call);
    DEFINITIONS
        .iter()
        .find(|d| d.0 == n)
        .map_or(RiskLevel::High, |d| d.3)
}

fn finite(v: f64, what: &str) -> Result<(), ToolError> {
    if v.is_finite() && v.abs() <= MAX_COORD {
        Ok(())
    } else {
        Err(invalid(format!("{what} is out of range")))
    }
}

fn label_ok(l: &Option<String>) -> Result<(), ToolError> {
    match l {
        Some(l) if l.chars().count() > 200 => Err(invalid("label is too long")),
        _ => Ok(()),
    }
}

fn is_key(k: &str) -> bool {
    NAMED_KEYS.contains(&k) || (k.chars().count() == 1 && k.chars().all(|c| c.is_ascii_graphic()))
}

/// Structural validation beyond the type system (spec §56: no unvalidated tool calls).
pub fn validate(call: &ToolCall) -> Result<(), ToolError> {
    match call {
        ToolCall::ReadScreen(_)
        | ToolCall::Copy(_)
        | ToolCall::Paste(_)
        | ToolCall::ReadClipboard(_) => Ok(()),
        ToolCall::FindElement(f) => {
            if f.label.trim().is_empty() || f.label.chars().count() > 200 {
                Err(invalid("label must be 1–200 characters"))
            } else {
                Ok(())
            }
        }
        ToolCall::Click(p)
        | ToolCall::DoubleClick(p)
        | ToolCall::RightClick(p)
        | ToolCall::MoveMouse(p) => {
            finite(p.x, "x")?;
            finite(p.y, "y")?;
            label_ok(&p.label)
        }
        ToolCall::Drag(d) => {
            for (v, w) in [
                (d.from.x, "from.x"),
                (d.from.y, "from.y"),
                (d.to.x, "to.x"),
                (d.to.y, "to.y"),
            ] {
                finite(v, w)?;
            }
            Ok(())
        }
        ToolCall::Scroll(s) => {
            finite(s.x, "x")?;
            finite(s.y, "y")?;
            if s.dy.abs() > 50 || s.dx.abs() > 50 {
                return Err(invalid("scroll at most 50 lines at a time"));
            }
            Ok(())
        }
        ToolCall::TypeText(t) => {
            if t.text.is_empty() || t.text.chars().count() > MAX_TYPE_CHARS {
                return Err(invalid(format!(
                    "text must be 1–{MAX_TYPE_CHARS} characters"
                )));
            }
            if t.text
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
            {
                return Err(invalid("text contains control characters"));
            }
            Ok(())
        }
        ToolCall::WriteClipboard(t) => {
            if t.text.chars().count() > MAX_CLIPBOARD_CHARS {
                Err(invalid("text is too long for the clipboard"))
            } else {
                Ok(())
            }
        }
        ToolCall::Keypress(k) => {
            if NAMED_KEYS.contains(&k.key.as_str()) {
                Ok(())
            } else {
                Err(invalid(format!("unknown key {:?}", k.key)))
            }
        }
        ToolCall::Hotkey(h) => {
            if !is_key(&h.key) {
                return Err(invalid(format!("unknown key {:?}", h.key)));
            }
            if h.modifiers.is_empty() {
                return Err(invalid(
                    "a shortcut needs at least one modifier; use keypress",
                ));
            }
            Ok(())
        }
        ToolCall::OpenApplication(a)
        | ToolCall::SwitchApplication(a)
        | ToolCall::CloseApplication(a) => {
            let ok = !a.app.trim().is_empty()
                && a.app.chars().count() <= 120
                && !a.app.contains('/')
                && !a.app.contains("..")
                && a.app
                    .chars()
                    .all(|c| c.is_alphanumeric() || " .-_&+'()".contains(c));
            if ok {
                Ok(())
            } else {
                Err(invalid(
                    "app must be an application name or bundle identifier",
                ))
            }
        }
        ToolCall::OpenUrl(u) => {
            let lower = u.url.to_ascii_lowercase();
            let ok = (lower.starts_with("https://") || lower.starts_with("http://"))
                && u.url.len() <= 2048
                && !u.url.chars().any(|c| c.is_whitespace() || c.is_control());
            if ok {
                Ok(())
            } else {
                Err(invalid("only http(s) web addresses can be opened"))
            }
        }
    }
}

const CRITICAL_PHRASES: &[&str] = &[
    "empty trash",
    "delete permanently",
    "permanently delete",
    "erase",
    "format disk",
    "factory reset",
    "delete account",
    "close account",
    "wipe",
];
const HIGH_WORDS: &[&str] = &[
    "delete",
    "remove",
    "trash",
    "discard",
    "uninstall",
    "reset",
    "revoke",
    "overwrite",
    "replace",
    "pay",
    "purchase",
    "buy",
    "checkout",
    "order",
    "transfer",
    "send",
    "submit",
    "publish",
    "post",
    "confirm",
    "sign out",
    "log out",
    "unsubscribe",
    "cancel subscription",
    "deactivate",
];

fn words(s: &str) -> String {
    let w: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    format!(" {} ", w.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn label_risk(label: &str) -> Option<(RiskLevel, String)> {
    let w = words(label);
    if let Some(p) = CRITICAL_PHRASES
        .iter()
        .find(|p| w.contains(&format!(" {p} ")))
    {
        return Some((
            RiskLevel::Critical,
            format!("“{label}” sounds irreversible ({p})"),
        ));
    }
    HIGH_WORDS
        .iter()
        .find(|p| w.contains(&format!(" {p} ")))
        .map(|p| (RiskLevel::High, format!("“{label}” may {p} something")))
}

/// Risk of this specific call (spec §56), raised above the tool default when the target or
/// keys suggest something destructive, financial or hard to undo.
pub fn assess(call: &ToolCall) -> (RiskLevel, Option<String>) {
    let base = base_risk(call);
    let raised = match call {
        ToolCall::Click(p) | ToolCall::DoubleClick(p) | ToolCall::RightClick(p)
            if p.label.is_some() =>
        {
            p.label.as_deref().and_then(label_risk)
        }
        ToolCall::TypeText(t) if t.text.contains('\n') => Some((
            base.raise(),
            "the text includes a line break, which may submit or send it".into(),
        )),
        ToolCall::Keypress(k) if k.key == "return" => Some((
            RiskLevel::High,
            "Return may submit a form, send a message or run a command".into(),
        )),
        ToolCall::Paste(_) => Some((
            RiskLevel::High,
            "the clipboard may contain anything, including line breaks that submit".into(),
        )),
        ToolCall::Click(p) | ToolCall::DoubleClick(p) | ToolCall::RightClick(p)
            if p.label.is_none() =>
        {
            Some((
                RiskLevel::Medium,
                "Wisp can't tell what's at that spot".into(),
            ))
        }
        ToolCall::Hotkey(h) => {
            let cmd = h.modifiers.contains(&KeyModifier::Command);
            let shift = h.modifiers.contains(&KeyModifier::Shift);
            let opt = h.modifiers.contains(&KeyModifier::Option);
            match (cmd, h.key.to_lowercase().as_str()) {
                (true, "delete") if shift => {
                    Some((RiskLevel::Critical, "⇧⌘⌫ empties the Trash".into()))
                }
                (true, "delete") if opt => Some((
                    RiskLevel::Critical,
                    "⌥⌘⌫ deletes immediately, skipping the Trash".into(),
                )),
                (true, "delete") => Some((RiskLevel::High, "⌘⌫ moves items to the Trash".into())),
                (true, "q") => Some((RiskLevel::High, "⌘Q quits the app".into())),
                (true, "w") => Some((RiskLevel::High, "⌘W closes the window".into())),
                (true, "escape") if opt => Some((RiskLevel::High, "⌥⌘⎋ force-quits apps".into())),
                (true, "z") if shift => None,
                _ => None,
            }
        }
        ToolCall::OpenUrl(u) if u.url.to_ascii_lowercase().starts_with("http://") => {
            Some((RiskLevel::High, "the address isn't secure (http)".into()))
        }
        _ => None,
    };
    match raised {
        Some((r, why)) if r > base => (r, Some(why)),
        _ => (base, None),
    }
}

fn quote(s: &str) -> String {
    let t: String = s.chars().take(60).collect();
    if s.chars().count() > 60 {
        format!("“{t}…”")
    } else {
        format!("“{t}”")
    }
}

fn keys(h: &wisp_ipc::types::tools::HotkeyArgs) -> String {
    let mut s = String::new();
    for (m, g) in [
        (KeyModifier::Control, "⌃"),
        (KeyModifier::Option, "⌥"),
        (KeyModifier::Shift, "⇧"),
        (KeyModifier::Command, "⌘"),
    ] {
        if h.modifiers.contains(&m) {
            s.push_str(g);
        }
    }
    s.push_str(&h.key.to_uppercase());
    s
}

/// Tools that can never be allowed "always": what they do depends entirely on content the
/// model chooses (text, shortcuts, links, the clipboard).
pub fn always_grantable(tool: &str) -> bool {
    !matches!(
        tool,
        "typeText" | "paste" | "hotkey" | "openUrl" | "keypress" | "writeClipboard"
    )
}

/// Plain-language title for approvals and the activity log. Never includes typed text in full.
pub fn title(call: &ToolCall) -> String {
    let at = |p: &wisp_ipc::types::tools::PointerArgs| {
        p.label
            .as_deref()
            .map_or_else(|| format!("at ({:.0}, {:.0})", p.x, p.y), quote)
    };
    match call {
        ToolCall::ReadScreen(_) => "Read the screen".into(),
        ToolCall::FindElement(f) => format!("Find {}", quote(&f.label)),
        ToolCall::Click(p) => format!("Click {}", at(p)),
        ToolCall::DoubleClick(p) => format!("Double-click {}", at(p)),
        ToolCall::RightClick(p) => format!("Right-click {}", at(p)),
        ToolCall::MoveMouse(p) => format!("Move the pointer to {}", at(p)),
        ToolCall::Drag(d) => format!(
            "Drag from ({:.0}, {:.0}) to ({:.0}, {:.0})",
            d.from.x, d.from.y, d.to.x, d.to.y
        ),
        ToolCall::Scroll(s) => format!(
            "Scroll {} {} lines",
            if s.dy >= 0 { "down" } else { "up" },
            s.dy.abs()
        ),
        ToolCall::TypeText(t) => format!(
            "Type {} ({} characters)",
            quote(&t.text),
            t.text.chars().count()
        ),
        ToolCall::Keypress(k) => format!("Press {}", k.key),
        ToolCall::Hotkey(h) => format!("Press {}", keys(h)),
        ToolCall::Copy(_) => "Copy the selection".into(),
        ToolCall::Paste(_) => "Paste the clipboard".into(),
        ToolCall::OpenApplication(a) => format!("Open {}", a.app),
        ToolCall::SwitchApplication(a) => format!("Switch to {}", a.app),
        ToolCall::CloseApplication(a) => format!("Quit {}", a.app),
        ToolCall::OpenUrl(u) => format!("Open {}", u.url.chars().take(80).collect::<String>()),
        ToolCall::ReadClipboard(_) => "Read the clipboard".into(),
        ToolCall::WriteClipboard(t) => format!("Copy {} to the clipboard", quote(&t.text)),
    }
}

/// Audit-safe argument summary (spec §105: no private content by default).
pub fn redacted_args(call: &ToolCall) -> serde_json::Value {
    let mut v = serde_json::to_value(call).unwrap_or_default();
    if let Some(args) = v.get_mut("args").and_then(|a| a.as_object_mut())
        && let Some(text) = args.get("text").and_then(|t| t.as_str())
    {
        let n = text.chars().count();
        args.insert("text".into(), format!("[{n} characters]").into());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_common::geometry::Point;
    use wisp_ipc::types::{app::Empty, tools::*};

    fn click(label: &str) -> ToolCall {
        ToolCall::Click(PointerArgs {
            x: 10.0,
            y: 20.0,
            label: Some(label.into()),
        })
    }

    #[test]
    fn every_tool_has_a_definition() {
        let defs = definitions();
        for call in [
            ToolCall::ReadScreen(Empty {}),
            click("x"),
            ToolCall::Drag(DragArgs {
                from: Point { x: 0.0, y: 0.0 },
                to: Point { x: 1.0, y: 1.0 },
            }),
            ToolCall::OpenUrl(UrlArgs {
                url: "https://a.b".into(),
            }),
            ToolCall::ReadClipboard(Empty {}),
        ] {
            assert!(
                defs.iter().any(|d| d.name == name(&call)),
                "{}",
                name(&call)
            );
        }
        assert_eq!(defs.len(), 19);
        assert!(
            defs.iter()
                .filter(|d| d.risk == RiskLevel::Safe)
                .all(|d| !d.requires_permission)
        );
    }

    #[test]
    fn validates_arguments() {
        assert!(
            validate(&ToolCall::Click(PointerArgs {
                x: f64::NAN,
                y: 0.0,
                label: None
            }))
            .is_err()
        );
        assert!(validate(&ToolCall::TypeText(TextArgs { text: "".into() })).is_err());
        assert!(
            validate(&ToolCall::TypeText(TextArgs {
                text: "bell\u{7}".into()
            }))
            .is_err()
        );
        assert!(
            validate(&ToolCall::Keypress(KeyArgs {
                key: "rm -rf".into()
            }))
            .is_err()
        );
        assert!(
            validate(&ToolCall::Hotkey(HotkeyArgs {
                key: "s".into(),
                modifiers: vec![]
            }))
            .is_err()
        );
        assert!(
            validate(&ToolCall::Hotkey(HotkeyArgs {
                key: "s".into(),
                modifiers: vec![KeyModifier::Command]
            }))
            .is_ok()
        );
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://a b",
            "ftp://x",
        ] {
            assert!(
                validate(&ToolCall::OpenUrl(UrlArgs { url: bad.into() })).is_err(),
                "{bad}"
            );
        }
        for bad in ["../../bin/sh", "/Applications/Evil.app", ""] {
            assert!(
                validate(&ToolCall::OpenApplication(AppArgs { app: bad.into() })).is_err(),
                "{bad}"
            );
        }
        assert!(
            validate(&ToolCall::OpenApplication(AppArgs {
                app: "com.apple.Safari".into()
            }))
            .is_ok()
        );
    }

    #[test]
    fn raises_risk_for_destructive_or_irreversible_targets() {
        assert_eq!(assess(&click("Save")).0, RiskLevel::Low);
        assert_eq!(assess(&click("Delete")).0, RiskLevel::High);
        assert_eq!(assess(&click("Move to Trash")).0, RiskLevel::High);
        assert_eq!(assess(&click("Empty Trash")).0, RiskLevel::Critical);
        assert_eq!(assess(&click("Place order")).0, RiskLevel::High);
        // Whole words only: "Postpone" isn't "post".
        assert_eq!(assess(&click("Postpone")).0, RiskLevel::Low);
        let empty_trash = ToolCall::Hotkey(HotkeyArgs {
            key: "delete".into(),
            modifiers: vec![KeyModifier::Command, KeyModifier::Shift],
        });
        assert_eq!(assess(&empty_trash).0, RiskLevel::Critical);
        assert_eq!(
            assess(&ToolCall::Keypress(KeyArgs {
                key: "return".into()
            }))
            .0,
            RiskLevel::High
        );
        assert_eq!(assess(&ToolCall::Paste(Empty {})).0, RiskLevel::High);
        let delete_now = ToolCall::Hotkey(HotkeyArgs {
            key: "delete".into(),
            modifiers: vec![KeyModifier::Command, KeyModifier::Option],
        });
        assert_eq!(assess(&delete_now).0, RiskLevel::Critical);
        // A click with no idea what's there isn't Low.
        assert_eq!(
            assess(&ToolCall::Click(PointerArgs {
                x: 1.0,
                y: 1.0,
                label: None
            }))
            .0,
            RiskLevel::Medium
        );
        assert!(!always_grantable("typeText") && always_grantable("click"));
        let (r, why) = assess(&ToolCall::TypeText(TextArgs {
            text: "hi\n".into(),
        }));
        assert_eq!(r, RiskLevel::High);
        assert!(why.expect("why").contains("line break"));
    }

    #[test]
    fn titles_and_audit_never_carry_full_text() {
        let t = ToolCall::TypeText(TextArgs {
            text: "my secret note that is long enough to be cut off somewhere around here".into(),
        });
        assert!(title(&t).contains('…'));
        assert_eq!(redacted_args(&t)["args"]["text"], "[70 characters]");
        assert_eq!(title(&click("Save")), "Click “Save”");
    }
}
