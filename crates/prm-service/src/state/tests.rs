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

/// The refresh line restarted at every phase, since each phase reported its own items from
/// zero: the phases now fill one run end to end.
#[test]
fn the_phases_fill_the_run_in_order_without_restarting() {
    use RefreshPhase::{Authenticating, Catalog, PullRequests, Resolving, Threads};
    let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
    let mut last_end = 0.0;
    for phase in [Authenticating, Catalog, PullRequests, Threads, Resolving] {
        let (start, end) = phase.span();
        assert!(
            close(start, last_end),
            "{phase:?} starts where the last phase ended"
        );
        assert!(end > start, "{phase:?}");
        assert!(
            close(phase_fraction(phase, 0, 0), start),
            "unknown total: the span start"
        );
        assert!(
            close(phase_fraction(phase, 7, 7), end),
            "all done: the span end"
        );
        last_end = end;
    }
    assert!(close(last_end, 1.0));
    assert!(phase_fraction(Threads, 1, 40) > phase_fraction(PullRequests, 4, 4));
    assert!(close(phase_fraction(Threads, 20, 40), 0.63));
}
