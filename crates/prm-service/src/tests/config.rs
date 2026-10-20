//! `ApplyConfig`: reclassify-only changes, rebaselines, added and removed orgs, autostart.

use prm_domain::{PrStatus, testing::org};

use super::support::{
    CONTOSO, FABRIKAM, bob, change_kinds, fake_org, list_calls, pr, profile, refreshed, rev, setup,
};
use crate::{command::Command, event::AppEvent, ports::AutostartState, testing::ServiceHarness};

#[tokio::test(start_paused = true)]
async fn classify_only_config_change_no_events() {
    let reviewed = prm_domain::testing::PrBuilder::new(pr(CONTOSO, 1).key)
        .author(super::support::ann())
        .reviewer(rev(), 10)
        .build();
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![reviewed])]));
    let state = refreshed(&mut h, CONTOSO, 1).await;
    assert_eq!(state.rows[0].status, PrStatus::Reviewed);

    let mut config = (*state.config).clone();
    config.orgs[0].people.reviewers = vec![bob()];
    h.send(Command::ApplyConfig(Box::new(config)));
    let state = h
        .wait_for("reclassified", |s| {
            s.rows
                .first()
                .is_some_and(|r| r.status == PrStatus::NeedsReview)
        })
        .await;

    assert_eq!(list_calls(&h, CONTOSO), 1, "no refresh needed");
    assert_eq!(change_kinds(&h), Vec::new());
    assert_eq!(state.rows.len(), 1);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn scope_change_rebaselines() {
    let bobs = prm_domain::testing::PrBuilder::new(prm_domain::testing::key(CONTOSO, 1, 5))
        .author(bob())
        .build();
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1), bobs])]));
    let state = refreshed(&mut h, CONTOSO, 1).await;

    let mut config = (*state.config).clone();
    config.orgs[0].people.tracked_authors.push(bob());
    h.send(Command::ApplyConfig(Box::new(config)));
    let state = refreshed(&mut h, CONTOSO, 2).await;

    assert_eq!(state.rows.len(), 2);
    assert_eq!(change_kinds(&h), Vec::new());
    h.settle().await;
    assert_eq!(h.notifier.ops(), Vec::new());
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn org_added_and_removed_via_apply_config() {
    let mut start = setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]);
    start
        .orgs
        .push((org(FABRIKAM), fake_org(1, vec![pr(FABRIKAM, 9)])));
    let mut h = ServiceHarness::start(start);
    let state = refreshed(&mut h, CONTOSO, 1).await;

    let mut config = (*state.config).clone();
    config.orgs.push(profile(FABRIKAM));
    h.send(Command::ApplyConfig(Box::new(config.clone())));
    let state = refreshed(&mut h, FABRIKAM, 1).await;
    assert_eq!(state.rows.len(), 2);
    assert!(state.show_org_column);

    config.orgs.remove(0);
    h.send(Command::ApplyConfig(Box::new(config)));
    let state = h
        .wait_for("contoso removed", |s| s.org(&org(CONTOSO)).is_none())
        .await;

    assert_eq!(state.rows.len(), 1);
    assert_eq!(state.rows[0].key.org, org(FABRIKAM));
    assert!(!state.show_org_column);
    let events = h.events.clone();
    h.wait_for("config applied", move |_| {
        events
            .events()
            .iter()
            .filter(|e| matches!(e, AppEvent::ConfigApplied { saved: true, .. }))
            .count()
            == 2
    })
    .await;
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn autostart_applied_on_config_change() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    let state = refreshed(&mut h, CONTOSO, 1).await;

    let mut config = (*state.config).clone();
    config.ui.launch_at_login = true;
    h.send(Command::ApplyConfig(Box::new(config)));
    let state = h
        .wait_for("autostart on", |s| {
            s.autostart.actual == Some(AutostartState::Enabled)
        })
        .await;

    assert!(state.autostart.desired);
    assert_eq!(h.autostart.sets(), vec![true]);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn disabling_org_hides_rows_and_stops_refresh() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    let state = refreshed(&mut h, CONTOSO, 1).await;

    let mut config = (*state.config).clone();
    config.orgs[0].enabled = false;
    h.send(Command::ApplyConfig(Box::new(config)));
    let state = h.wait_for("rows hidden", |s| s.rows.is_empty()).await;
    h.advance(std::time::Duration::from_mins(20)).await;

    assert!(state.first_run);
    assert_eq!(list_calls(&h, CONTOSO), 1);
    h.stop().await;
}
