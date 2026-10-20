use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::{
    scope::ProjectSel,
    testing::{PrBuilder, key, person, project, repo},
};

fn scope(entries: &[(u128, bool, &[u128])]) -> ResolvedScope {
    let mut projects = BTreeMap::new();
    for (p, all, repos) in entries {
        projects.insert(
            project(*p),
            ProjectSel {
                name: "P".into(),
                all_repos: *all,
                repos: repos.iter().map(|r| repo(*r)).collect::<BTreeSet<_>>(),
            },
        );
    }
    ResolvedScope {
        projects,
        ..ResolvedScope::default()
    }
}

#[test]
fn plan_whole_project_one_query() {
    let s = scope(&[(1, true, &[11])]);
    assert_eq!(
        plan_queries(&s, QueryMode::PerProject),
        vec![PrQuery::Project {
            project: project(1)
        }]
    );
}

#[test]
fn plan_two_repos_same_project_uses_project_query() {
    let s = scope(&[(1, false, &[11, 12])]);
    assert_eq!(
        plan_queries(&s, QueryMode::PerProject),
        vec![PrQuery::Project {
            project: project(1)
        }]
    );
}

#[test]
fn plan_single_repo_uses_repo_query() {
    let s = scope(&[(1, false, &[11]), (2, false, &[])]);
    assert_eq!(
        plan_queries(&s, QueryMode::PerProject),
        vec![PrQuery::Repo {
            project: project(1),
            repo: repo(11)
        }]
    );
}

#[test]
fn plan_org_wide_single_query() {
    let s = scope(&[(1, true, &[11]), (2, true, &[21])]);
    assert_eq!(plan_queries(&s, QueryMode::OrgWide), vec![PrQuery::OrgWide]);
}

#[test]
fn plan_empty_scope_no_queries() {
    assert_eq!(
        plan_queries(&ResolvedScope::default(), QueryMode::OrgWide),
        Vec::<PrQuery>::new()
    );
    assert_eq!(
        plan_queries(&scope(&[(1, false, &[])]), QueryMode::PerProject),
        Vec::<PrQuery>::new()
    );
}

fn track(authors: &[u128], drafts: DraftPolicy) -> TrackSpec {
    TrackSpec {
        authors: authors.iter().map(|a| crate::testing::vsid(*a)).collect(),
        drafts,
    }
}

#[test]
fn select_tracked_author_and_scope() {
    let s = scope(&[(1, false, &[11])]);
    let prs = vec![
        PrBuilder::new(key("contoso", 11, 1))
            .author(person(1, "A"))
            .build(),
        PrBuilder::new(key("contoso", 11, 2))
            .author(person(2, "B"))
            .build(),
        PrBuilder::new(key("contoso", 12, 3))
            .author(person(1, "A"))
            .build(),
    ];
    let kept = select_tracked(prs, &s, &track(&[1], DraftPolicy::Show));
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].key.number.0, 1);
}

#[test]
fn select_tracked_includes_me() {
    let s = scope(&[(1, true, &[11])]);
    let prs = vec![
        PrBuilder::new(key("contoso", 11, 1))
            .author(person(9, "Me"))
            .build(),
    ];
    assert_eq!(
        select_tracked(prs, &s, &track(&[1, 9], DraftPolicy::Show)).len(),
        1
    );
}

#[test]
fn select_tracked_hides_drafts() {
    let s = scope(&[(1, true, &[11])]);
    let prs = vec![
        PrBuilder::new(key("contoso", 11, 1))
            .author(person(1, "A"))
            .draft()
            .build(),
        PrBuilder::new(key("contoso", 11, 2))
            .author(person(1, "A"))
            .build(),
    ];
    assert_eq!(
        select_tracked(prs.clone(), &s, &track(&[1], DraftPolicy::Hide)).len(),
        1
    );
    assert_eq!(
        select_tracked(prs, &s, &track(&[1], DraftPolicy::Show)).len(),
        2
    );
}
