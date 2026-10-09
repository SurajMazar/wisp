//! Central permission policy (spec §57–58, §104): decides whether an action may run, must be
//! approved, or is refused. Pure, so every rule is unit-tested.
//!
//! * Computer control is off until the user turns it on.
//! * Safe (read-only) tools run without asking.
//! * Low/medium tools ask, unless allowed for the session or always (and "confirm every action"
//!   is off). A standing "never" refuses.
//! * High-risk actions ask every time and can only be allowed once.
//! * Critical actions ask every time, can only be allowed once, and say why.
//! * Nothing escalates silently: grants are per tool and never cover a riskier call.

use wisp_ipc::types::tools::{ApprovalDecision, RiskLevel};

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Run now; the string says why (for the audit log).
    Allow(&'static str),
    /// Ask the user, offering these choices.
    Ask(Vec<ApprovalDecision>),
    Deny(String),
}

#[derive(Debug, Clone, Default)]
pub struct Grants<'a> {
    pub always: &'a [String],
    pub never: &'a [String],
    pub session: &'a [String],
}

pub struct Context<'a> {
    pub enabled: bool,
    /// Whether "Always allow" may be offered/used for this tool.
    pub always_grantable: bool,
    pub confirm_every_action: bool,
    pub tool: &'a str,
    pub risk: RiskLevel,
    pub grants: Grants<'a>,
}

pub fn evaluate(c: &Context<'_>) -> Outcome {
    if c.risk == RiskLevel::Safe {
        return Outcome::Allow("autoAllow");
    }
    if !c.enabled {
        return Outcome::Deny(
            "Computer control is off. Turn it on in Settings → Computer Control to let Wisp act for you."
                .into(),
        );
    }
    let has = |list: &[String]| list.iter().any(|t| t == c.tool);
    if has(c.grants.never) {
        return Outcome::Deny("You chose never to allow this action.".into());
    }
    match c.risk {
        RiskLevel::Critical | RiskLevel::High => {
            Outcome::Ask(vec![ApprovalDecision::AllowOnce, ApprovalDecision::Deny])
        }
        _ => {
            if !c.confirm_every_action {
                if c.always_grantable && has(c.grants.always) {
                    return Outcome::Allow("granted");
                }
                if has(c.grants.session) {
                    return Outcome::Allow("allowSession");
                }
            }
            let mut choices = vec![ApprovalDecision::AllowOnce, ApprovalDecision::AllowSession];
            if c.always_grantable {
                choices.push(ApprovalDecision::AlwaysAllow);
            }
            choices.push(ApprovalDecision::Deny);
            Outcome::Ask(choices)
        }
    }
}

/// A user's answer may only pick one of the offered choices.
pub fn accept(offered: &[ApprovalDecision], chosen: ApprovalDecision) -> ApprovalDecision {
    if offered.contains(&chosen) {
        chosen
    } else {
        ApprovalDecision::Deny
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ApprovalDecision::*;

    fn ctx<'a>(tool: &'a str, risk: RiskLevel, grants: Grants<'a>) -> Context<'a> {
        Context {
            enabled: true,
            always_grantable: true,
            confirm_every_action: false,
            tool,
            risk,
            grants,
        }
    }

    #[test]
    fn safe_tools_run_even_when_control_is_off() {
        let mut c = ctx("readScreen", RiskLevel::Safe, Grants::default());
        c.enabled = false;
        assert_eq!(evaluate(&c), Outcome::Allow("autoAllow"));
        let mut c = ctx("click", RiskLevel::Low, Grants::default());
        c.enabled = false;
        assert!(matches!(evaluate(&c), Outcome::Deny(m) if m.contains("Computer control is off")));
    }

    #[test]
    fn grants_cover_low_and_medium_but_never_high_or_critical() {
        let always = vec!["click".to_string()];
        let g = || Grants {
            always: &always,
            ..Default::default()
        };
        assert_eq!(
            evaluate(&ctx("click", RiskLevel::Low, g())),
            Outcome::Allow("granted")
        );
        // Same tool, riskier target ("Delete"): asks, and only once is offered.
        assert_eq!(
            evaluate(&ctx("click", RiskLevel::High, g())),
            Outcome::Ask(vec![AllowOnce, Deny])
        );
        assert_eq!(
            evaluate(&ctx("click", RiskLevel::Critical, g())),
            Outcome::Ask(vec![AllowOnce, Deny])
        );
        let session = vec!["typeText".to_string()];
        let s = Grants {
            session: &session,
            ..Default::default()
        };
        assert_eq!(
            evaluate(&ctx("typeText", RiskLevel::Medium, s)),
            Outcome::Allow("allowSession")
        );
    }

    #[test]
    fn confirm_every_action_ignores_grants_and_never_refuses() {
        let always = vec!["click".to_string()];
        let mut c = ctx(
            "click",
            RiskLevel::Low,
            Grants {
                always: &always,
                ..Default::default()
            },
        );
        c.confirm_every_action = true;
        assert!(matches!(evaluate(&c), Outcome::Ask(_)));
        let never = vec!["paste".to_string()];
        assert!(matches!(
            evaluate(&ctx(
                "paste",
                RiskLevel::Medium,
                Grants {
                    never: &never,
                    ..Default::default()
                }
            )),
            Outcome::Deny(_)
        ));
    }

    #[test]
    fn content_driven_tools_are_never_always_allowed() {
        let always = vec!["typeText".to_string()];
        let mut c = ctx(
            "typeText",
            RiskLevel::Medium,
            Grants {
                always: &always,
                ..Default::default()
            },
        );
        c.always_grantable = false;
        assert_eq!(
            evaluate(&c),
            Outcome::Ask(vec![AllowOnce, AllowSession, Deny])
        );
    }

    #[test]
    fn answers_outside_the_offer_are_denied() {
        assert_eq!(accept(&[AllowOnce, Deny], AlwaysAllow), Deny);
        assert_eq!(accept(&[AllowOnce, Deny], AllowOnce), AllowOnce);
    }
}
