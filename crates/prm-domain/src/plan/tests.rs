use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::{
    scope::ProjectSel,
    testing::{PrBuilder, key, person, project, record, repo, ts, vsid},
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
        authors: Authors::Only(authors.iter().map(|a| vsid(*a)).collect()),
        drafts,
    }
}

fn everyone(except: Option<u128>) -> TrackSpec {
    TrackSpec {
        authors: Authors::Everyone {
            except: except.map(vsid),
        },
        drafts: DraftPolicy::Show,
    }
}

/// PR `n` in repo 11 by person `author`.
fn by(n: i32, author: u128) -> PullRequest {
    PrBuilder::new(key("contoso", 11, n))
        .author(person(author, "P"))
        .build()
}

/// PR numbers in their order.
fn numbers(prs: &[PullRequest]) -> Vec<i32> {
    prs.iter().map(|pr| pr.key.number.0).collect()
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
fn select_tracked_explicit_list_with_and_without_me() {
    let s = scope(&[(1, true, &[11])]);
    let prs = vec![by(1, 1), by(2, 2), by(3, 9)];
    let cases: [(&[u128], Vec<i32>); 2] = [(&[1, 9], vec![1, 3]), (&[1], vec![1])];
    for (authors, expected) in cases {
        let kept = select_tracked(prs.clone(), &s, &track(authors, DraftPolicy::Show));
        assert_eq!(numbers(&kept), expected, "authors {authors:?}");
    }
}

#[test]
fn select_tracked_everyone_keeps_every_author_in_scope() {
    let s = scope(&[(1, false, &[11])]);
    let outside = PrBuilder::new(key("contoso", 12, 4))
        .author(person(3, "C"))
        .build();
    let prs = vec![by(1, 1), by(2, 2), by(3, 9), outside];
    let kept = select_tracked(prs, &s, &everyone(None));
    assert_eq!(numbers(&kept), vec![1, 2, 3]);
}

#[test]
fn select_tracked_everyone_except_me() {
    let s = scope(&[(1, true, &[11])]);
    let prs = vec![by(1, 1), by(2, 9), by(3, 2)];
    let kept = select_tracked(prs, &s, &everyone(Some(9)));
    assert_eq!(numbers(&kept), vec![1, 3]);
}

#[test]
fn authors_tracks_follows_each_rule() {
    let cases = [
        (everyone(None).authors, [true, true, true]),
        (everyone(Some(9)).authors, [true, true, false]),
        (
            track(&[1, 9], DraftPolicy::Show).authors,
            [true, false, true],
        ),
        (track(&[1], DraftPolicy::Show).authors, [true, false, false]),
    ];
    for (authors, expected) in cases {
        let tracked = [1, 2, 9].map(|n| authors.tracks(vsid(n)));
        assert_eq!(tracked, expected, "{authors:?}");
    }
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

/// Previous records for the PRs numbered in `read_at`, each last read that many seconds after
/// the base time.
fn read_before(prs: &[PullRequest], read_at: &[(i32, i64)]) -> BTreeMap<PrKey, PrRecord> {
    let mut prev = BTreeMap::new();
    for (n, at) in read_at {
        let Some(pr) = prs.iter().find(|pr| pr.key.number.0 == *n) else {
            continue;
        };
        let mut known = record(pr.clone(), Vec::new());
        known.fetched_at = ts(crate::testing::BASE_TIME_SECS + at);
        prev.insert(pr.key.clone(), known);
    }
    prev
}

#[test]
fn plan_thread_reads_within_budget_reads_all_in_order() {
    let prs = vec![by(3, 1), by(1, 2), by(2, 3)];
    let reads = plan_thread_reads(prs, &BTreeMap::new(), vsid(9), 3);
    assert_eq!(numbers(&reads.read), vec![3, 1, 2]);
    assert_eq!(reads.reuse, Vec::new());
    assert_eq!(reads.deferred, 0);
}

#[test]
fn plan_thread_reads_over_budget_reads_mine_then_new_then_oldest() {
    let mine_as_reviewer = PrBuilder::new(key("contoso", 11, 6))
        .author(person(2, "B"))
        .reviewer(person(9, "Me"), 0)
        .build();
    let prs = vec![
        by(1, 1),
        by(2, 2),
        by(3, 3),
        by(4, 4),
        by(5, 9),
        mine_as_reviewer,
        by(7, 7),
    ];
    // 1 and 2 were read recently, 3 long ago; 4 and 7 are new; 5 is mine and known.
    let prev = read_before(&prs, &[(1, 50), (2, 40), (3, 10), (5, 60)]);
    let reads = plan_thread_reads(prs, &prev, vsid(9), 4);
    assert_eq!(numbers(&reads.read), vec![6, 5, 4, 7]);
    assert_eq!(numbers(&reads.reuse), vec![3, 2, 1]);
    assert_eq!(reads.deferred, 0);
}

#[test]
fn plan_thread_reads_defers_new_prs_over_budget() {
    let prs = vec![by(1, 1), by(2, 2), by(3, 3), by(4, 4)];
    let prev = read_before(&prs, &[(4, 0)]);
    let reads = plan_thread_reads(prs, &prev, vsid(9), 2);
    assert_eq!(numbers(&reads.read), vec![1, 2]);
    assert_eq!(numbers(&reads.reuse), vec![4]);
    assert_eq!(reads.deferred, 1, "PR 3 waits for a later refresh");
}
