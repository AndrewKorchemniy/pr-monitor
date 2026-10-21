//! What changed between two normalized configs, so the service restarts only what it must.

use std::collections::BTreeMap;

use prm_domain::OrgId;

use crate::schema::{Config, OrgProfile};

/// Which parts of one org changed. Each flag maps to a different service reaction; label,
/// mute and PAT-expiry edits need none, because the org list is rebuilt on every apply.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct OrgChanges {
    /// The org URL (id) itself.
    pub endpoint: bool,
    /// Auth mode, tenant or Entra settings: re-run sign-in.
    pub auth: bool,
    /// Projects or repos: re-plan queries and rebaseline.
    pub scope: bool,
    /// Tracked authors, include-me or drafts: rebaseline.
    pub authors: bool,
    /// Reviewers, ignored authors or review policy: reclassify only.
    pub classify: bool,
    /// The org was turned on or off: start or stop its refreshes.
    pub enabled: bool,
}

impl OrgChanges {
    /// Whether the service has anything to react to.
    pub fn any(self) -> bool {
        self != Self::default()
    }
}

/// Everything in a config apply that needs a service reaction.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ConfigChanges {
    /// Orgs that are gone: stop their jobs and delete their data.
    pub removed: Vec<OrgId>,
    /// Orgs present in both configs whose settings changed.
    pub changed: BTreeMap<OrgId, OrgChanges>,
    /// Interval or jitter: reschedule every org.
    pub refresh: bool,
    /// Proxy or timeouts: they apply after a restart.
    pub network: bool,
    /// Launch at login: update the OS registration.
    pub autostart: bool,
}

/// Both configs must already be normalized (ids set). For a duplicated org id the first
/// entry counts, as in [`Config::org`]; `normalize` turns later duplicates off.
pub fn diff_configs(old: &Config, new: &Config) -> ConfigChanges {
    let (old_orgs, new_orgs) = (index(old), index(new));
    let mut changes = ConfigChanges {
        removed: old_orgs
            .keys()
            .filter(|k| !new_orgs.contains_key(*k))
            .map(|k| (*k).clone())
            .collect(),
        refresh: old.refresh != new.refresh,
        network: old.network != new.network,
        autostart: old.ui.launch_at_login != new.ui.launch_at_login,
        ..ConfigChanges::default()
    };
    for (id, new_org) in &new_orgs {
        if let Some(old_org) = old_orgs.get(id) {
            let org_changes = diff_org(old_org, new_org);
            if org_changes.any() {
                changes.changed.insert((*id).clone(), org_changes);
            }
        }
    }
    changes
}

/// Orgs by id, keeping the first entry for each id.
fn index(cfg: &Config) -> BTreeMap<&OrgId, &OrgProfile> {
    let mut map = BTreeMap::new();
    for org in &cfg.orgs {
        if let Some(id) = org.org_id() {
            map.entry(id).or_insert(org);
        }
    }
    map
}

fn diff_org(old: &OrgProfile, new: &OrgProfile) -> OrgChanges {
    OrgChanges {
        endpoint: old.url != new.url,
        auth: old.auth != new.auth || old.tenant != new.tenant || old.entra != new.entra,
        scope: old.scope != new.scope || old.org_wide_query != new.org_wide_query,
        authors: old.people.tracked_authors != new.people.tracked_authors
            || old.include_me != new.include_me
            || old.drafts != new.drafts,
        classify: old.people.reviewers != new.people.reviewers
            || old.people.ignored_comment_authors != new.people.ignored_comment_authors
            || old.review != new.review,
        enabled: old.enabled != new.enabled,
    }
}

#[cfg(test)]
mod tests;
