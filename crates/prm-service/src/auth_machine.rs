//! The per-org auth state machine: pure transitions plus the effects the actor carries out.
//! One toast per transition into a problem state, never at startup.

use prm_domain::{
    Timestamp,
    auth::{AuthError, AuthMethod, AuthSeverity, SignInMethod},
};

use crate::{errors::UiError, state::AuthStateView};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) enum AuthState {
    #[default]
    Unknown,
    Checking,
    SignedIn {
        method: AuthMethod,
    },
    SigningIn {
        method: SignInMethod,
        since: Timestamp,
    },
    NeedsSignIn {
        error: AuthError,
    },
    Blocked {
        error: AuthError,
    },
    SignedOut,
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AuthInput {
    RefreshStarted,
    AuthOk {
        method: AuthMethod,
    },
    AuthFailed(AuthError),
    SignInRequested(SignInMethod),
    SignInOk,
    SignInFailed(AuthError),
    /// Restores the state from before the sign-in started.
    SignInCancelled {
        restore: Box<AuthState>,
    },
    PatStored,
    SignOutRequested,
    Disabled,
    Enabled,
    /// Auth settings changed: start over.
    Reset,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum AuthEffect {
    PauseRefresh,
    ResumeRefresh { immediate: bool },
    NotifyProblem,
    StartSignIn(SignInMethod),
    CancelSignIn,
}

impl AuthState {
    /// States in which background refreshes do not run and their results are ignored.
    const fn is_inert(&self) -> bool {
        matches!(
            self,
            Self::SigningIn { .. } | Self::SignedOut | Self::Disabled
        )
    }

    pub(crate) fn view(&self) -> AuthStateView {
        match self {
            Self::Unknown => AuthStateView::Unknown,
            Self::Checking => AuthStateView::Checking,
            Self::SignedIn { .. } => AuthStateView::SignedIn,
            Self::SigningIn { method, since } => AuthStateView::SigningIn {
                method: *method,
                since: *since,
            },
            Self::NeedsSignIn { error } => AuthStateView::NeedsSignIn {
                error: UiError::from(error),
            },
            Self::Blocked { error } => AuthStateView::Blocked {
                error: UiError::from(error),
            },
            Self::SignedOut => AuthStateView::SignedOut,
            Self::Disabled => AuthStateView::Disabled,
        }
    }

    pub(crate) const fn is_signed_in(&self) -> bool {
        matches!(self, Self::SignedIn { .. })
    }
}

type Transition = (AuthState, Vec<AuthEffect>);

/// The next state and what the actor must do about it.
pub(crate) fn transition(state: &AuthState, input: AuthInput, now: Timestamp) -> Transition {
    match input {
        AuthInput::RefreshStarted => refresh_started(state),
        AuthInput::AuthOk { method } => auth_ok(state, method),
        AuthInput::AuthFailed(error) => auth_failed(state, error),
        AuthInput::SignInRequested(method) => sign_in_requested(state, method, now),
        AuthInput::SignInOk | AuthInput::PatStored => resume_checking(state),
        AuthInput::SignInFailed(error) => sign_in_failed(error),
        AuthInput::SignInCancelled { restore } => (*restore, Vec::new()),
        AuthInput::SignOutRequested => (
            AuthState::SignedOut,
            vec![AuthEffect::PauseRefresh, AuthEffect::CancelSignIn],
        ),
        AuthInput::Disabled => (
            AuthState::Disabled,
            vec![AuthEffect::PauseRefresh, AuthEffect::CancelSignIn],
        ),
        AuthInput::Enabled | AuthInput::Reset => (
            AuthState::Unknown,
            vec![AuthEffect::ResumeRefresh { immediate: true }],
        ),
    }
}

fn unchanged(state: &AuthState) -> Transition {
    (state.clone(), Vec::new())
}

fn refresh_started(state: &AuthState) -> Transition {
    if matches!(state, AuthState::Unknown) {
        (AuthState::Checking, Vec::new())
    } else {
        unchanged(state)
    }
}

fn auth_ok(state: &AuthState, method: AuthMethod) -> Transition {
    if state.is_inert() {
        return unchanged(state);
    }
    let was_paused = matches!(
        state,
        AuthState::NeedsSignIn { .. } | AuthState::Blocked { .. }
    );
    let effects = if was_paused {
        vec![AuthEffect::ResumeRefresh { immediate: true }]
    } else {
        Vec::new()
    };
    (AuthState::SignedIn { method }, effects)
}

fn problem_state(error: AuthError) -> AuthState {
    match error.severity() {
        AuthSeverity::Blocked => AuthState::Blocked { error },
        AuthSeverity::NeedsSignIn | AuthSeverity::Transient => AuthState::NeedsSignIn { error },
    }
}

fn auth_failed(state: &AuthState, error: AuthError) -> Transition {
    if state.is_inert() || error.severity() == AuthSeverity::Transient {
        return unchanged(state);
    }
    let mut effects = vec![AuthEffect::PauseRefresh];
    if state.is_signed_in() {
        effects.push(AuthEffect::NotifyProblem);
    }
    (problem_state(error), effects)
}

fn sign_in_requested(state: &AuthState, method: SignInMethod, now: Timestamp) -> Transition {
    if matches!(state, AuthState::Disabled) {
        return unchanged(state);
    }
    (
        AuthState::SigningIn { method, since: now },
        vec![AuthEffect::StartSignIn(method)],
    )
}

fn resume_checking(state: &AuthState) -> Transition {
    if matches!(state, AuthState::Disabled) {
        return unchanged(state);
    }
    (
        AuthState::Checking,
        vec![AuthEffect::ResumeRefresh { immediate: true }],
    )
}

fn sign_in_failed(error: AuthError) -> Transition {
    (problem_state(error), vec![AuthEffect::PauseRefresh])
}

#[cfg(test)]
mod tests;
