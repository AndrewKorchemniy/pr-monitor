//! WCAG contrast rules every theme is checked against. Built-in themes must pass all of them.

use crate::{
    contrast::contrast_ratio,
    issue::{IssueLevel, ThemeId, ThemeIssue},
    palette::Palette,
};

/// `fg` must reach `min` on every key in `bg`.
pub struct ContrastRule {
    pub fg: &'static str,
    pub bg: &'static [&'static str],
    pub min: f64,
}

/// Below `min` but within this much is a Warning; further below is an Error.
const WARNING_MARGIN: f64 = 1.5;

const fn rule(fg: &'static str, bg: &'static [&'static str], min: f64) -> ContrastRule {
    ContrastRule { fg, bg, min }
}

const PRIMARY_BACKGROUNDS: &[&str] = &[
    "surface.canvas",
    "surface.sunken",
    "surface.raised",
    "surface.overlay",
    "row.selected",
    "row.hover",
];
const SECONDARY_BACKGROUNDS: &[&str] = &[
    "surface.canvas",
    "surface.sunken",
    "surface.raised",
    "row.selected",
    "row.hover",
];
const TABLE: &[&str] = &["surface.sunken", "row.selected"];
const SUNKEN: &[&str] = &["surface.sunken"];

pub const CONTRAST_RULES: &[ContrastRule] = &[
    rule("text.primary", PRIMARY_BACKGROUNDS, 7.0),
    rule("text.secondary", SECONDARY_BACKGROUNDS, 4.5),
    rule("text.muted", &["surface.sunken", "row.hover"], 3.0),
    rule(
        "accent.text",
        &["surface.canvas", "surface.sunken", "surface.raised"],
        4.5,
    ),
    rule("text.on_accent", &["accent.base", "accent.hover"], 4.5),
    rule("border.focus", &["surface.sunken", "surface.canvas"], 3.0),
    rule("border.control", &["surface.sunken", "surface.canvas"], 3.0),
    rule(
        "signature.revision",
        &["surface.sunken", "row.hover", "row.selected"],
        3.0,
    ),
    rule("status.needs_review.fg", &["status.needs_review.bg"], 4.5),
    rule(
        "status.active_comments.fg",
        &["status.active_comments.bg"],
        4.5,
    ),
    rule("status.reviewed.fg", &["status.reviewed.bg"], 4.5),
    rule("status.needs_review.solid", TABLE, 3.0),
    rule("status.active_comments.solid", TABLE, 3.0),
    rule("status.reviewed.solid", TABLE, 3.0),
    rule("feedback.danger.fg", &["feedback.danger.bg"], 4.5),
    rule("feedback.warning.fg", &["feedback.warning.bg"], 4.5),
    rule("feedback.success.fg", &["feedback.success.bg"], 4.5),
    rule("feedback.info.fg", &["feedback.info.bg"], 4.5),
    rule("vote.approved", SUNKEN, 3.0),
    rule("vote.waiting", SUNKEN, 3.0),
    rule("vote.rejected", SUNKEN, 3.0),
];

/// Every rule violation as an issue (empty for a good theme).
pub fn validate(theme: &ThemeId, palette: &Palette) -> Vec<ThemeIssue> {
    let mut issues = Vec::new();
    for rule in CONTRAST_RULES {
        let Some(fg) = palette.get(rule.fg) else {
            continue;
        };
        for bg_key in rule.bg {
            let Some(bg) = palette.get(bg_key) else {
                continue;
            };
            let ratio = contrast_ratio(fg, bg);
            if ratio >= rule.min {
                continue;
            }
            let level = if ratio >= rule.min - WARNING_MARGIN {
                IssueLevel::Warning
            } else {
                IssueLevel::Error
            };
            issues.push(ThemeIssue {
                key: Some(rule.fg),
                ..ThemeIssue::new(
                    theme,
                    level,
                    format!(
                        "{} on {bg_key} is {ratio:.2}:1; needs {:.1}:1",
                        rule.fg, rule.min
                    ),
                )
            });
        }
    }
    issues
}

#[cfg(test)]
mod tests;
