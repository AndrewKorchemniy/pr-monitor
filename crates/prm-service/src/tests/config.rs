//! `ApplyConfig`: reclassify-only changes, rebaselines, added and removed orgs, autostart.

use std::time::Duration;

use prm_domain::{PrStatus, testing::org};

use super::support::{
    CONTOSO, FABRIKAM, bob, change_kinds, fake_org, list_calls, me, pr, profile, refreshed, rev,
    setup,
};
use crate::{
    command::Command,
    event::AppEvent,
    ports::AutostartState,
    state::{AuthStateView, OrgHealth},
    testing::ServiceHarness,
};

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
    h.advance(Duration::from_mins(20)).await;

    assert!(state.first_run);
    assert_eq!(list_calls(&h, CONTOSO), 1);
    h.stop().await;
}

const TENANT: &str = "72f988bf-86f1-41af-91ab-2d7cd011db47";

/// Editing a disabled org's authors or tenant used to leave it due but never refreshed: the
/// actor woke and published in a busy loop until restart.
#[tokio::test(start_paused = true)]
async fn disabled_org_edits_stay_idle() {
    let mut start = setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]);
    start.config.orgs[0].enabled = false;
    let h = ServiceHarness::start(start);
    h.settle().await;
    let mut config = (*h.snapshot().config).clone();
    config.orgs[0].people.tracked_authors.push(bob());
    config.orgs[0].tenant = TENANT.into();
    let wakes = h.events.wakes();

    h.send(Command::ApplyConfig(Box::new(config)));
    h.advance(Duration::from_secs(10)).await;

    let woke = h.events.wakes() - wakes;
    assert!(woke < 10, "{woke} wakes");
    let state = h.snapshot();
    let status = state.org(&org(CONTOSO)).unwrap();
    assert_eq!(status.auth.state, AuthStateView::Disabled);
    assert_eq!(status.health, OrgHealth::Disabled);
    assert_eq!(list_calls(&h, CONTOSO), 0);
    h.stop().await;
}

/// Changing the tenant and widening the authors in one save used to skip the rebaseline, so
/// every PR that came into view toasted as new.
#[tokio::test(start_paused = true)]
async fn auth_and_authors_change_together_rebaselines() {
    let bobs = prm_domain::testing::PrBuilder::new(prm_domain::testing::key(CONTOSO, 1, 5))
        .author(bob())
        .build();
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1), bobs])]));
    let state = refreshed(&mut h, CONTOSO, 1).await;

    let mut config = (*state.config).clone();
    config.orgs[0].tenant = TENANT.into();
    config.orgs[0].people.tracked_authors.push(bob());
    h.send(Command::ApplyConfig(Box::new(config)));
    let state = refreshed(&mut h, CONTOSO, 2).await;
    h.settle().await;

    assert_eq!(state.rows.len(), 2);
    assert_eq!(change_kinds(&h), Vec::new());
    assert_eq!(h.notifier.ops(), Vec::new());
    h.stop().await;
}

/// Changing a signed-out org's tenant used to resume it into a sign-in error banner.
#[tokio::test(start_paused = true)]
async fn signed_out_org_stays_signed_out_after_auth_change() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    let state = refreshed(&mut h, CONTOSO, 1).await;
    h.send(Command::SignOut { org: org(CONTOSO) });
    h.wait_for("signed out", |s| {
        s.org(&org(CONTOSO)).unwrap().auth.state == AuthStateView::SignedOut
    })
    .await;

    let mut config = (*state.config).clone();
    config.orgs[0].tenant = TENANT.into();
    h.send(Command::ApplyConfig(Box::new(config)));
    h.advance(Duration::from_mins(20)).await;

    let state = h.snapshot();
    let status = state.org(&org(CONTOSO)).unwrap();
    assert_eq!(status.auth.state, AuthStateView::SignedOut);
    assert_eq!(list_calls(&h, CONTOSO), 1);
    assert!(
        state.banners.iter().all(|b| b.org.is_none()),
        "{:?}",
        state.banners
    );
    h.stop().await;
}

/// The signed-in identity used to be cached across auth changes, so the old account's PRs
/// were tracked and the user's own comments toasted.
#[tokio::test(start_paused = true)]
async fn auth_change_refetches_identity() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    let state = refreshed(&mut h, CONTOSO, 1).await;
    assert_eq!(state.org(&org(CONTOSO)).unwrap().auth.me, Some(me()));
    h.api(&org(CONTOSO)).update(|o| o.me = Some(bob()));

    let mut config = (*state.config).clone();
    config.orgs[0].tenant = TENANT.into();
    h.send(Command::ApplyConfig(Box::new(config)));
    let state = refreshed(&mut h, CONTOSO, 2).await;

    assert_eq!(state.org(&org(CONTOSO)).unwrap().auth.me, Some(bob()));
    h.stop().await;
}
