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

fn default_root() -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        r"C:\Users\me\AppData\Local\PRMonitor"
    } else {
        "/home/me/.local/share/PRMonitor"
    })
}

#[test]
fn root_without_home_is_the_default_instance() {
    let root = Root::new(default_root(), false, Some(default_root()));
    assert_eq!(
        root,
        Root {
            path: default_root(),
            scope: InstanceScope::Default,
            home: None
        }
    );
}

/// Regression: `--home` pointed at the default folder got an isolated lock, so it ran as a
/// second primary on the default instance's files.
#[test]
fn root_home_naming_the_default_folder_is_the_default_instance() {
    let spelled = if cfg!(windows) {
        PathBuf::from(r"c:\users\me\appdata\local\prmonitor")
    } else {
        default_root()
    };
    let root = Root::new(spelled, true, Some(default_root()));
    assert_eq!(root.scope, InstanceScope::Default);
    assert_eq!(root.path, default_root(), "the default spelling is kept");
    assert_eq!(root.home, None);
}

/// Regression: the raw (possibly relative) `--home` text went into the launch-at-login
/// command; it is now the resolved absolute root.
#[test]
fn root_home_elsewhere_is_isolated_and_carries_the_resolved_path() {
    let custom = PathBuf::from(if cfg!(windows) {
        r"C:\work\prm-data"
    } else {
        "/work/prm-data"
    });
    let root = Root::new(custom.clone(), true, Some(default_root()));
    assert_eq!(
        root,
        Root {
            path: custom.clone(),
            scope: InstanceScope::Isolated,
            home: Some(custom)
        }
    );
}

/// `--home %LOCALAPPDATA%\PRMonitor\` used to run as an isolated instance beside the default
/// one, both writing the same `state.json`.
#[test]
fn root_scope_treats_the_default_folder_as_default() {
    let default = PathBuf::from("/data/PRMonitor");
    let other = PathBuf::from("/data/prm-test");
    let spelled = PathBuf::from("/data/./PRMonitor/");
    let cases = [
        (None, &default, InstanceScope::Default),
        (Some(&spelled), &spelled, InstanceScope::Default),
        (Some(&other), &other, InstanceScope::Isolated),
    ];
    for (home, root, expected) in cases {
        assert_eq!(
            root_scope(home.map(PathBuf::as_path), root, Some(&default)),
            expected,
            "{home:?} {root:?}"
        );
    }
    assert_eq!(
        root_scope(Some(&other), &other, None),
        InstanceScope::Isolated,
        "no default folder to compare with"
    );
}
