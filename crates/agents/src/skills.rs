//! SKILL.md (spec §64): reusable expertise as plain text — instructions, workflows, reference
//! information, examples. Skills are never executed; their text is offered to the model when
//! the user's request matches and the skill is enabled.
//!
//! ```text
//! ---
//! name: Mac keyboard shortcuts
//! description: Teach and use the common macOS shortcuts.
//! activation: auto            # auto (match triggers) | always | manual (agents only)
//! triggers: [shortcut, keyboard, hotkey]
//! tools: [hotkey, keypress]   # informational: tools the skill's workflows use
//! ---
//! # Instructions …
//! ```

pub const MAX_SKILL_BYTES: usize = 50_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    Auto,
    Always,
    Manual,
}

impl Activation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSkill {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub activation: Activation,
    pub triggers: Vec<String>,
    pub tools: Vec<String>,
    /// Markdown after the front matter.
    pub body: String,
}

/// "Mac keyboard shortcuts!" → "mac-keyboard-shortcuts".
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').chars().take(60).collect()
}

fn list(v: &str) -> Vec<String> {
    v.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches(['"', '\'']).trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn parse(content: &str) -> Result<ParsedSkill, String> {
    if content.len() > MAX_SKILL_BYTES {
        return Err(format!(
            "A skill can be at most {} KB.",
            MAX_SKILL_BYTES / 1000
        ));
    }
    if content
        .chars()
        .any(|c| c == '\0' || (c.is_control() && !matches!(c, '\n' | '\r' | '\t')))
    {
        return Err("This doesn't look like a text SKILL.md file.".into());
    }
    let text = content.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let rest = text
        .strip_prefix("---\n")
        .ok_or("SKILL.md must start with front matter (--- name: … ---).")?;
    let (front, body) = rest
        .split_once("\n---")
        .ok_or("The front matter isn't closed with ---.")?;
    let body = body.trim_start_matches('-').trim().to_owned();

    let (mut name, mut description, mut activation, mut triggers, mut tools) =
        (None, String::new(), Activation::Auto, vec![], vec![]);
    let mut current_list: Option<&str> = None;
    for raw in front.lines() {
        let line = raw.split(" #").next().unwrap_or(raw);
        if line.trim().is_empty() {
            continue;
        }
        if let Some(item) = line.trim().strip_prefix("- ") {
            match current_list {
                Some("triggers") => triggers.push(item.trim().trim_matches(['"', '\'']).to_owned()),
                Some("tools") => tools.push(item.trim().trim_matches(['"', '\'']).to_owned()),
                _ => return Err(format!("Unexpected list item: {}", item.trim())),
            }
            continue;
        }
        let (k, v) = line.split_once(':').ok_or_else(|| {
            format!(
                "Front matter lines look like `key: value` (got {:?}).",
                line.trim()
            )
        })?;
        let (k, v) = (k.trim(), v.trim());
        current_list = None;
        match k {
            "name" => name = Some(v.trim_matches(['"', '\'']).to_owned()),
            "description" => description = v.trim_matches(['"', '\'']).to_owned(),
            "activation" => {
                activation = match v {
                    "auto" | "" => Activation::Auto,
                    "always" => Activation::Always,
                    "manual" => Activation::Manual,
                    other => {
                        return Err(format!(
                            "activation must be auto, always or manual (got {other:?})."
                        ));
                    }
                }
            }
            "triggers" if v.is_empty() => current_list = Some("triggers"),
            "triggers" => triggers = list(v),
            "tools" if v.is_empty() => current_list = Some("tools"),
            "tools" => tools = list(v),
            // Unknown keys are kept in the file but ignored, so skills from newer versions still load.
            _ => {}
        }
    }
    let name = name
        .filter(|n| !n.trim().is_empty())
        .ok_or("The front matter needs a name.")?;
    if name.chars().count() > 60 {
        return Err("A skill name can be at most 60 characters.".into());
    }
    if description.chars().count() > 300 {
        return Err("A description can be at most 300 characters.".into());
    }
    if body.is_empty() {
        return Err("The skill has no instructions after the front matter.".into());
    }
    if triggers.len() > 30 || triggers.iter().any(|t| t.chars().count() > 40) {
        return Err("Use at most 30 short triggers.".into());
    }
    let slug = slugify(&name);
    if slug.is_empty() {
        return Err("The name needs some letters or numbers.".into());
    }
    Ok(ParsedSkill {
        name,
        slug,
        description,
        activation,
        triggers,
        tools,
        body,
    })
}

/// Rebuild a SKILL.md from fields (for new skills made in the app).
pub fn render(
    name: &str,
    description: &str,
    activation: Activation,
    triggers: &[String],
    body: &str,
) -> String {
    let mut s = format!(
        "---\nname: {name}\ndescription: {description}\nactivation: {}\n",
        activation.as_str()
    );
    if !triggers.is_empty() {
        s.push_str(&format!("triggers: [{}]\n", triggers.join(", ")));
    }
    s.push_str("---\n\n");
    s.push_str(body.trim());
    s.push('\n');
    s
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(str::to_owned)
        .collect()
}

/// How relevant a skill is to `request` (0 = not at all). Triggers count most; then the name.
pub fn relevance(skill: &ParsedSkill, request: &str) -> u32 {
    match skill.activation {
        Activation::Always => return 100,
        Activation::Manual => return 0,
        Activation::Auto => {}
    }
    let req = format!(" {} ", words(request).join(" "));
    let mut score = 0;
    for t in &skill.triggers {
        let t = words(t).join(" ");
        if !t.is_empty() && req.contains(&format!(" {t}")) {
            score += 10;
        }
    }
    for w in words(&skill.name) {
        if req.contains(&format!(" {w}")) {
            score += 3;
        }
    }
    score
}

pub const SAMPLE_SKILL: &str = r#"---
name: Mac keyboard shortcuts
description: Suggest and teach the standard macOS keyboard shortcuts.
activation: auto
triggers: [shortcut, shortcuts, keyboard, hotkey, faster, quickly]
tools: [hotkey, keypress]
---

# Instructions
When the user wants to do something faster, or asks about shortcuts, suggest the standard macOS
shortcut and write it with symbols, for example ⌘C. Mention the menu where it appears so they can
find it again. Teach one or two shortcuts at a time, not a list.

# Reference
- Copy ⌘C, Paste ⌘V, Cut ⌘X, Undo ⌘Z, Redo ⇧⌘Z, Select all ⌘A
- Find ⌘F, Save ⌘S, Print ⌘P, New ⌘N, Open ⌘O, Close window ⌘W, Quit app ⌘Q
- Switch apps ⌘Tab, Spotlight ⌘Space, Screenshot area ⇧⌘4, Screenshot tools ⇧⌘5
- Force quit ⌥⌘Esc, Hide app ⌘H, Minimize ⌘M, Settings for the current app ⌘,
- Move to Trash ⌘⌫ (Finder), Show hidden files ⇧⌘. (Finder)

# Examples
User: How do I copy this faster? → "Select it and press ⌘C — it's the same as Edit → Copy."
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_sample_and_lists_in_both_styles() {
        let s = parse(SAMPLE_SKILL).expect("sample");
        assert_eq!(
            (s.name.as_str(), s.slug.as_str()),
            ("Mac keyboard shortcuts", "mac-keyboard-shortcuts")
        );
        assert_eq!(s.activation, Activation::Auto);
        assert!(s.triggers.contains(&"hotkey".to_owned()));
        assert!(s.body.starts_with("# Instructions"));
        let multi =
            parse("---\nname: X\ntriggers:\n  - docker\n  - \"compose\"\n---\nUse compose.")
                .expect("multi");
        assert_eq!(multi.triggers, vec!["docker", "compose"]);
    }

    #[test]
    fn rejects_bad_files_with_reasons() {
        for (bad, why) in [
            ("no front matter", "front matter"),
            ("---\nname: X\nno close", "isn't closed"),
            ("---\ndescription: d\n---\nbody", "needs a name"),
            (
                "---\nname: X\nactivation: sometimes\n---\nbody",
                "activation",
            ),
            ("---\nname: X\n---\n", "no instructions"),
            ("---\nname: X\n---\nbinary\u{0}", "text"),
        ] {
            let e = parse(bad).expect_err(bad);
            assert!(e.contains(why), "{bad:?} → {e}");
        }
        assert!(
            parse(&format!(
                "---\nname: X\n---\n{}",
                "a".repeat(MAX_SKILL_BYTES)
            ))
            .is_err()
        );
    }

    #[test]
    fn matches_requests_to_skills() {
        let s = parse(SAMPLE_SKILL).expect("sample");
        assert!(relevance(&s, "Is there a keyboard shortcut for this?") >= 10);
        assert_eq!(relevance(&s, "What's the weather?"), 0);
        let always = parse("---\nname: Tone\nactivation: always\n---\nBe brief.").expect("p");
        assert_eq!(relevance(&always, "anything"), 100);
        let manual =
            parse("---\nname: Docker\nactivation: manual\ntriggers: [docker]\n---\nx").expect("p");
        assert_eq!(relevance(&manual, "docker please"), 0);
    }

    #[test]
    fn renders_round_trip() {
        let md = render(
            "Notes helper",
            "Write tidy notes",
            Activation::Always,
            &["note".into()],
            "Use bullet points.",
        );
        let p = parse(&md).expect("round trip");
        assert_eq!(
            (p.name.as_str(), p.activation, p.body.as_str()),
            ("Notes helper", Activation::Always, "Use bullet points.")
        );
        assert_eq!(slugify("  Hello, World!! "), "hello-world");
    }
}
