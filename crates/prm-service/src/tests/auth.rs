//! Auth pause and recovery: probes, sign-in, sign-out, PATs.

use std::time::Duration;

use prm_ado::{
    AdoError,
    testing::{ApiCall, token},
};
use prm_domain::{
    OrgId, Secret,
    auth::{AuthError, AuthMethod, AzBroker, SignInMethod},
    notify::NotificationKind,
    testing::org,
};
use tokio::time::Instant;

use super::support::{CONTOSO, list_calls, pr, refreshed, setup};
use crate::{
    command::Command,
    event::AppEvent,
    state::{AppState, AuthStateView, OrgHealth, PauseReason},
    testing::ServiceHarness,
};

fn not_logged_in() -> AuthError {
    AuthError::AzNotLoggedIn { tenant: None }
}

fn auth_view(state: &AppState) -> AuthStateView {
    state.org(&org(CONTOSO)).unwrap().auth.state.clone()
}

fn needs_sign_in(state: &AppState) -> bool {
    matches!(auth_view(state), AuthStateView::NeedsSignIn { .. })
}

fn valid_pat() -> Secret {
    let mut value = "a".repeat(76);
    value.push_str("AZDOxxxx");
    Secret::new(value)
}

/// Starts with sign-in broken: the first refresh pauses the org.
async fn paused_harness() -> ServiceHarness {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    let auth = h.auth.auth(&org(CONTOSO));
    auth.set_token(Err(not_logged_in()));
    auth.set_sign_in(Err(not_logged_in()));
    h.wait_for("paused for sign-in", needs_sign_in).await;
    h
}

fn fix_sign_in(h: &ServiceHarness, id: &OrgId) {
    let auth = h.auth.auth(id);
    auth.set_sign_in(Ok(AuthMethod::AzureCli));
    auth.set_token(Ok(token("t")));
}

#[tokio::test(start_paused = true)]
async fn auth_failure_pauses_and_probes_every_15m() {
    let mut h = paused_harness().await;
    let paused_at = Instant::now();
    let state = h.snapshot();
    let status = state.org(&org(CONTOSO)).unwrap();
    assert_eq!(
        status.health,
        OrgHealth::Paused {
            reason: PauseReason::Auth
        }
    );
    assert_eq!(h.notifier.ops(), Vec::new(), "no toast at startup");
    let auth = h.auth.auth(&org(CONTOSO));
    let probes = auth.sign_ins();

    h.advance(Duration::from_mins(14)).await;
    assert_eq!(auth.sign_ins(), probes, "no probe before 15 min");
    fix_sign_in(&h, &org(CONTOSO));
    h.wait_for("signed in", |s| auth_view(s) == AuthStateView::SignedIn)
        .await;

    assert_eq!(paused_at.elapsed(), Duration::from_mins(15));
    assert_eq!(auth.sign_ins(), probes + 1);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn probe_success_resumes_immediately() {
    let mut h = paused_harness().await;
    assert_eq!(list_calls(&h, CONTOSO), 0);
    fix_sign_in(&h, &org(CONTOSO));
    h.wait_for("signed in", |s| auth_view(s) == AuthStateView::SignedIn)
        .await;
    let probed_at = Instant::now();

    refreshed(&mut h, CONTOSO, 1).await;

    assert_eq!(probed_at.elapsed(), Duration::ZERO);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn probe_on_ui_shown_debounced() {
    let h = paused_harness().await;
    let auth = h.auth.auth(&org(CONTOSO));
    let probes = auth.sign_ins();
    let show = || Command::UiPresence {
        visible: true,
        focused: true,
    };
    let hide = || Command::UiPresence {
        visible: false,
        focused: false,
    };

    h.send(show());
    h.settle().await;
    assert_eq!(auth.sign_ins(), probes + 1, "shown: probe now");
    h.send(hide());
    h.send(show());
    h.settle().await;
    assert_eq!(auth.sign_ins(), probes + 1, "shown again within 60 s");
    h.send(hide());
    h.advance(Duration::from_secs(61)).await;
    h.send(show());
    h.settle().await;
    assert_eq!(auth.sign_ins(), probes + 2);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn auth_lost_after_sign_in_toasts_once() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let auth = h.auth.auth(&org(CONTOSO));
    auth.set_token(Err(not_logged_in()));
    auth.set_sign_in(Err(not_logged_in()));

    h.send(Command::RefreshNow);
    let notifier = h.notifier.clone();
    h.wait_for("auth toast", move |_| !notifier.shown().is_empty())
        .await;
    let probes = auth.clone();
    h.wait_for("two failed probes", move |_| probes.sign_ins() >= 2)
        .await;
    h.settle().await;

    let shown = h.notifier.shown();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].kind, NotificationKind::Auth);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn sign_in_az_login_resumes_refresh() {
    let mut h = paused_harness().await;
    fix_sign_in(&h, &org(CONTOSO));
    let method = SignInMethod::AzLogin {
        broker: AzBroker::Default,
    };

    h.send(Command::SignIn {
        org: org(CONTOSO),
        method,
    });
    refreshed(&mut h, CONTOSO, 1).await;

    let finished = h.events().into_iter().any(|e| {
        e == AppEvent::SignInFinished {
            org: org(CONTOSO),
            result: Ok(AuthMethod::AzureCli),
        }
    });
    assert!(finished);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn sign_out_marks_stale_and_blocks_refresh() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;

    h.send(Command::SignOut { org: org(CONTOSO) });
    let state = h
        .wait_for("signed out", |s| auth_view(s) == AuthStateView::SignedOut)
        .await;
    assert!(state.rows.iter().all(|r| r.stale));
    h.advance(Duration::from_mins(20)).await;

    assert_eq!(list_calls(&h, CONTOSO), 1);
    let repo = h.repo.clone();
    h.wait_for("signed-out state saved", move |_| {
        repo.last_state()
            .is_some_and(|saved| saved.orgs[&org(CONTOSO)].signed_out)
    })
    .await;
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn set_pat_validates_before_storing() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let checks = h.api(&org(CONTOSO)).calls(ApiCall::ConnectionData);

    h.send(Command::SetPat {
        org: org(CONTOSO),
        pat: Secret::new("short"),
    });
    let events = h.events.clone();
    h.wait_for("PAT result", move |_| {
        events
            .events()
            .iter()
            .any(|e| matches!(e, AppEvent::PatResult { .. }))
    })
    .await;

    assert_eq!(
        h.auth.auth(&org(CONTOSO)).stored_pats(),
        Vec::<String>::new()
    );
    assert_eq!(h.api(&org(CONTOSO)).calls(ApiCall::ConnectionData), checks);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn set_pat_401_not_stored() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    h.api(&org(CONTOSO)).fail(
        ApiCall::ConnectionData,
        AdoError::Auth(AuthError::Unauthorized { code: None }),
    );

    h.send(Command::SetPat {
        org: org(CONTOSO),
        pat: valid_pat(),
    });
    let events = h.events.clone();
    h.wait_for("PAT result", move |_| {
        events
            .events()
            .iter()
            .any(|e| matches!(e, AppEvent::PatResult { .. }))
    })
    .await;

    let result = h.events().into_iter().find_map(|e| {
        if let AppEvent::PatResult { result, .. } = e {
            Some(result)
        } else {
            None
        }
    });
    let error = result.unwrap().unwrap_err();
    assert_eq!(error.title.as_ref(), AuthError::PatInvalid.to_string());
    assert_eq!(
        h.auth.auth(&org(CONTOSO)).stored_pats(),
        Vec::<String>::new()
    );
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn set_pat_stores_and_refreshes() {
    let mut h = paused_harness().await;
    h.auth.auth(&org(CONTOSO)).set_token(Ok(token("pat")));

    h.send(Command::SetPat {
        org: org(CONTOSO),
        pat: valid_pat(),
    });
    refreshed(&mut h, CONTOSO, 1).await;

    assert_eq!(h.auth.auth(&org(CONTOSO)).stored_pats().len(), 1);
    h.stop().await;
}
