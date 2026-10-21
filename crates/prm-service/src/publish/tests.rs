use prm_auth::AuthAvailability;
use prm_domain::{
    auth::{AuthError, SettingsSection},
    scope::ScopeStats,
    testing::{org, ts},
    view::StatusSummary,
};

use super::*;
use crate::{errors::UiError, state::AuthStateView};

fn today() -> time::Date {
    ts(1_700_000_000).inner().date()
}

fn status(name: &str, health: OrgHealth, auth: AuthStateView) -> OrgStatus {
    OrgStatus {
        id: org(name),
        label: Arc::from(name),
        url: Arc::from(format!("https://dev.azure.com/{name}")),
        enabled: true,
        auth: AuthStatus {
            state: auth,
            method: Some(AuthMethod::AzureCli),
            me: None,
            tenant: None,
            availability: AuthAvailability::default(),
        },
        health,
        last_ok: None,
        next_due: None,
        rate: RateHeadroom::default(),
        scope: ScopeStats::default(),
        scope_issues: Arc::from([]),
        counts: StatusSummary::default(),
        catalog: None,
        catalog_error: None,
        last_error: None,
        pat_expires_on: None,
    }
}

fn failed(transient: bool) -> OrgHealth {
    let mut error = UiError::new("x");
    error.transient = transient;
    OrgHealth::Failed {
        error,
        since: ts(0),
        retry_at: None,
    }
}

fn ids(orgs: &[OrgStatus], notifications: &NotificationsStatus) -> Vec<String> {
    let input = BannerInput {
        orgs,
        issues: &[],
        notifications,
        autostart: &AutostartStatus::default(),
        state_not_saved: false,
        today: today(),
    };
    banners(&input).iter().map(|b| b.id.to_string()).collect()
}

fn quiet() -> NotificationsStatus {
    NotificationsStatus {
        enabled: true,
        capability: NotifierCapability::Enabled,
        ..NotificationsStatus::default()
    }
}

#[test]
fn banner_auth_per_org_needing_sign_in() {
    let error = UiError::from(&AuthError::AzNotLoggedIn { tenant: None });
    let orgs = [
        status("a", OrgHealth::Ok, AuthStateView::NeedsSignIn { error }),
        status("b", OrgHealth::Ok, AuthStateView::SignedIn),
    ];
    assert_eq!(ids(&orgs, &quiet()), ["auth:a"]);
}

#[test]
fn banner_offline_only_when_every_org_failed_transiently() {
    let all_down = [
        status("a", failed(true), AuthStateView::SignedIn),
        status("b", failed(true), AuthStateView::SignedIn),
    ];
    assert_eq!(ids(&all_down, &quiet()), ["offline"]);
    let one_up = [
        status("a", failed(true), AuthStateView::SignedIn),
        status("b", OrgHealth::Ok, AuthStateView::SignedIn),
    ];
    assert_eq!(ids(&one_up, &quiet()), Vec::<String>::new());
    let permanent = [status("a", failed(false), AuthStateView::SignedIn)];
    assert_eq!(ids(&permanent, &quiet()), Vec::<String>::new());
}

#[test]
fn banner_pat_expiry_within_7_days() {
    let mut org_status = status("a", OrgHealth::Ok, AuthStateView::SignedIn);
    org_status.auth.method = Some(AuthMethod::Pat);
    org_status.pat_expires_on = today().checked_add(time::Duration::days(7));
    assert_eq!(ids(&[org_status.clone()], &quiet()), ["pat-expiry:a"]);
    org_status.pat_expires_on = today().checked_add(time::Duration::days(8));
    assert_eq!(ids(&[org_status], &quiet()), Vec::<String>::new());
}

#[test]
fn banner_notify_policy_when_blocked() {
    let orgs = [status("a", OrgHealth::Ok, AuthStateView::SignedIn)];
    let blocked = NotificationsStatus {
        capability: NotifierCapability::DisabledByPolicy,
        ..quiet()
    };
    assert_eq!(ids(&orgs, &blocked), ["notify-policy"]);
    let off = NotificationsStatus {
        enabled: false,
        ..blocked
    };
    assert_eq!(ids(&orgs, &off), Vec::<String>::new());
}

#[test]
fn banner_config_errors_and_autostart() {
    let issues = [ConfigIssue::new(Severity::Error, "orgs[0].url", "bad")];
    let autostart = AutostartStatus {
        desired: true,
        actual: Some(AutostartState::DisabledByUser),
        error: None,
    };
    let input = BannerInput {
        orgs: &[],
        issues: &issues,
        notifications: &quiet(),
        autostart: &autostart,
        state_not_saved: false,
        today: today(),
    };
    let found = banners(&input);
    let ids: Vec<&str> = found.iter().map(|b| b.id.as_ref()).collect();
    assert_eq!(ids, ["config", "autostart-disabled"]);
    assert_eq!(
        found[0].action,
        Some(UserAction::OpenSettings(SettingsSection::General))
    );
}

#[test]
fn banner_state_not_saved_stays_until_a_save_succeeds() {
    let (notifications, autostart) = (quiet(), AutostartStatus::default());
    let input = |state_not_saved| BannerInput {
        orgs: &[],
        issues: &[],
        notifications: &notifications,
        autostart: &autostart,
        state_not_saved,
        today: today(),
    };

    let failed = banners(&input(true));
    let saved = banners(&input(false));

    let ids: Vec<&str> = failed.iter().map(|b| b.id.as_ref()).collect();
    assert_eq!(ids, ["state-not-saved"]);
    assert!(!failed[0].dismissible);
    assert_eq!(failed[0].title.as_ref(), text::STATE_NOT_SAVED);
    assert_eq!(saved, []);
}
