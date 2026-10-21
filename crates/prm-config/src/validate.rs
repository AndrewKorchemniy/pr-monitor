//! `normalize`: fixes up a loaded config (clamps, canonical URLs, dedupe) and reports what it
//! changed. Pure; runs after every load and before every save.

use std::{collections::HashSet, ops::RangeInclusive};

use prm_domain::{Person, ids::parse_org_input};

use crate::{
    error::{ConfigIssue, Severity},
    limits,
    schema::{Config, OrgProfile},
};

/// Microsoft first-party app ids. Allowed, but the user should know sign-in logs show that app.
const FIRST_PARTY_CLIENT_IDS: [&str; 3] = [
    "04b07795-8ddb-461a-bbee-02f9e1bf7b46", // Azure CLI
    "872cd9fa-d31f-45e0-9eab-6e460a02d1f1", // Visual Studio
    "d735b71b-9eee-4a4f-ad23-421660877ba6", // Git Credential Manager
];

pub fn normalize(mut cfg: Config) -> (Config, Vec<ConfigIssue>) {
    let mut issues = Vec::new();
    clamp_numbers(&mut cfg, &mut issues);
    let mut seen = HashSet::new();
    for (index, org) in cfg.orgs.iter_mut().enumerate() {
        normalize_org(org, &format!("orgs[{index}]"), &mut issues);
        let Some(id) = org.id.clone() else { continue };
        if !seen.insert(id.clone()) {
            org.enabled = false;
            issues.push(ConfigIssue::new(
                Severity::Error,
                format!("orgs[{index}].url"),
                format!("“{id}” is listed twice; the second entry is turned off."),
            ));
        }
    }
    (cfg, issues)
}

fn clamp_numbers(cfg: &mut Config, issues: &mut Vec<ConfigIssue>) {
    let mut clamp = Clamper { issues };
    clamp.ord(
        "refresh.interval_minutes",
        &mut cfg.refresh.interval_minutes,
        limits::INTERVAL_MINUTES,
    );
    clamp.ord(
        "refresh.jitter_percent",
        &mut cfg.refresh.jitter_percent,
        limits::JITTER_PERCENT,
    );
    clamp.ord(
        "refresh.startup_delay_secs",
        &mut cfg.refresh.startup_delay_secs,
        limits::STARTUP_DELAY_SECS,
    );
    clamp.ord(
        "notifications.max_per_refresh",
        &mut cfg.notifications.max_per_refresh,
        limits::MAX_PER_REFRESH,
    );
    clamp.ord(
        "network.connect_timeout_secs",
        &mut cfg.network.connect_timeout_secs,
        limits::CONNECT_TIMEOUT_SECS,
    );
    clamp.ord(
        "network.request_timeout_secs",
        &mut cfg.network.request_timeout_secs,
        limits::REQUEST_TIMEOUT_SECS,
    );
    clamp.ord(
        "network.max_concurrency",
        &mut cfg.network.max_concurrency,
        limits::MAX_CONCURRENCY,
    );
    clamp.float(
        "ui.font_scale",
        &mut cfg.ui.font_scale,
        limits::FONT_SCALE,
        limits::DEFAULT_FONT_SCALE,
    );
}

/// Clamps values into range and records a warning for each change.
struct Clamper<'a> {
    issues: &'a mut Vec<ConfigIssue>,
}

impl Clamper<'_> {
    fn ord<T: Ord + Copy + std::fmt::Display>(
        &mut self,
        path: &str,
        value: &mut T,
        range: RangeInclusive<T>,
    ) {
        let clamped = (*value).clamp(*range.start(), *range.end());
        if clamped != *value {
            self.warn(path, &value.to_string(), &clamped.to_string());
            *value = clamped;
        }
    }

    /// Like [`Self::ord`]; a non-finite value (NaN, infinity) becomes `default`.
    fn float(&mut self, path: &str, value: &mut f32, range: RangeInclusive<f32>, default: f32) {
        let clamped = if value.is_finite() {
            value.clamp(*range.start(), *range.end())
        } else {
            default
        };
        if clamped.to_bits() != value.to_bits() {
            self.warn(path, &value.to_string(), &clamped.to_string());
            *value = clamped;
        }
    }

    fn warn(&mut self, path: &str, from: &str, to: &str) {
        self.issues.push(ConfigIssue::new(
            Severity::Warning,
            path,
            format!("{from} is out of range; using {to}."),
        ));
    }
}

fn normalize_org(org: &mut OrgProfile, path: &str, issues: &mut Vec<ConfigIssue>) {
    match parse_org_input(&org.url) {
        Ok(id) => {
            if org.id.as_ref().is_some_and(|old| *old != id) {
                issues.push(ConfigIssue::new(
                    Severity::Warning,
                    format!("{path}.id"),
                    format!("The id doesn't match the URL; using “{id}”."),
                ));
            }
            org.url = id.web_url();
            org.id = Some(id);
        }
        Err(e) => {
            org.id = None;
            org.enabled = false;
            issues.push(ConfigIssue::new(
                Severity::Error,
                format!("{path}.url"),
                format!(
                    "“{}” isn't an Azure DevOps organization URL ({e}).",
                    org.url
                ),
            ));
        }
    }
    normalize_ids(org, path, issues);
    dedupe(&mut org.people.tracked_authors);
    dedupe(&mut org.people.reviewers);
    dedupe(&mut org.people.ignored_comment_authors);
    drop_empty_projects(org, path, issues);
}

fn normalize_ids(org: &mut OrgProfile, path: &str, issues: &mut Vec<ConfigIssue>) {
    if !org.tenant.trim().is_empty() && org.tenant_override().is_none() {
        issues.push(warning(
            &format!("{path}.tenant"),
            "The tenant isn't a GUID and is ignored.",
        ));
        org.tenant.clear();
    }
    check_client_id(org, path, issues);
    if !org.pat_expires_on.trim().is_empty() && org.pat_expiry().is_none() {
        issues.push(warning(
            &format!("{path}.pat_expires_on"),
            "The date isn't a real YYYY-MM-DD date and is ignored.",
        ));
        org.pat_expires_on.clear();
    }
}

fn check_client_id(org: &mut OrgProfile, path: &str, issues: &mut Vec<ConfigIssue>) {
    let client_id = org.entra.client_id.trim().to_ascii_lowercase();
    if client_id.is_empty() {
        return;
    }
    let field = format!("{path}.entra.client_id");
    if uuid::Uuid::parse_str(&client_id).is_err() {
        issues.push(warning(
            &field,
            "The client ID isn't a GUID and is ignored.",
        ));
        org.entra.client_id.clear();
    } else if FIRST_PARTY_CLIENT_IDS.contains(&client_id.as_str()) {
        let message = "This is a Microsoft app ID; sign-in logs will show that app.";
        issues.push(ConfigIssue::new(Severity::Info, field, message));
    }
}

fn drop_empty_projects(org: &mut OrgProfile, path: &str, issues: &mut Vec<ConfigIssue>) {
    let before = org.scope.projects.len();
    org.scope
        .projects
        .retain(|p| p.id.is_some() || !p.name.trim().is_empty());
    if org.scope.projects.len() != before {
        issues.push(warning(
            &format!("{path}.scope.projects"),
            "A project without an id or name was removed.",
        ));
    }
}

/// Keeps the first occurrence of each id.
fn dedupe(people: &mut Vec<Person>) {
    let mut seen = HashSet::new();
    people.retain(|p| seen.insert(p.id));
}

fn warning(path: &str, message: &str) -> ConfigIssue {
    ConfigIssue::new(Severity::Warning, path, message)
}

#[cfg(test)]
mod tests;
