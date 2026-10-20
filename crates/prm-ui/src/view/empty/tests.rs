use prm_config::{Config, OrgProfile};
use prm_service::UiError;

use super::*;
use crate::testing::{org_status, sample_rows, state_with};

fn empty_state(orgs: Vec<prm_service::OrgStatus>, config: Config) -> AppState {
    AppState {
        orgs: orgs.into(),
        config: Arc::new(config),
        ..AppState::default()
    }
}

fn kind(
    state: &AppState,
    visible: usize,
    text: &str,
    status: StatusFilter,
    unread_only: bool,
) -> Option<EmptyKind> {
    empty_kind(&EmptyInputs {
        state,
        visible,
        text,
        status,
        unread_only,
    })
}

fn watching_me() -> Config {
    Config {
        orgs: vec![OrgProfile {
            id: Some(prm_domain::testing::org("contoso")),
            include_me: true,
            ..OrgProfile::default()
        }],
        ..Config::default()
    }
}

#[test]
fn no_rows_while_checking_sign_in_is_loading() {
    let state = empty_state(
        vec![org_status("contoso", AuthStateView::Checking)],
        watching_me(),
    );
    let Some(EmptyKind::Loading { org, .. }) = kind(&state, 0, "", StatusFilter::All, false) else {
        panic!("expected loading");
    };
    assert_eq!(org.as_ref(), "contoso");
}

#[test]
fn no_rows_with_every_org_signed_out_asks_to_sign_in() {
    let needs = AuthStateView::NeedsSignIn {
        error: UiError::new("Signed out"),
    };
    let state = empty_state(vec![org_status("contoso", needs)], watching_me());
    assert_eq!(
        kind(&state, 0, "", StatusFilter::All, false),
        Some(EmptyKind::SignedOut)
    );
}

#[test]
fn no_rows_and_nobody_tracked_asks_for_people() {
    let state = empty_state(
        vec![org_status("contoso", AuthStateView::SignedIn)],
        Config::default(),
    );
    assert_eq!(
        kind(&state, 0, "", StatusFilter::All, false),
        Some(EmptyKind::NobodyTracked)
    );
    let tracking = empty_state(
        vec![org_status("contoso", AuthStateView::SignedIn)],
        watching_me(),
    );
    assert_eq!(
        kind(&tracking, 0, "", StatusFilter::All, false),
        Some(EmptyKind::NoActivePrs)
    );
}

#[test]
fn filtered_out_rows_explain_which_filter_hid_them() {
    let state = state_with(sample_rows());
    assert_eq!(kind(&state, 2, "", StatusFilter::All, false), None);
    assert_eq!(
        kind(&state, 0, " zzz ", StatusFilter::Reviewed, true),
        Some(EmptyKind::NoMatch {
            query: "zzz".to_owned()
        })
    );
    assert_eq!(
        kind(&state, 0, "", StatusFilter::Reviewed, true),
        Some(EmptyKind::StatusEmpty(PrStatus::Reviewed))
    );
    assert_eq!(
        kind(&state, 0, "", StatusFilter::All, true),
        Some(EmptyKind::NothingChanged)
    );
}
