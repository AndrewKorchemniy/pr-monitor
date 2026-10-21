//! What changed between two normalized configs, so the service restarts only what it must.

use std::collections::BTreeMap;

use prm_domain::OrgId;

use crate::schema::{Config, OrgProfile};

/// Which parts of one org changed. Each flag maps to a different service reaction.
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
    pub enabled: bool,
    /// Label, muted or PAT expiry: no refresh needed.
    pub cosmetic: bool,
}

impl OrgChanges {
    pub fn any(self) -> bool {
        self != Self::default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ConfigChanges {
    pub added: Vec<OrgId>,
    pub removed: Vec<OrgId>,
    pub changed: BTreeMap<OrgId, OrgChanges>,
    pub refresh: bool,
    pub network: bool,
    pub notifications: bool,
    pub ui: bool,
    pub autostart: bool,
}

/// Both configs must already be normalized (ids set).
pub fn diff_configs(old: &Config, new: &Config) -> ConfigChanges {
    let (old_orgs, new_orgs) = (index_orgs(old), index_orgs(new));
    let mut changes = ConfigChanges {
        added: new_orgs
            .keys()
            .filter(|k| !old_orgs.contains_key(k))
            .cloned()
            .collect(),
        removed: old_orgs
            .keys()
            .filter(|k| !new_orgs.contains_key(k))
            .cloned()
            .collect(),
        refresh: old.refresh != new.refresh,
        network: old.network != new.network,
        notifications: old.notifications != new.notifications,
        ui: old.ui != new.ui,
        autostart: old.ui.launch_at_login != new.ui.launch_at_login,
        ..ConfigChanges::default()
    };
    for (id, new_org) in &new_orgs {
        if let Some(old_org) = old_orgs.get(id) {
            let org_changes = diff_org(old_org, new_org);
            if org_changes.any() {
                changes.changed.insert(id.clone(), org_changes);
            }
        }
    }
    changes
}

/// Profiles by id. The first profile per id is the org; `normalize` disables later duplicates,
/// and the service ignores them, so their changes must not be applied to the org.
fn index_orgs(cfg: &Config) -> BTreeMap<OrgId, OrgProfile> {
    let mut orgs = BTreeMap::new();
    for profile in &cfg.orgs {
        if let Some(id) = profile.org_id() {
            orgs.entry(id.clone()).or_insert_with(|| profile.clone());
        }
    }
    orgs
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
        cosmetic: old.label != new.label
            || old.muted != new.muted
            || old.pat_expires_on != new.pat_expires_on,
    }
}

#[cfg(test)]
mod tests;
