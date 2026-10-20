use super::*;

#[test]
fn self_test_short_circuits_before_instance_lock() {
    let args = Args {
        mode: Mode::SelfTest,
        ..Args::default()
    };
    assert_eq!(plan(&args), StartupPlan::SelfTest);
}

#[test]
fn version_and_help_only_print() {
    let version = plan(&Args {
        mode: Mode::Version,
        ..Args::default()
    });
    assert!(matches!(version, StartupPlan::Print(ref text) if text.contains(build_info::VERSION)));
    let help = plan(&Args {
        mode: Mode::Help,
        ..Args::default()
    });
    assert!(matches!(help, StartupPlan::Print(ref text) if text.contains("--self-test")));
    assert_eq!(plan(&Args::default()), StartupPlan::Run);
}

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
    assert_eq!(forward_request(&Args::default()), Some(HandoffRequest::Show));
}

#[test]
fn after_forward_takes_over_only_when_nobody_listens() {
    let cases = [
        (ForwardOutcome::Delivered, Next::Done),
        (
            ForwardOutcome::Rejected("bad request".to_owned()),
            Next::Fatal("bad request".to_owned()),
        ),
        (ForwardOutcome::NoPrimary, Next::TakeOver),
        (ForwardOutcome::NotResponding, Next::NotResponding),
    ];
    for (outcome, expected) in cases {
        let label = format!("{outcome:?}");
        assert_eq!(after_forward(outcome), expected, "{label}");
    }
}

#[test]
fn should_relaunch_only_for_a_first_init_error_before_the_first_frame() {
    // (init_error, already_relaunched, first_frame_seen) -> relaunch
    let cases = [
        ((true, false, false), true),
        ((true, true, false), false),
        ((true, false, true), false),
        ((true, true, true), false),
        ((false, false, false), false),
        ((false, true, false), false),
        ((false, false, true), false),
        ((false, true, true), false),
    ];
    for ((init_error, relaunched, frame), expected) in cases {
        assert_eq!(
            should_relaunch(init_error, relaunched, frame),
            expected,
            "init_error={init_error} already_relaunched={relaunched} first_frame_seen={frame}"
        );
    }
}
