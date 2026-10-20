//! Manual refresh and cancellation.

use std::time::Duration;

use prm_ado::{AdoError, testing::ApiCall};
use prm_domain::testing::org;

use super::support::{CONTOSO, bob, change_kinds, list_calls, pr, refreshed, setup};
use crate::{command::Command, state::OrgHealth, testing::ServiceHarness};

#[tokio::test(start_paused = true)]
async fn manual_refresh_inflight_noop() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    h.api(&org(CONTOSO))
        .set_delay(Some(Duration::from_secs(10)));

    h.send(Command::RefreshNow);
    h.settle().await;
    h.advance(Duration::from_secs(3)).await;
    h.send(Command::RefreshNow);
    h.settle().await;

    assert_eq!(list_calls(&h, CONTOSO), 2);
    refreshed(&mut h, CONTOSO, 2).await;
    assert_eq!(list_calls(&h, CONTOSO), 2);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn manual_refresh_debounced_2s() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;

    h.send(Command::RefreshNow);
    refreshed(&mut h, CONTOSO, 2).await;
    h.send(Command::RefreshNow);
    h.settle().await;
    assert_eq!(list_calls(&h, CONTOSO), 2, "within 2 s: ignored");

    h.advance(Duration::from_secs(2)).await;
    h.send(Command::RefreshOrg(org(CONTOSO)));
    refreshed(&mut h, CONTOSO, 3).await;
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn manual_refresh_resets_failure_backoff() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let api = h.api(&org(CONTOSO));
    api.fail(
        ApiCall::ActivePullRequests,
        AdoError::Network("down".into()),
    );

    h.send(Command::RefreshNow);
    refreshed(&mut h, CONTOSO, 2).await;
    h.advance(Duration::from_secs(3)).await;
    h.send(Command::RefreshNow);
    let state = refreshed(&mut h, CONTOSO, 3).await;

    let OrgHealth::Failed {
        since, retry_at, ..
    } = &state.org(&org(CONTOSO)).unwrap().health
    else {
        panic!("expected Failed");
    };
    let backoff = retry_at.unwrap().since(*since);
    assert!(
        backoff <= Duration::from_secs(66),
        "first-failure backoff, got {backoff:?}"
    );
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn config_change_cancels_and_drops_stale_generation() {
    let bobs = prm_domain::testing::PrBuilder::new(prm_domain::testing::key(CONTOSO, 1, 2))
        .author(bob())
        .build();
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1), bobs])]));
    let state = refreshed(&mut h, CONTOSO, 1).await;
    assert_eq!(state.rows.len(), 1);
    h.api(&org(CONTOSO))
        .set_delay(Some(Duration::from_secs(30)));
    h.send(Command::RefreshNow);
    h.settle().await;

    let mut config = (*state.config).clone();
    config.orgs[0].people.tracked_authors.push(bob());
    h.send(Command::ApplyConfig(Box::new(config)));
    let state = h.wait_for("bob's PR", |s| s.rows.len() == 2).await;

    assert_eq!(
        list_calls(&h, CONTOSO),
        3,
        "the cancelled refresh is not retried"
    );
    assert_eq!(change_kinds(&h), Vec::new(), "a rebaseline reports nothing");
    assert!(!state.rows.iter().any(|r| r.stale));
    h.stop().await;
}
