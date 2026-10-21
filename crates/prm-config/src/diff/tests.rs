use super::*;
use crate::validate::normalize;

fn config(orgs: &[&str]) -> Config {
    let orgs = orgs
        .iter()
        .map(|url| OrgProfile {
            url: (*url).into(),
            ..OrgProfile::default()
        })
        .collect();
    normalize(Config {
        orgs,
        ..Config::default()
    })
    .0
}

fn only_change(old: &Config, new: &Config) -> OrgChanges {
    let changes = diff_configs(old, new);
    assert_eq!(changes.changed.len(), 1, "{changes:?}");
    changes.changed.into_values().next().unwrap()
}

#[test]
fn diff_configs_classify_only() {
    let old = config(&["contoso"]);
    let mut new = old.clone();
    new.orgs[0]
        .people
        .reviewers
        .push(prm_domain::Person::new(prm_domain::Vsid::from_u128(1), "R"));
    assert_eq!(
        only_change(&old, &new),
        OrgChanges {
            classify: true,
            ..OrgChanges::default()
        }
    );
}

#[test]
fn diff_configs_scope_and_authors() {
    let old = config(&["contoso"]);
    let mut new = old.clone();
    new.orgs[0].scope.all_projects = false;
    new.orgs[0].include_me = false;
    let change = only_change(&old, &new);
    assert!(change.scope && change.authors && !change.classify);
}

#[test]
fn diff_configs_auth_endpoint() {
    let old = config(&["contoso"]);
    let mut new = old.clone();
    new.orgs[0].tenant = "72f988bf-86f1-41af-91ab-2d7cd011db47".into();
    assert_eq!(
        only_change(&old, &new),
        OrgChanges {
            auth: true,
            ..OrgChanges::default()
        }
    );
}

#[test]
fn diff_configs_add_remove() {
    let changes = diff_configs(
        &config(&["contoso", "fabrikam"]),
        &config(&["contoso", "tailspin"]),
    );
    assert_eq!(
        changes.added.iter().map(OrgId::as_str).collect::<Vec<_>>(),
        vec!["tailspin"]
    );
    assert_eq!(
        changes
            .removed
            .iter()
            .map(OrgId::as_str)
            .collect::<Vec<_>>(),
        vec!["fabrikam"]
    );
    assert!(changes.changed.is_empty());
}

#[test]
fn diff_configs_global_sections() {
    let old = config(&[]);
    let mut new = old.clone();
    new.ui.launch_at_login = true;
    let changes = diff_configs(&old, &new);
    assert!(changes.ui && changes.autostart && !changes.refresh);
}

/// A duplicate entry used to stand in for the org (last one won), so the org was diffed
/// against the disabled duplicate's profile.
#[test]
fn diff_configs_duplicate_uses_first_entry() {
    let old = config(&[
        "https://dev.azure.com/contoso",
        "https://contoso.visualstudio.com",
    ]);
    let mut new = old.clone();
    new.orgs[1].label = "Duplicate".into();
    let changes = diff_configs(&old, &new);
    assert!(changes.changed.is_empty(), "{changes:?}");

    new.orgs[0].label = "Contoso".into();
    assert_eq!(
        only_change(&old, &new),
        OrgChanges {
            cosmetic: true,
            ..OrgChanges::default()
        }
    );
}
