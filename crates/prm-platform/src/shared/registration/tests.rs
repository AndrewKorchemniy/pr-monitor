use std::fs;

use super::*;

#[test]
fn registration_scope_table() {
    let cases = [
        (true, false, false, RegistrationScope::Full),
        (true, true, false, RegistrationScope::ToastOnlyIfMissing),
        (false, false, false, RegistrationScope::ToastOnlyIfMissing),
        (false, true, true, RegistrationScope::Full),
        (true, true, true, RegistrationScope::Full),
    ];
    for (default_root, debug, env, expected) in cases {
        assert_eq!(
            registration_scope(default_root, debug, env),
            expected,
            "{default_root} {debug} {env}"
        );
    }
}

#[test]
fn safe_to_purge_table() {
    let dir = tempfile::tempdir().unwrap();
    let ours = dir.path().join("custom-root");
    fs::create_dir_all(&ours).unwrap();
    assert!(
        !safe_to_purge(&ours, InstanceScope::Isolated, None),
        "empty folder"
    );
    fs::write(ours.join("config.toml"), "").unwrap();
    assert!(safe_to_purge(&ours, InstanceScope::Isolated, None));
    assert!(
        !safe_to_purge(&ours, InstanceScope::Isolated, Some(&ours)),
        "home dir"
    );
    assert!(
        !safe_to_purge(Path::new("/"), InstanceScope::Isolated, None),
        "too shallow"
    );

    let default = dir.path().join("PRMonitor");
    assert!(safe_to_purge(&default, InstanceScope::Default, None));
    assert!(!safe_to_purge(&ours, InstanceScope::Default, None));
}
