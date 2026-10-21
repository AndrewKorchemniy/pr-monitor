//! Tracking everyone: an empty people list, the signed-in user's own PRs, and thread reads
//! spread over refreshes when a project has more active PRs than the read budget.

use std::{collections::BTreeSet, time::Duration};

use prm_ado::{AdoError, testing::ApiCall};
use prm_config::OrgPersisted;
use prm_domain::{
    Person, PrKey,
    diff::ChangeKind,
    model::{PrState, PullRequest},
    testing::{PrBuilder, ThreadBuilder, key, org, person, record},
    tombstone::CloseOutcome,
};
use tokio::time::Instant;

use super::support::{CONTOSO, ann, bob, change_kinds, me, pr, refreshed, rev, setup};
use crate::{
    command::Command,
    consts::{CATCH_UP_DELAY, THREAD_READ_BUDGET},
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

fn loading_banners(state: &AppState) -> Vec<String> {
    state
        .banners
        .iter()
        .filter(|b| b.id.starts_with("loading:"))
        .map(|b| b.title.to_string())
        .collect()
}

/// A project with more active PRs than the read budget never reads more threads than the
/// budget in one refresh. The first load fills in on a catch-up refresh that comes on its
/// own, well before the interval, without a toast; a banner says more are coming meanwhile.
/// Only a PR opened afterwards is news.
#[tokio::test(start_paused = true)]
async fn large_project_reads_threads_within_the_budget() {
    let count = THREAD_READ_BUDGET + 10;
    let mut start = setup(vec![(CONTOSO, crowd(count))]);
    start.config.orgs[0].people.tracked_authors.clear();
    let mut h = ServiceHarness::start(start);
    let api = h.api(&org(CONTOSO));
    let budget = u32::try_from(THREAD_READ_BUDGET).unwrap();

    let state = refreshed(&mut h, CONTOSO, 1).await;
    let first = Instant::now();
    assert_eq!(api.calls(ApiCall::Threads), budget);
    assert_eq!(
        state.rows.len(),
        THREAD_READ_BUDGET,
        "10 PRs wait their turn"
    );
    let banners = loading_banners(&state);
    assert_eq!(banners, vec!["Loading 10 more pull requests in contoso"]);

    let state = refreshed(&mut h, CONTOSO, 2).await;
    assert!(
        first.elapsed() <= 2 * CATCH_UP_DELAY,
        "{:?}",
        first.elapsed()
    );
    assert_eq!(api.calls(ApiCall::Threads), 2 * budget);
    assert_eq!(state.rows.len(), count);
    assert!(state.rows.iter().all(|r| !r.stale && !r.unread));
    assert_eq!(loading_banners(&state), Vec::<String>::new());

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

/// A new PR whose first threads read failed used to show with no threads and no `NewPr`;
/// once read, all its comments toasted as new.
#[tokio::test(start_paused = true)]
async fn new_pr_whose_threads_fail_waits_for_a_read() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let api = h.api(&org(CONTOSO));
    let opened = pr(CONTOSO, 2);
    let thread = ThreadBuilder::human(1).comment(rev()).build();
    api.update(|o| {
        o.threads.insert(opened.key.clone(), vec![thread]);
        o.prs.push(opened.clone());
    });
    api.fail(ApiCall::Threads, AdoError::Network("flaky".into()));

    h.send(Command::RefreshNow);
    let state = refreshed(&mut h, CONTOSO, 2).await;
    assert_eq!(row_numbers(&state), BTreeSet::from([1]), "held back");

    api.clear_failures();
    h.advance(Duration::from_secs(3)).await;
    h.send(Command::RefreshNow);
    let state = refreshed(&mut h, CONTOSO, 3).await;

    let row = state.row(&opened.key).unwrap();
    assert_eq!(row.active_threads, 1);
    assert_eq!(change_kinds(&h), vec![(opened.key, ChangeKind::NewPr)]);
    h.stop().await;
}

/// state.json from before an empty people list meant everyone: the baseline holds only the
/// user's own PRs. The first refresh after the upgrade used to toast every colleague's PR.
#[tokio::test(start_paused = true)]
async fn upgrade_with_an_empty_people_list_rebaselines_silently() {
    let mine = pr_by(3, me());
    let mut start = everyone_setup(true);
    start.persisted.orgs.insert(
        org(CONTOSO),
        OrgPersisted {
            baseline: true,
            me: Some(me()),
            records: [(mine.key.clone(), record(mine, Vec::new()))].into(),
            ..OrgPersisted::default()
        },
    );
    let mut h = ServiceHarness::start(start);

    let state = refreshed(&mut h, CONTOSO, 1).await;
    h.settle().await;

    assert_eq!(row_numbers(&state), BTreeSet::from([1, 2, 3]));
    assert_eq!(change_kinds(&h), Vec::new());
    assert_eq!(h.notifier.ops(), Vec::new());
    h.stop().await;
}
