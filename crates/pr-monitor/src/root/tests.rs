use super::*;

fn default_root() -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        r"C:\Users\me\AppData\Local\pr-monitor"
    } else {
        "/home/me/.local/share/pr-monitor"
    })
}

fn default_config_root() -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        r"C:\Users\me\AppData\Roaming\pr-monitor"
    } else {
        "/home/me/.config/pr-monitor"
    })
}

fn default_roots() -> Roots {
    Roots {
        config: default_config_root(),
        local: default_root(),
    }
}

fn one_folder(path: PathBuf) -> Roots {
    Roots {
        config: path.clone(),
        local: path,
    }
}

#[test]
fn root_without_home_is_the_default_instance() {
    let root = Root::new(default_roots(), false, Some(default_roots()));
    assert_eq!(
        root,
        Root {
            path: default_root(),
            config_root: default_config_root(),
            scope: InstanceScope::Default,
            home: None,
            legacy: None,
        }
    );
}

/// Regression: `--home` pointed at the default folder got an isolated lock, so it ran as a
/// second primary on the default instance's files.
#[test]
fn root_home_naming_the_default_folder_is_the_default_instance() {
    let spelled = if cfg!(windows) {
        PathBuf::from(r"c:\users\me\appdata\local\pr-monitor")
    } else {
        default_root()
    };
    let root = Root::new(one_folder(spelled), true, Some(default_roots()));
    assert_eq!(root.scope, InstanceScope::Default);
    assert_eq!(root.path, default_root(), "the default spelling is kept");
    assert_eq!(root.config_root, default_config_root());
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
    let root = Root::new(one_folder(custom.clone()), true, Some(default_roots()));
    assert_eq!(
        root,
        Root {
            path: custom.clone(),
            config_root: custom.clone(),
            scope: InstanceScope::Isolated,
            home: Some(custom),
            legacy: None,
        }
    );
}

/// `--home %LOCALAPPDATA%\pr-monitor\` used to run as an isolated instance beside the default
/// one, both writing the same `state.json`.
#[test]
fn root_scope_treats_the_default_folder_as_default() {
    let default = PathBuf::from("/data/pr-monitor");
    let other = PathBuf::from("/data/prm-test");
    let spelled = PathBuf::from("/data/./pr-monitor/");
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
