//! Shared fixtures: one project with one repo per org, Ann as the tracked author, Rev as the
//! reviewer, and helpers that wait for refreshes.

use std::sync::Arc;

use prm_ado::testing::{ApiCall, FakeOrg};
use prm_config::{Config, OrgProfile, schema::PeopleConfig};
use prm_domain::{
    Person, PrKey,
    diff::ChangeKind,
    model::{Project, PullRequest, Repository},
    testing::{PrBuilder, key, org, person, project, repo},
};

use crate::{
    event::AppEvent,
    state::{AppState, OrgHealth},
    testing::{HarnessSetup, ServiceHarness},
};

pub(super) const CONTOSO: &str = "contoso";
pub(super) const FABRIKAM: &str = "fabrikam";

pub(super) fn me() -> Person {
    person(10, "Me")
}

pub(super) fn ann() -> Person {
    person(2, "Ann")
}

pub(super) fn bob() -> Person {
    person(4, "Bob")
}

pub(super) fn rev() -> Person {
    person(3, "Rev")
}

pub(super) fn profile(name: &str) -> OrgProfile {
    OrgProfile {
        url: format!("https://dev.azure.com/{name}"),
        include_me: false,
        people: PeopleConfig {
            tracked_authors: vec![ann()],
            reviewers: vec![rev()],
            ignored_comment_authors: Vec::new(),
        },
        ..OrgProfile::default()
    }
}

pub(super) fn config(names: &[&str]) -> Config {
    Config {
        orgs: names.iter().map(|n| profile(n)).collect(),
        ..Config::default()
    }
}

pub(super) fn project_n(n: u128) -> Project {
    Project {
        id: project(n),
        name: format!("Project{n}").into(),
    }
}

pub(super) fn repo_n(n: u128) -> Repository {
    Repository {
        id: repo(n),
        name: format!("repo{n}").into(),
        project: project(n),
        project_name: format!("Project{n}").into(),
        disabled: false,
        in_maintenance: false,
        is_fork: false,
        web_url: None,
    }
}

/// An org whose projects 1..=n each hold one repo with the same number.
pub(super) fn fake_org(projects: u128, prs: Vec<PullRequest>) -> FakeOrg {
    FakeOrg {
        me: Some(me()),
        projects: (1..=projects).map(project_n).collect(),
        repos: (1..=projects).map(repo_n).collect(),
        prs,
        ..FakeOrg::default()
    }
}

/// Ann's PR `n` in project 1 / repo 1.
pub(super) fn pr(name: &str, n: i32) -> PullRequest {
    PrBuilder::new(key(name, 1, n)).author(ann()).build()
}

/// A PR in project `p` / repo `p`.
pub(super) fn pr_in(name: &str, p: u128, n: i32) -> PullRequest {
    PrBuilder::new(key(name, p, n))
        .project(p)
        .author(ann())
        .build()
}

pub(super) fn setup(orgs: Vec<(&str, Vec<PullRequest>)>) -> HarnessSetup {
    let names: Vec<&str> = orgs.iter().map(|(n, _)| *n).collect();
    HarnessSetup {
        config: config(&names),
        orgs: orgs
            .into_iter()
            .map(|(n, prs)| (org(n), fake_org(1, prs)))
            .collect(),
        ..HarnessSetup::default()
    }
}

pub(super) fn list_calls(h: &ServiceHarness, name: &str) -> u32 {
    h.api(&org(name)).calls(ApiCall::ActivePullRequests)
}

/// The org finished at least `n` PR listings and is no longer refreshing.
pub(super) async fn refreshed(h: &mut ServiceHarness, name: &str, n: u32) -> Arc<AppState> {
    let api = h.api(&org(name));
    let id = org(name);
    h.wait_for(&format!("{name} refresh #{n}"), move |s| {
        api.calls(ApiCall::ActivePullRequests) >= n && settled(s, &id)
    })
    .await
}

/// The org has a result (not idle, not refreshing).
pub(super) fn settled(state: &AppState, id: &prm_domain::OrgId) -> bool {
    state
        .org(id)
        .is_some_and(|o| !matches!(o.health, OrgHealth::Refreshing { .. } | OrgHealth::Idle))
}

/// Every change event emitted so far, flattened.
pub(super) fn change_kinds(h: &ServiceHarness) -> Vec<(PrKey, ChangeKind)> {
    h.events()
        .into_iter()
        .filter_map(|e| {
            if let AppEvent::Changes(events) = e {
                Some(events)
            } else {
                None
            }
        })
        .flat_map(|events| {
            events
                .iter()
                .map(|e| (e.pr.key.clone(), e.kind.clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}
