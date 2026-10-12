//! Agent runtime primitives (spec §60–62): the action language agents speak, plan parsing,
//! translating actions into tool calls against what's on screen, loop detection and limits.
//!
//! Small local models are much more reliable with a tiny line-based command language than with
//! JSON tool calling, so an agent replies with:
//!
//! ```text
//! THOUGHT: The Save button is at the bottom right.
//! STEP: 2
//! ACTION: click "Save"
//! ```
//!
//! Pure — the core runs the loop and every action goes through the tool pipeline.

pub mod schedule;
pub mod skills;

use wisp_common::geometry::Point;
use wisp_ipc::types::{
    app::Empty,
    focus::FocusRegion,
    tools::{
        AppArgs, DayArgs, EventArgs, HotkeyArgs, KeyArgs, KeyModifier, MoveArgs, MusicArgs,
        MusicCommand, NoteArgs, PathArgs, PointerArgs, ReminderArgs, ScrollArgs, TextArgs,
        ToolCall, UrlArgs,
    },
};
use wisp_screen::guide::{self, TargetRef};

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Label(String),
    /// 0–1000 relative to the observed window.
    Coordinates(f64, f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Click {
        target: Target,
        count: u8,
        right: bool,
    },
    Type(String),
    Press(String),
    Hotkey {
        key: String,
        modifiers: Vec<KeyModifier>,
    },
    Scroll {
        down: bool,
        amount: i32,
    },
    OpenApp(String),
    SwitchApp(String),
    OpenUrl(String),
    Copy,
    Paste,
    Wait(u32),
    Done(String),
    Ask(String),
    /// Save something worth knowing next time (agent memory).
    Remember(String),
    /// A background tool (reminders, calendar, notes, music, files, web pages).
    Background(ToolCall),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub thought: Option<String>,
    /// 1-based plan item being worked on.
    pub plan_step: Option<usize>,
    pub action: Action,
}

/// The command reference included in the agent prompt.
pub const ACTION_HELP: &str = r#"click "label"            click the control with this label
double_click "label"     double-click it
right_click "label"      right-click it
click 412,230            click at x,y from 0 to 1000 across the window image (only if there is no label)
type "text"              type into the focused field
press return             press a key: return, tab, escape, space, delete, up, down, left, right
hotkey cmd+s             press a shortcut (cmd, option, ctrl, shift)
scroll down 5            scroll the window (up or down, lines)
open_app "Notes"         open an application
switch_app "Safari"      bring a running application to the front
open_url "https://…"     open a web address
copy / paste             copy the selection / paste
wait 2                   wait seconds for something to load
done "summary"           the goal is complete; say what you did
ask "question"           you need the user to answer something
remember "fact"          save something useful for next time (not secrets)"#;

/// Commands for background work: through apps and files directly, never the screen.
pub const BACKGROUND_ACTION_HELP: &str = r#"remind "title" at 2026-10-13 09:00   add a reminder (the time is optional)
reminders                            list open reminders
event "title" at 2026-10-13 15:00 for 30   add a calendar event lasting 30 minutes
events 2026-10-13                    list a day's events (today and tomorrow work too)
note "title" "text"                  write a note in Notes
music play                           music: play, pause, next or previous
files "~/Desktop"                    list a folder in the home folder
move "~/Desktop/a.png" to "~/Pictures/Screenshots"   move into a folder (created if missing)
trash "~/Desktop/old.zip"            move to the Trash (the user is asked first)
read_page "https://…"                read a web page's text
wait 2                               wait seconds
done "summary"                       the goal is complete; say briefly what you did
ask "question"                       you need the user to answer something
remember "fact"                      save something useful for next time (not secrets)"#;

/// "Monday, 2026-10-12 18:40" in local time, for prompts that need to know when "tomorrow" is.
pub fn now_for_prompt() -> String {
    chrono::Local::now()
        .format("%A, %Y-%m-%d %H:%M")
        .to_string()
}

/// Every "quoted" string in order, and the words outside quotes.
fn quoted(s: &str) -> (Vec<String>, Vec<String>) {
    let (mut q, mut words, mut cur, mut inside) = (vec![], vec![], String::new(), None::<char>);
    for c in s.chars() {
        match inside {
            Some(close) if c == close => {
                q.push(std::mem::take(&mut cur));
                inside = None;
            }
            Some(_) => cur.push(c),
            None if c == '"' || c == '“' => {
                if !cur.trim().is_empty() {
                    words.extend(cur.split_whitespace().map(str::to_owned));
                }
                cur.clear();
                inside = Some(if c == '“' { '”' } else { '"' });
            }
            None => cur.push(c),
        }
    }
    words.extend(cur.split_whitespace().map(str::to_owned));
    (q, words)
}

/// "2026-10-13" / "today" / "tomorrow" → "YYYY-MM-DD".
fn day(w: &str) -> Option<String> {
    let today = chrono::Local::now().date_naive();
    match w.to_lowercase().as_str() {
        "today" => Some(today.format("%Y-%m-%d").to_string()),
        "tomorrow" => Some(
            (today + chrono::Days::new(1))
                .format("%Y-%m-%d")
                .to_string(),
        ),
        other => chrono::NaiveDate::parse_from_str(other, "%Y-%m-%d")
            .ok()
            .map(|d| d.format("%Y-%m-%d").to_string()),
    }
}

/// Words after "at": a day and a time ("9:00", "09:00", "9am", "3:30pm") → "YYYY-MM-DD HH:MM".
fn when(words: &[String]) -> Option<String> {
    let i = words.iter().position(|w| w.eq_ignore_ascii_case("at"))?;
    let d = day(words.get(i + 1)?)?;
    let t = words.get(i + 2)?.to_lowercase();
    let (t, pm) = match t.strip_suffix("pm") {
        Some(t) => (t.to_owned(), true),
        None => (t.trim_end_matches("am").to_owned(), false),
    };
    let (h, m) = t.split_once(':').unwrap_or((&t, "00"));
    let (mut h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
    if pm && h < 12 {
        h += 12;
    }
    (h < 24 && m < 60).then(|| format!("{d} {h:02}:{m:02}"))
}

fn unquote(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .or_else(|| s.strip_prefix('“').and_then(|r| r.strip_suffix('”')))
        .or_else(|| s.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')))
        .unwrap_or(s);
    let inner = inner.replace("\\n", "\n").replace("\\\"", "\"");
    (!inner.trim().is_empty()).then_some(inner)
}

fn coords(s: &str) -> Option<(f64, f64)> {
    let mut it = s.trim().trim_matches(|c| c == '(' || c == ')').split(',');
    let x = it.next()?.trim().parse::<f64>().ok()?;
    let y = it.next()?.trim().parse::<f64>().ok()?;
    (it.next().is_none() && (0.0..=1000.0).contains(&x) && (0.0..=1000.0).contains(&y))
        .then_some((x, y))
}

fn modifier(m: &str) -> Option<KeyModifier> {
    Some(match m {
        "cmd" | "command" | "⌘" => KeyModifier::Command,
        "option" | "opt" | "alt" | "⌥" => KeyModifier::Option,
        "ctrl" | "control" | "⌃" => KeyModifier::Control,
        "shift" | "⇧" => KeyModifier::Shift,
        _ => return None,
    })
}

const KEYS: &[&str] = &[
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
];

fn key_name(k: &str) -> Option<String> {
    let k = k.trim().to_lowercase();
    let k = match k.as_str() {
        "enter" => "return".to_owned(),
        "esc" => "escape".to_owned(),
        "backspace" => "delete".to_owned(),
        other => other.to_owned(),
    };
    KEYS.iter()
        .find(|n| n.to_lowercase() == k)
        .map(|n| (*n).to_owned())
}

/// Parse one action line (without the `ACTION:` prefix).
pub fn parse_action(line: &str) -> Result<Action, String> {
    let line = line
        .trim()
        .trim_start_matches('`')
        .trim_end_matches('`')
        .trim();
    let (verb, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim();
    let verb = verb.to_lowercase().replace('-', "_");
    let need = |what: &str| format!("{verb} needs {what}");
    let target = |r: &str| -> Result<Target, String> {
        if let Some((x, y)) = coords(r) {
            return Ok(Target::Coordinates(x, y));
        }
        unquote(r)
            .map(Target::Label)
            .ok_or_else(|| need("a label in quotes"))
    };
    Ok(match verb.as_str() {
        "click" => Action::Click {
            target: target(rest)?,
            count: 1,
            right: false,
        },
        "double_click" | "doubleclick" => Action::Click {
            target: target(rest)?,
            count: 2,
            right: false,
        },
        "right_click" | "rightclick" => Action::Click {
            target: target(rest)?,
            count: 1,
            right: true,
        },
        "type" => Action::Type(unquote(rest).ok_or_else(|| need("text in quotes"))?),
        "press" => Action::Press(key_name(rest).ok_or_else(|| format!("unknown key {rest:?}"))?),
        "hotkey" | "shortcut" => {
            let parts: Vec<String> = rest.split('+').map(|p| p.trim().to_lowercase()).collect();
            let (key, mods) = parts.split_last().ok_or_else(|| need("keys like cmd+s"))?;
            let modifiers = mods
                .iter()
                .map(|m| modifier(m).ok_or_else(|| format!("unknown modifier {m:?}")))
                .collect::<Result<Vec<_>, _>>()?;
            if modifiers.is_empty() {
                return Err("a shortcut needs a modifier, like cmd+s".into());
            }
            let key = key_name(key).unwrap_or_else(|| key.clone());
            if key.chars().count() != 1 && !KEYS.contains(&key.as_str()) {
                return Err(format!("unknown key {key:?}"));
            }
            Action::Hotkey { key, modifiers }
        }
        "scroll" => {
            let mut it = rest.split_whitespace();
            let down = match it.next().map(str::to_lowercase).as_deref() {
                Some("down") => true,
                Some("up") => false,
                _ => return Err(need("up or down")),
            };
            let amount = it
                .next()
                .and_then(|n| n.parse::<i32>().ok())
                .unwrap_or(5)
                .clamp(1, 30);
            Action::Scroll { down, amount }
        }
        "open_app" | "open" | "launch" => {
            Action::OpenApp(unquote(rest).ok_or_else(|| need("an app name"))?)
        }
        "switch_app" | "switch" => {
            Action::SwitchApp(unquote(rest).ok_or_else(|| need("an app name"))?)
        }
        "open_url" | "go" => Action::OpenUrl(unquote(rest).ok_or_else(|| need("an address"))?),
        "copy" => Action::Copy,
        "paste" => Action::Paste,
        "wait" => Action::Wait(rest.parse::<u32>().unwrap_or(2).clamp(1, 10)),
        "done" | "finish" => Action::Done(unquote(rest).unwrap_or_else(|| "Done.".into())),
        "ask" => Action::Ask(unquote(rest).ok_or_else(|| need("a question"))?),
        "remember" => Action::Remember(unquote(rest).ok_or_else(|| need("what to remember"))?),
        "remind" | "reminder" => {
            let (q, words) = quoted(rest);
            let title = q
                .first()
                .cloned()
                .ok_or_else(|| need("a title in quotes"))?;
            let due = if words.iter().any(|w| w.eq_ignore_ascii_case("at")) {
                Some(when(&words).ok_or_else(|| need("a time like at 2026-10-13 09:00"))?)
            } else {
                None
            };
            Action::Background(ToolCall::CreateReminder(ReminderArgs { title, due }))
        }
        "reminders" => Action::Background(ToolCall::ListReminders(Empty {})),
        "event" => {
            let (q, words) = quoted(rest);
            let title = q
                .first()
                .cloned()
                .ok_or_else(|| need("a title in quotes"))?;
            let start = when(&words).ok_or_else(|| need("a start like at 2026-10-13 15:00"))?;
            let minutes = words
                .iter()
                .position(|w| w.eq_ignore_ascii_case("for"))
                .and_then(|i| words.get(i + 1))
                .and_then(|m| m.trim_end_matches("min").parse::<u32>().ok())
                .unwrap_or(30)
                .clamp(5, 1440);
            Action::Background(ToolCall::CreateEvent(EventArgs {
                title,
                start,
                minutes,
            }))
        }
        "events" | "calendar" => Action::Background(ToolCall::ListEvents(DayArgs {
            day: day(if rest.is_empty() { "today" } else { rest })
                .ok_or_else(|| need("a day like 2026-10-13"))?,
        })),
        "note" => {
            let (q, _) = quoted(rest);
            let mut q = q.into_iter();
            let title = q.next().ok_or_else(|| need("a title in quotes"))?;
            Action::Background(ToolCall::CreateNote(NoteArgs {
                title,
                body: q.next().unwrap_or_default(),
            }))
        }
        "music" => Action::Background(ToolCall::Music(MusicArgs {
            command: match rest.to_lowercase().as_str() {
                "play" | "resume" => MusicCommand::Play,
                "pause" | "stop" => MusicCommand::Pause,
                "next" | "skip" => MusicCommand::Next,
                "previous" | "back" => MusicCommand::Previous,
                _ => return Err(need("play, pause, next or previous")),
            },
        })),
        "files" | "list" => Action::Background(ToolCall::ListFiles(PathArgs {
            path: unquote(rest).unwrap_or_else(|| "~".into()),
        })),
        "move" => {
            let (q, _) = quoted(rest);
            match &q[..] {
                [from, to] => Action::Background(ToolCall::MoveFile(MoveArgs {
                    from: from.clone(),
                    to: to.clone(),
                })),
                _ => return Err(need(r#""from" to "folder""#)),
            }
        }
        "trash" => Action::Background(ToolCall::TrashFile(PathArgs {
            path: unquote(rest).ok_or_else(|| need("a path in quotes"))?,
        })),
        "read_page" | "fetch" | "read" => Action::Background(ToolCall::FetchPage(UrlArgs {
            url: unquote(rest).ok_or_else(|| need("an address"))?,
        })),
        other => return Err(format!("unknown action {other:?}")),
    })
}

/// Parse a full reply: optional THOUGHT and STEP lines, and exactly one ACTION.
pub fn parse_decision(reply: &str) -> Result<Decision, String> {
    let mut thought = None;
    let mut plan_step = None;
    let mut action = None;
    for line in reply.lines() {
        let l = line.trim().trim_start_matches(['*', '-', '#']).trim();
        let upper = l.to_uppercase();
        if upper.starts_with("THOUGHT:") {
            thought = Some(l[8..].trim().to_owned());
        } else if upper.starts_with("STEP:") {
            plan_step = l[5..].trim().trim_end_matches('.').parse::<usize>().ok();
        } else if upper.starts_with("ACTION:") && action.is_none() {
            action = Some(parse_action(&l[7..])?);
        }
    }
    // Lenient: a reply that is just an action line.
    let action = match action {
        Some(a) => a,
        None => reply
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .ok_or("the reply was empty")
            .and_then(|l| parse_action(l).map_err(|_| "no ACTION line"))?,
    };
    Ok(Decision {
        thought,
        plan_step,
        action,
    })
}

/// "1. Open Notes\n2. Make a note" → items. At most 8.
pub fn parse_plan(reply: &str) -> Vec<String> {
    reply
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l
                .split_once(['.', ')'])
                .filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
                .map(|(_, r)| r)
                .or_else(|| l.strip_prefix("- "))
                .or_else(|| l.strip_prefix("* "))?;
            let item = rest.trim().trim_matches('*').trim();
            (!item.is_empty()).then(|| item.chars().take(120).collect())
        })
        .take(8)
        .collect()
}

/// Turn an action into a tool call, resolving labels against the observed window.
/// `Ok(None)` for actions that aren't tools (wait, done, ask).
pub fn to_tool_call(
    action: &Action,
    screen: Option<&FocusRegion>,
) -> Result<Option<ToolCall>, String> {
    let pointer = |t: &Target| -> Result<PointerArgs, String> {
        let screen = screen.ok_or("there's no screen to click on")?;
        let (tref, label) = match t {
            Target::Label(l) => (TargetRef::Label(l.clone()), Some(l.clone())),
            Target::Coordinates(x, y) => (TargetRef::Coordinates(*x, *y), None),
        };
        let found = guide::resolve(&tref, screen).ok_or_else(|| match t {
            Target::Label(l) => format!("couldn't find “{l}” on screen"),
            Target::Coordinates(..) => "those coordinates aren't on screen".into(),
        })?;
        Ok(PointerArgs {
            x: found.rect.x + found.rect.width / 2.0,
            y: found.rect.y + found.rect.height / 2.0,
            label,
        })
    };
    Ok(Some(match action {
        Action::Click {
            target,
            count,
            right,
        } => {
            let p = pointer(target)?;
            match (count, right) {
                (_, true) => ToolCall::RightClick(p),
                (2, _) => ToolCall::DoubleClick(p),
                _ => ToolCall::Click(p),
            }
        }
        Action::Type(t) => ToolCall::TypeText(TextArgs { text: t.clone() }),
        Action::Press(k) => ToolCall::Keypress(KeyArgs { key: k.clone() }),
        Action::Hotkey { key, modifiers } => ToolCall::Hotkey(HotkeyArgs {
            key: key.clone(),
            modifiers: modifiers.clone(),
        }),
        Action::Scroll { down, amount } => {
            let s = screen.ok_or("there's no window to scroll")?;
            let c = Point {
                x: s.crop.x + s.crop.width / 2.0,
                y: s.crop.y + s.crop.height / 2.0,
            };
            ToolCall::Scroll(ScrollArgs {
                x: c.x,
                y: c.y,
                dy: if *down { *amount } else { -*amount },
                dx: 0,
            })
        }
        Action::OpenApp(a) => ToolCall::OpenApplication(AppArgs { app: a.clone() }),
        Action::SwitchApp(a) => ToolCall::SwitchApplication(AppArgs { app: a.clone() }),
        Action::OpenUrl(u) => ToolCall::OpenUrl(UrlArgs { url: u.clone() }),
        Action::Copy => ToolCall::Copy(Empty {}),
        Action::Paste => ToolCall::Paste(Empty {}),
        Action::Background(call) => call.clone(),
        Action::Wait(_) | Action::Done(_) | Action::Ask(_) | Action::Remember(_) => {
            return Ok(None);
        }
    }))
}

/// Notices an agent repeating itself (spec §61).
#[derive(Debug, Default)]
pub struct LoopDetector {
    last: Option<String>,
    repeats: u32,
}

impl LoopDetector {
    /// Record an action; returns true when the same action has now happened `limit` times in a row.
    pub fn record(&mut self, action: &Action, limit: u32) -> bool {
        let key = format!("{action:?}").to_lowercase();
        if self.last.as_deref() == Some(key.as_str()) {
            self.repeats += 1;
        } else {
            self.last = Some(key);
            self.repeats = 1;
        }
        self.repeats >= limit
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_common::geometry::Rect;
    use wisp_ipc::types::{
        accessibility::{AxContext, AxNode},
        focus::{DisplayInfo, FocusKind},
    };

    fn screen() -> FocusRegion {
        FocusRegion {
            id: wisp_common::ids::new_id(),
            kind: FocusKind::Window,
            display: DisplayInfo {
                id: 1,
                name: "d".into(),
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
                nodes: vec![AxNode {
                    role: "AXButton".into(),
                    subrole: None,
                    title: Some("Save".into()),
                    description: None,
                    value: None,
                    identifier: None,
                    frame: Some(Rect::new(560.0, 440.0, 80.0, 28.0)),
                    enabled: true,
                    focused: false,
                    selected: false,
                    actions: vec!["AXPress".into()],
                    depth: 2,
                }],
                summary: String::new(),
            }),
            text: None,
            created_at_ms: 0,
        }
    }

    #[test]
    fn parses_the_action_language() {
        assert_eq!(
            parse_action(r#"click "Save""#),
            Ok(Action::Click {
                target: Target::Label("Save".into()),
                count: 1,
                right: false
            })
        );
        assert_eq!(
            parse_action("double-click “Photo 1”"),
            Ok(Action::Click {
                target: Target::Label("Photo 1".into()),
                count: 2,
                right: false
            })
        );
        assert_eq!(
            parse_action("click 412, 230"),
            Ok(Action::Click {
                target: Target::Coordinates(412.0, 230.0),
                count: 1,
                right: false
            })
        );
        assert_eq!(
            parse_action(r#"type "Hello\nWorld""#),
            Ok(Action::Type("Hello\nWorld".into()))
        );
        assert_eq!(
            parse_action("press Enter"),
            Ok(Action::Press("return".into()))
        );
        assert_eq!(
            parse_action("hotkey cmd+shift+n"),
            Ok(Action::Hotkey {
                key: "n".into(),
                modifiers: vec![KeyModifier::Command, KeyModifier::Shift]
            })
        );
        assert_eq!(
            parse_action("scroll down 3"),
            Ok(Action::Scroll {
                down: true,
                amount: 3
            })
        );
        assert_eq!(
            parse_action(r#"open_app "Notes""#),
            Ok(Action::OpenApp("Notes".into()))
        );
        assert_eq!(
            parse_action(r#"done "Saved the file""#),
            Ok(Action::Done("Saved the file".into()))
        );
        assert!(parse_action("hotkey s").is_err());
        assert!(parse_action("rm -rf /").is_err());
        assert!(parse_action("click").is_err());
    }

    #[test]
    fn parses_decisions_and_plans() {
        let d = parse_decision(
            "THOUGHT: Save is visible.\nSTEP: 2\nACTION: click \"Save\"\nACTION: click \"Other\"",
        )
        .expect("parse");
        assert_eq!(d.thought.as_deref(), Some("Save is visible."));
        assert_eq!(d.plan_step, Some(2));
        assert!(matches!(d.action, Action::Click { .. }));
        assert_eq!(
            parse_decision("done \"ok\"").expect("lenient").action,
            Action::Done("ok".into())
        );
        assert!(parse_decision("I think we should click Save").is_err());
        assert_eq!(
            parse_plan(
                "Here's the plan:\n1. Open Notes\n2) Create a note\n- Type the text\n\nThanks"
            ),
            vec!["Open Notes", "Create a note", "Type the text"]
        );
    }

    #[test]
    fn resolves_actions_to_tool_calls_on_screen() {
        let s = screen();
        let call = to_tool_call(&parse_action(r#"click "save""#).expect("p"), Some(&s))
            .expect("ok")
            .expect("tool");
        assert_eq!(
            call,
            ToolCall::Click(PointerArgs {
                x: 600.0,
                y: 454.0,
                label: Some("save".into())
            })
        );
        let err = to_tool_call(&parse_action(r#"click "Print""#).expect("p"), Some(&s))
            .expect_err("missing");
        assert!(err.contains("couldn't find “Print”"));
        assert_eq!(to_tool_call(&Action::Wait(2), Some(&s)), Ok(None));
        let scroll = to_tool_call(
            &Action::Scroll {
                down: false,
                amount: 4,
            },
            Some(&s),
        )
        .expect("ok")
        .expect("tool");
        assert_eq!(
            scroll,
            ToolCall::Scroll(ScrollArgs {
                x: 400.0,
                y: 300.0,
                dy: -4,
                dx: 0
            })
        );
    }

    #[test]
    fn notices_repetition() {
        let mut d = LoopDetector::default();
        let a = Action::Press("tab".into());
        assert!(!d.record(&a, 3));
        assert!(!d.record(&a, 3));
        assert!(d.record(&a, 3));
        assert!(!d.record(&Action::Copy, 3));
    }

    #[test]
    fn parses_background_actions() {
        let today = chrono::Local::now().date_naive();
        let tomorrow = (today + chrono::Days::new(1))
            .format("%Y-%m-%d")
            .to_string();
        assert_eq!(
            parse_action(r#"remind "Call Sam" at tomorrow 9:00"#).expect("remind"),
            Action::Background(ToolCall::CreateReminder(ReminderArgs {
                title: "Call Sam".into(),
                due: Some(format!("{tomorrow} 09:00")),
            }))
        );
        assert_eq!(
            parse_action(r#"remind "Buy milk""#).expect("no time"),
            Action::Background(ToolCall::CreateReminder(ReminderArgs {
                title: "Buy milk".into(),
                due: None,
            }))
        );
        assert_eq!(
            parse_action(r#"event "Standup" at 2026-10-13 3:30pm for 15"#).expect("event"),
            Action::Background(ToolCall::CreateEvent(EventArgs {
                title: "Standup".into(),
                start: "2026-10-13 15:30".into(),
                minutes: 15,
            }))
        );
        assert_eq!(
            parse_action(r#"note "Ideas" "One\nTwo""#).expect("note"),
            Action::Background(ToolCall::CreateNote(NoteArgs {
                title: "Ideas".into(),
                body: "One\\nTwo".into(),
            }))
        );
        assert_eq!(
            parse_action(r#"move "~/Desktop/a.png" to "~/Pictures/Shots""#).expect("move"),
            Action::Background(ToolCall::MoveFile(MoveArgs {
                from: "~/Desktop/a.png".into(),
                to: "~/Pictures/Shots".into(),
            }))
        );
        assert!(matches!(
            parse_action("events today").expect("events"),
            Action::Background(ToolCall::ListEvents(DayArgs { day })) if day == today.format("%Y-%m-%d").to_string()
        ));
        assert!(matches!(
            parse_action("music next"),
            Ok(Action::Background(ToolCall::Music(_)))
        ));
        assert!(parse_action(r#"remind "x" at someday"#).is_err());
        assert!(parse_action("music louder").is_err());
        // Background actions become tool calls without any screen.
        let a = parse_action(r#"read_page "https://example.com""#).expect("read");
        assert!(matches!(
            to_tool_call(&a, None),
            Ok(Some(ToolCall::FetchPage(_)))
        ));
    }
}
