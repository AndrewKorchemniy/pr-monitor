use prm_domain::{
    auth::{AuthError, AuthMethod, AzBroker, SignInMethod},
    testing::ts,
};

use super::*;

const AZ: AuthMethod = AuthMethod::AzureCli;

fn needs_sign_in() -> AuthError {
    AuthError::AzNotLoggedIn { tenant: None }
}

fn signed_in() -> AuthState {
    AuthState::SignedIn { method: AZ }
}

fn step(state: &AuthState, input: AuthInput) -> (AuthState, Vec<AuthEffect>) {
    transition(state, input, ts(100))
}

#[test]
fn auth_signed_in_to_needs_sign_in_notifies_once() {
    let (state, effects) = step(&signed_in(), AuthInput::AuthFailed(needs_sign_in()));
    assert_eq!(
        state,
        AuthState::NeedsSignIn {
            error: needs_sign_in()
        }
    );
    assert_eq!(
        effects,
        vec![AuthEffect::PauseRefresh, AuthEffect::NotifyProblem]
    );

    let (again, effects) = step(&state, AuthInput::AuthFailed(needs_sign_in()));
    assert_eq!(again, state);
    assert_eq!(effects, vec![AuthEffect::PauseRefresh]);
}

#[test]
fn auth_startup_failure_no_toast() {
    for start in [AuthState::Unknown, AuthState::Checking] {
        let (state, effects) = step(&start, AuthInput::AuthFailed(needs_sign_in()));
        assert!(matches!(state, AuthState::NeedsSignIn { .. }), "{start:?}");
        assert_eq!(effects, vec![AuthEffect::PauseRefresh], "{start:?}");
    }
}

#[test]
fn auth_blocked_error_goes_to_blocked() {
    let error = AuthError::ConditionalAccessBlocked("AADSTS53003".into());
    let (state, _) = step(&signed_in(), AuthInput::AuthFailed(error.clone()));
    assert_eq!(state, AuthState::Blocked { error });
}

#[test]
fn auth_sign_in_cancel_restores_previous() {
    let before = AuthState::NeedsSignIn {
        error: needs_sign_in(),
    };
    let method = SignInMethod::AzLogin {
        broker: AzBroker::Default,
    };
    let (signing, effects) = step(&before, AuthInput::SignInRequested(method));
    assert!(matches!(signing, AuthState::SigningIn { .. }));
    assert_eq!(effects, vec![AuthEffect::StartSignIn(method)]);

    let restore = Box::new(before.clone());
    let (state, effects) = step(&signing, AuthInput::SignInCancelled { restore });
    assert_eq!(state, before);
    assert_eq!(effects, Vec::new());
}

#[test]
fn auth_transient_error_keeps_state() {
    let (state, effects) = step(&signed_in(), AuthInput::AuthFailed(AuthError::AzTimeout));
    assert_eq!(state, signed_in());
    assert_eq!(effects, Vec::new());
}

#[test]
fn auth_pat_stored_resumes() {
    let paused = AuthState::NeedsSignIn {
        error: AuthError::PatInvalid,
    };
    let (state, effects) = step(&paused, AuthInput::PatStored);
    assert_eq!(state, AuthState::Checking);
    assert_eq!(effects, vec![AuthEffect::ResumeRefresh { immediate: true }]);
}

#[test]
fn auth_ok_after_problem_resumes_immediately() {
    let paused = AuthState::NeedsSignIn {
        error: needs_sign_in(),
    };
    let (state, effects) = step(&paused, AuthInput::AuthOk { method: AZ });
    assert_eq!(state, signed_in());
    assert_eq!(effects, vec![AuthEffect::ResumeRefresh { immediate: true }]);

    let (_, effects) = step(&AuthState::Checking, AuthInput::AuthOk { method: AZ });
    assert_eq!(effects, Vec::new());
}

#[test]
fn auth_refresh_inputs_ignored_while_inert() {
    let inert = [
        AuthState::SignedOut,
        AuthState::Disabled,
        AuthState::SigningIn {
            method: SignInMethod::Silent,
            since: ts(0),
        },
    ];
    for state in inert {
        let (after_ok, _) = step(&state, AuthInput::AuthOk { method: AZ });
        let (after_fail, _) = step(&state, AuthInput::AuthFailed(needs_sign_in()));
        assert_eq!(after_ok, state);
        assert_eq!(after_fail, state);
    }
}

#[test]
fn auth_sign_out_pauses_and_cancels() {
    let (state, effects) = step(&signed_in(), AuthInput::SignOutRequested);
    assert_eq!(state, AuthState::SignedOut);
    assert_eq!(
        effects,
        vec![AuthEffect::PauseRefresh, AuthEffect::CancelSignIn]
    );
}

#[test]
fn auth_disabled_ignores_sign_in() {
    let (state, effects) = step(
        &AuthState::Disabled,
        AuthInput::SignInRequested(SignInMethod::Silent),
    );
    assert_eq!(state, AuthState::Disabled);
    assert_eq!(effects, Vec::new());
}

#[test]
fn auth_refresh_started_checks_only_from_unknown() {
    assert_eq!(
        step(&AuthState::Unknown, AuthInput::RefreshStarted).0,
        AuthState::Checking
    );
    assert_eq!(step(&signed_in(), AuthInput::RefreshStarted).0, signed_in());
}

#[test]
fn auth_view_carries_ui_error() {
    let view = AuthState::NeedsSignIn {
        error: needs_sign_in(),
    }
    .view();
    let AuthStateView::NeedsSignIn { error } = view else {
        panic!("expected NeedsSignIn, got {view:?}");
    };
    assert_eq!(error.title.as_ref(), needs_sign_in().to_string());
    assert!(error.action.is_some());
}
