//! Notification cycles: baseline silence, coalescing, focus and pause gates, save-before-toast.

use std::{sync::Arc, time::Duration};

use prm_domain::{OrgId, notify::NotificationKind, testing::org};

use super::support::{CONTOSO, FABRIKAM, pr, refreshed, setup};
use crate::{command::Command, testing::ServiceHarness};

/// Adds Ann's PR `n` to the fake org.
fn add_pr(h: &ServiceHarness, name: &str, n: i32) {
    h.api(&org(name)).update(|o| o.prs.push(pr(name, n)));
}

async fn wait_toasts(h: &mut ServiceHarness, count: usize) {
    let notifier = h.notifier.clone();
    h.wait_for(&format!("{count} toasts"), move |_| {
        notifier.shown().len() >= count
    })
    .await;
}

#[tokio::test(start_paused = true)]
async fn first_run_baseline_no_toasts_no_unread() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1), pr(CONTOSO, 2)])]));

    let state = refreshed(&mut h, CONTOSO, 1).await;
    h.settle().await;

    assert_eq!(state.rows.len(), 2);
    assert!(state.rows.iter().all(|r| !r.unread));
    assert_eq!(h.notifier.ops(), Vec::new());
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn new_pr_after_baseline_toasts_and_is_unread() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    add_pr(&h, CONTOSO, 2);

    h.send(Command::RefreshNow);
    wait_toasts(&mut h, 1).await;

    let shown = h.notifier.shown();
    assert_eq!(shown[0].kind, NotificationKind::Pr);
    assert_eq!(shown[0].tag, pr(CONTOSO, 2).key.tag());
    let state = h.snapshot();
    let row = state.row(&pr(CONTOSO, 2).key).unwrap();
    assert!(row.unread);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn notifications_coalesced_per_cycle_across_orgs() {
    let mut h = ServiceHarness::start(setup(vec![
        (CONTOSO, vec![pr(CONTOSO, 1)]),
        (FABRIKAM, vec![pr(FABRIKAM, 1)]),
    ]));
    refreshed(&mut h, CONTOSO, 1).await;
    refreshed(&mut h, FABRIKAM, 1).await;
    add_pr(&h, CONTOSO, 2);
    add_pr(&h, FABRIKAM, 2);

    h.send(Command::RefreshNow);
    wait_toasts(&mut h, 2).await;

    let attributions: Vec<String> = h
        .notifier
        .shown()
        .iter()
        .filter_map(|n| n.attribution.clone())
        .collect();
    assert_eq!(attributions.len(), 2);
    assert!(
        attributions.iter().any(|a| a.contains(CONTOSO)),
        "{attributions:?}"
    );
    assert!(
        attributions.iter().any(|a| a.contains(FABRIKAM)),
        "{attributions:?}"
    );
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn toasts_suppressed_when_focused() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    h.send(Command::UiPresence {
        visible: true,
        focused: true,
    });
    add_pr(&h, CONTOSO, 2);

    h.send(Command::RefreshNow);
    let state = h
        .wait_for("suppressed", |s| s.notifications.last_suppressed > 0)
        .await;

    assert!(state.row(&pr(CONTOSO, 2).key).unwrap().unread);
    assert_eq!(h.notifier.shown(), Vec::new());
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn paused_notifications_suppressed_until_expiry() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let until = h.clock.now_plus(Duration::from_mins(30));
    h.send(Command::SetNotificationsPaused { until: Some(until) });
    add_pr(&h, CONTOSO, 2);

    h.send(Command::RefreshNow);
    h.wait_for("suppressed", |s| s.notifications.last_suppressed > 0)
        .await;
    assert_eq!(h.notifier.shown(), Vec::new());
    let state = h
        .wait_for("pause expired", |s| s.notifications.paused_until.is_none())
        .await;

    assert!(h.clock.now_plus(Duration::ZERO) >= until);
    assert!(state.notifications.enabled);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn persist_happens_before_toasts() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let repo = h.repo.clone();
    let id: OrgId = org(CONTOSO);
    h.notifier.set_probe(Arc::new(move || {
        repo.last_state()
            .and_then(|s| s.orgs.get(&id).map(|o| o.records.len()))
            .unwrap_or(0)
    }));
    add_pr(&h, CONTOSO, 2);

    h.send(Command::RefreshNow);
    wait_toasts(&mut h, 1).await;

    assert_eq!(
        h.notifier.probes(),
        vec![2],
        "state with the new PR was saved first"
    );
    h.stop().await;
}
