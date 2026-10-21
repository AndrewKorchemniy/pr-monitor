//! Tracking everyone: an empty people list, the signed-in user's own PRs, and thread reads
//! spread over refreshes when a project has more active PRs than the read budget.

use std::{collections::BTreeSet, time::Duration};

use prm_ado::testing::ApiCall;
use prm_domain::{
    Person, PrKey,
    diff::ChangeKind,
    model::{PrState, PullRequest},
    testing::{PrBuilder, key, org, person},
    tombstone::CloseOutcome,
};

use super::support::{CONTOSO, ann, bob, change_kinds, me, pr, refreshed, setup};
use crate::{
    command::Command,
    consts::THREAD_READ_BUDGET,
    state::AppState,
    testing::{HarnessSetup, ServiceHarness},
};

/// PR `n` in project 1 / repo 1 by `author`.
fn pr_by(n: i32, author: Person) -> PullRequest {
    PrBuilder::new(key(CONTOSO, 1, n)).author(author).build()
}

/// Ann's PR 1, Bob's PR 2 and the signed-in user's PR 3, with nobody listed.
fn everyone_setup(include_me: bool) -> HarnessSetup {
    let prs = vec![pr(CONTOSO, 1), pr_by(2, bob()), pr_by(3, me())];
    let mut start = setup(vec![(CONTOSO, prs)]);
    start.config.orgs[0].people.tracked_authors.clear();
    start.config.orgs[0].include_me = include_me;
    start
}

fn row_numbers(state: &AppState) -> BTreeSet<i32> {
    state.rows.iter().map(|r| r.key.number.0).collect()
}

/// An empty list used to show only the signed-in user's PRs (or none), though it reads as
/// "everyone in the chosen repositories".
#[tokio::test(start_paused = true)]
async fn empty_people_list_publishes_every_author_but_me() {
    let mut h = ServiceHarness::start(everyone_setup(false));

    let state = refreshed(&mut h, CONTOSO, 1).await;

    assert_eq!(row_numbers(&state), BTreeSet::from([1, 2]));
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn empty_people_list_with_include_me_publishes_everyone() {
    let mut h = ServiceHarness::start(everyone_setup(true));

    let state = refreshed(&mut h, CONTOSO, 1).await;

    assert_eq!(row_numbers(&state), BTreeSet::from([1, 2, 3]));
    h.stop().await;
}

/// Clearing the list widens what is shown; the newly shown PRs are not news.
#[tokio::test(start_paused = true)]
async fn clearing_the_people_list_rebaselines() {
    let prs = vec![pr(CONTOSO, 1), pr_by(2, bob())];
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, prs)]));
    let state = refreshed(&mut h, CONTOSO, 1).await;
    assert_eq!(row_numbers(&state), BTreeSet::from([1]));

    let mut config = (*state.config).clone();
    config.orgs[0].people.tracked_authors.clear();
    h.send(Command::ApplyConfig(Box::new(config)));
    let state = refreshed(&mut h, CONTOSO, 2).await;

    assert_eq!(row_numbers(&state), BTreeSet::from([1, 2]));
    assert_eq!(change_kinds(&h), Vec::new());
    h.stop().await;
}

/// A PR that leaves the listing is looked up and closed when anyone's PRs are tracked, not
/// only listed people's.
#[tokio::test(start_paused = true)]
async fn vanished_pr_closes_when_tracking_everyone() {
    let mut h = ServiceHarness::start(everyone_setup(false));
    refreshed(&mut h, CONTOSO, 1).await;
    h.api(&org(CONTOSO))
        .update(|o| o.prs[1].state = PrState::Completed);

    h.send(Command::RefreshNow);
    let state = refreshed(&mut h, CONTOSO, 2).await;

    assert_eq!(row_numbers(&state), BTreeSet::from([1]));
    let closed = ChangeKind::Closed {
        outcome: CloseOutcome::Completed,
    };
    assert_eq!(change_kinds(&h), vec![(pr_by(2, bob()).key, closed)]);
    h.stop().await;
}

/// `count` PRs by as many different authors, numbered from 1.
fn crowd(count: usize) -> Vec<PullRequest> {
    (1..=count)
        .map(|n| {
            let author = person(1_000 + u128::try_from(n).unwrap(), "Contoso dev");
            pr_by(i32::try_from(n).unwrap(), author)
        })
        .collect()
}

/// A project with more active PRs than the read budget never reads more threads than the
/// budget in one refresh. The first load fills in over two refreshes without a toast, and
/// only a PR opened afterwards is news.
#[tokio::test(start_paused = true)]
async fn large_project_reads_threads_within_the_budget() {
    let count = THREAD_READ_BUDGET + 10;
    let mut start = setup(vec![(CONTOSO, crowd(count))]);
    start.config.orgs[0].people.tracked_authors.clear();
    let mut h = ServiceHarness::start(start);
    let api = h.api(&org(CONTOSO));
    let budget = u32::try_from(THREAD_READ_BUDGET).unwrap();

    let state = refreshed(&mut h, CONTOSO, 1).await;
    assert_eq!(api.calls(ApiCall::Threads), budget);
    assert_eq!(
        state.rows.len(),
        THREAD_READ_BUDGET,
        "10 PRs wait their turn"
    );

    h.send(Command::RefreshNow);
    let state = refreshed(&mut h, CONTOSO, 2).await;
    assert_eq!(api.calls(ApiCall::Threads), 2 * budget);
    assert_eq!(state.rows.len(), count);
    assert!(state.rows.iter().all(|r| !r.stale && !r.unread));

    let opened = pr_by(9_999, ann());
    let opened_key: PrKey = opened.key.clone();
    api.update(|o| o.prs.push(opened));
    h.advance(Duration::from_secs(3)).await;
    h.send(Command::RefreshNow);
    let state = refreshed(&mut h, CONTOSO, 3).await;
    assert_eq!(state.rows.len(), count + 1);
    assert_eq!(change_kinds(&h), vec![(opened_key, ChangeKind::NewPr)]);
    h.stop().await;
}
