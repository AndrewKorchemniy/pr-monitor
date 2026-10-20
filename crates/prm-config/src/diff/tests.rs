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
    assert!(changes.autostart && !changes.refresh && !changes.network);
}

/// With an org listed twice, the index kept the last (disabled) duplicate, so an edit to the
/// real first entry was missed and newly in-scope PRs toasted instead of rebaselining.
#[test]
fn diff_configs_duplicate_org_compares_the_first_entry() {
    let old = config(&["contoso", "contoso"]);
    let mut new = old.clone();
    new.orgs[0].scope.all_projects = false;
    let change = only_change(&old, &new);
    assert!(change.scope && !change.enabled, "{change:?}");
}

#[test]
fn diff_configs_label_only_needs_no_reaction() {
    let old = config(&["contoso"]);
    let mut new = old.clone();
    new.orgs[0].label = "Contoso".into();
    new.orgs[0].muted = true;
    assert_eq!(diff_configs(&old, &new), ConfigChanges::default());
}
