use prm_domain::{auth::SignInMethod, testing::ts};

use super::*;

#[test]
fn auth_state_needs_sign_in_only_without_a_usable_credential() {
    let error = || UiError::new("Sign in again");
    let cases = [
        (AuthStateView::Unknown, false),
        (AuthStateView::Checking, false),
        (AuthStateView::SignedIn, false),
        (
            AuthStateView::SigningIn {
                method: SignInMethod::Entra,
                since: ts(0),
            },
            false,
        ),
        (AuthStateView::NeedsSignIn { error: error() }, true),
        (AuthStateView::Blocked { error: error() }, true),
        (AuthStateView::SignedOut, true),
        (AuthStateView::Disabled, false),
    ];
    for (state, expected) in cases {
        assert_eq!(state.needs_sign_in(), expected, "{state:?}");
    }
}
