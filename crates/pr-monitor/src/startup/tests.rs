use super::*;

#[test]
fn a_second_launch_forwards_links_shows_or_stays_quiet() {
    let link = Args {
        activate: Some("prmonitor://refresh".to_owned()),
        minimized: true,
        ..Args::default()
    };
    assert_eq!(
        forward_request(&link),
        Some(HandoffRequest::Activate(AppUri::Refresh))
    );
    let bad_link = Args {
        activate: Some("nonsense".to_owned()),
        ..Args::default()
    };
    assert_eq!(forward_request(&bad_link), Some(HandoffRequest::Show));
    let autostart = Args {
        minimized: true,
        ..Args::default()
    };
    assert_eq!(forward_request(&autostart), None);
    assert_eq!(
        forward_request(&Args::default()),
        Some(HandoffRequest::Show)
    );
}

#[test]
fn should_relaunch_only_for_a_first_init_error_before_the_first_frame() {
    let cases = [
        ((true, Attempt::First, Phase::Init), true),
        ((true, Attempt::Fallback, Phase::Init), false),
        ((true, Attempt::First, Phase::Running), false),
        ((true, Attempt::Fallback, Phase::Running), false),
        ((false, Attempt::First, Phase::Init), false),
        ((false, Attempt::Fallback, Phase::Init), false),
        ((false, Attempt::First, Phase::Running), false),
        ((false, Attempt::Fallback, Phase::Running), false),
    ];
    for ((init_error, attempt, phase), expected) in cases {
        assert_eq!(
            should_relaunch(init_error, attempt, phase),
            expected,
            "init_error={init_error} attempt={attempt:?} phase={phase:?}"
        );
    }
}

#[test]
fn attempt_is_a_fallback_only_when_relaunched() {
    assert_eq!(Attempt::of(&Args::default()), Attempt::First);
    let child = Args {
        relaunched_after: Some(7),
        ..Args::default()
    };
    assert_eq!(Attempt::of(&child), Attempt::Fallback);
}
