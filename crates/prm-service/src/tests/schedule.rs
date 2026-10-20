//! Startup and scheduling: first refresh, interval with jitter, no bursts, resume after sleep.

use std::{collections::HashMap, time::Duration};

use prm_ado::AdoApi;
use prm_config::{OrgPersisted, OrgProfile, PersistedState, normalize};
use prm_domain::testing::{BASE_TIME_SECS, org, record, ts};
use tokio::time::Instant;

use super::support::{CONTOSO, FABRIKAM, config, list_calls, pr, profile, refreshed, setup};
use crate::{handle::initial_state, state::OrgHealth, testing::ServiceHarness};

#[tokio::test(start_paused = true)]
async fn startup_refreshes_each_enabled_org() {
    let mut start = setup(vec![
        (CONTOSO, vec![pr(CONTOSO, 1)]),
        (FABRIKAM, vec![pr(FABRIKAM, 2)]),
    ]);
    start.config.orgs.push(OrgProfile {
        enabled: false,
        ..profile("northwind")
    });
    let mut h = ServiceHarness::start(start);

    refreshed(&mut h, CONTOSO, 1).await;
    let state = refreshed(&mut h, FABRIKAM, 1).await;

    assert_eq!(state.rows.len(), 2);
    let northwind = state.org(&org("northwind")).unwrap();
    assert_eq!(northwind.health, OrgHealth::Disabled);
    assert_eq!(h.api(&org("northwind")).request_count(), 0);
    assert!(state.show_org_column);
    h.stop().await;
}

#[test]
fn startup_paints_cached_rows_before_first_refresh() {
    let (config, _) = normalize(config(&[CONTOSO]));
    let cached = pr(CONTOSO, 7);
    let mut persisted = PersistedState::default();
    persisted.orgs.insert(
        org(CONTOSO),
        OrgPersisted {
            baseline: true,
            records: [(cached.key.clone(), record(cached.clone(), Vec::new()))].into(),
            ..OrgPersisted::default()
        },
    );

    let state = initial_state(&config, &persisted, &HashMap::new(), ts(BASE_TIME_SECS));

    assert_eq!(state.rows.len(), 1);
    assert_eq!(state.rows[0].key, cached.key);
    assert!(!state.first_run);
    assert_eq!(state.org(&org(CONTOSO)).unwrap().health, OrgHealth::Idle);
}

#[test]
fn startup_without_enabled_orgs_is_first_run() {
    let state = initial_state(
        &prm_config::Config::default(),
        &PersistedState::default(),
        &HashMap::new(),
        ts(BASE_TIME_SECS),
    );
    assert!(state.first_run);
}

#[tokio::test(start_paused = true)]
async fn interval_respected_with_jitter() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let first = Instant::now();

    refreshed(&mut h, CONTOSO, 2).await;

    let gap = first.elapsed();
    let window = Duration::from_secs(270)..=Duration::from_secs(330);
    assert!(window.contains(&gap), "{gap:?}");
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn long_stall_fires_once_no_burst() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;

    h.advance(Duration::from_hours(1)).await;
    refreshed(&mut h, CONTOSO, 2).await;
    h.settle().await;

    assert_eq!(list_calls(&h, CONTOSO), 2);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn resume_after_sleep_refreshes_within_60s() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;

    h.clock.jump(Duration::from_hours(2));
    let resumed = Instant::now();
    refreshed(&mut h, CONTOSO, 2).await;

    let waited = resumed.elapsed();
    assert!(waited <= Duration::from_secs(65), "{waited:?}");
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn startup_delay_postpones_first_refresh() {
    let mut start = setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]);
    start.startup_delay = Duration::from_secs(30);
    let began = Instant::now();
    let mut h = ServiceHarness::start(start);

    refreshed(&mut h, CONTOSO, 1).await;

    assert_eq!(began.elapsed(), Duration::from_secs(30));
    h.stop().await;
}
