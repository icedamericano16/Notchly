//! Per-app rules for mirrored notifications: show (default), quiet (history only, no
//! pop-up), muted (dropped) or priority (pops up through quiet hours and Focus, and stays
//! longer). Apps are matched by display name, ignoring case.

use crate::settings::Popups;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Show,
    /// Kept in the history; no pop-up.
    Quiet,
    /// Not shown or kept.
    Muted,
    /// Pops up even during quiet hours and Focus, for twice as long.
    Priority,
}

impl Rule {
    pub fn from_index(i: u8) -> Rule {
        match i {
            1 => Rule::Quiet,
            2 => Rule::Muted,
            3 => Rule::Priority,
            _ => Rule::Show,
        }
    }
    pub fn index(self) -> u8 {
        match self {
            Rule::Show => 0,
            Rule::Quiet => 1,
            Rule::Muted => 2,
            Rule::Priority => 3,
        }
    }
    pub fn describe(self) -> &'static str {
        match self {
            Rule::Show => "Shown",
            Rule::Quiet => "History only",
            Rule::Muted => "Muted",
            Rule::Priority => "Priority",
        }
    }
}

fn has(list: &[String], app: &str) -> bool {
    let app = app.trim();
    !app.is_empty() && list.iter().any(|a| a.trim().eq_ignore_ascii_case(app))
}

/// The rule for `app`. If the lists disagree (edited by hand), the most restrictive wins:
/// muted, then quiet, then priority.
pub fn rule_for(p: &Popups, app: &str) -> Rule {
    if has(&p.muted_apps, app) {
        Rule::Muted
    } else if has(&p.quiet_apps, app) {
        Rule::Quiet
    } else if has(&p.priority_apps, app) {
        Rule::Priority
    } else {
        Rule::Show
    }
}

/// Puts `app` under `rule`, taking it out of the other lists.
pub fn set_rule(p: &mut Popups, app: &str, rule: Rule) {
    let app = app.trim();
    if app.is_empty() {
        return;
    }
    for list in [&mut p.muted_apps, &mut p.quiet_apps, &mut p.priority_apps] {
        list.retain(|a| !a.trim().eq_ignore_ascii_case(app));
    }
    match rule {
        Rule::Show => {}
        Rule::Quiet => p.quiet_apps.push(app.to_string()),
        Rule::Muted => p.muted_apps.push(app.to_string()),
        Rule::Priority => p.priority_apps.push(app.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_by_app_name() {
        let mut p = Popups::default();
        assert_eq!(rule_for(&p, "Slack"), Rule::Show);
        set_rule(&mut p, "Slack", Rule::Quiet);
        set_rule(&mut p, "WhatsApp", Rule::Priority);
        set_rule(&mut p, "Spammy", Rule::Muted);
        assert_eq!(rule_for(&p, "slack"), Rule::Quiet);
        assert_eq!(rule_for(&p, " WHATSAPP "), Rule::Priority);
        assert_eq!(rule_for(&p, "Spammy"), Rule::Muted);
        assert_eq!(rule_for(&p, ""), Rule::Show);
    }

    #[test]
    fn setting_a_rule_moves_the_app() {
        let mut p = Popups::default();
        set_rule(&mut p, "Teams", Rule::Muted);
        set_rule(&mut p, "teams", Rule::Priority);
        assert!(p.muted_apps.is_empty());
        assert_eq!(p.priority_apps, vec!["teams".to_string()]);
        set_rule(&mut p, "Teams", Rule::Show);
        assert!(p.priority_apps.is_empty());
    }

    #[test]
    fn hand_edited_conflicts_pick_the_strictest() {
        let mut p = Popups::default();
        p.priority_apps.push("Mail".into());
        p.quiet_apps.push("Mail".into());
        assert_eq!(rule_for(&p, "Mail"), Rule::Quiet);
        p.muted_apps.push("mail".into());
        assert_eq!(rule_for(&p, "Mail"), Rule::Muted);
    }

    #[test]
    fn indices_round_trip() {
        for r in [Rule::Show, Rule::Quiet, Rule::Muted, Rule::Priority] {
            assert_eq!(Rule::from_index(r.index()), r);
        }
    }
}
