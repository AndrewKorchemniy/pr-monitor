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
