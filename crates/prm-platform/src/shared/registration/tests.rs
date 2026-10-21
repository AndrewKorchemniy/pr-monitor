use std::fs;

use super::*;

#[test]
fn registration_scope_table() {
    use BuildKind::{Debug, Release};
    use InstanceScope::{Default, Isolated};
    use RegisterOverride::{Auto, Forced};
    let cases = [
        (Default, Release, Auto, RegistrationScope::Full),
        (Default, Debug, Auto, RegistrationScope::ToastOnlyIfMissing),
        (
            Isolated,
            Release,
            Auto,
            RegistrationScope::ToastOnlyIfMissing,
        ),
        (Isolated, Debug, Forced, RegistrationScope::Full),
        (Default, Debug, Forced, RegistrationScope::Full),
    ];
    for (scope, build, register, expected) in cases {
        assert_eq!(
            registration_scope(scope, build, register),
            expected,
            "{scope:?} {build:?} {register:?}"
        );
    }
}

/// A dev build on the default root must not point the installed app's launch-at-login entry
/// at `target\debug`.
#[test]
fn should_heal_autostart_table() {
    let cases = [
        (InstanceScope::Default, RegistrationScope::Full, true),
        (
            InstanceScope::Default,
            RegistrationScope::ToastOnlyIfMissing,
            false,
        ),
        (InstanceScope::Isolated, RegistrationScope::Full, true),
        (
            InstanceScope::Isolated,
            RegistrationScope::ToastOnlyIfMissing,
            true,
        ),
    ];
    for (scope, registration, expected) in cases {
        assert_eq!(
            should_heal_autostart(scope, registration),
            expected,
            "{scope:?} {registration:?}"
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
        "the user's home dir"
    );
    assert!(
        !safe_to_purge(Path::new("/"), InstanceScope::Isolated, None),
        "too shallow"
    );

    let default = dir.path().join("PRMonitor");
    assert!(safe_to_purge(&default, InstanceScope::Default, None));
    assert!(!safe_to_purge(&ours, InstanceScope::Default, None));
}

/// `--home D:\prm --unregister --purge` was always refused: the guard compared the root with
/// the `--home` value it came from instead of the user's home.
#[test]
fn safe_to_purge_allows_an_absolute_home_override() {
    let dir = tempfile::tempdir().unwrap();
    let ours = dir.path().join("prm");
    let user_home = dir.path().join("alice");
    fs::create_dir_all(&ours).unwrap();
    fs::create_dir_all(&user_home).unwrap();
    fs::write(ours.join("state.json"), "{}").unwrap();

    assert!(safe_to_purge(
        &ours,
        InstanceScope::Isolated,
        Some(&user_home)
    ));
}

/// `--home .` run from the profile folder made the root `C:\Users\alice\.`, which the old
/// textual check did not see as the home folder, so `--purge` deleted the whole profile.
#[test]
fn safe_to_purge_refuses_the_user_home_however_spelled() {
    let dir = tempfile::tempdir().unwrap();
    let user_home = dir.path().join("alice");
    fs::create_dir_all(&user_home).unwrap();
    fs::write(user_home.join("config.toml"), "").unwrap();

    for root in [user_home.join("."), user_home.join("x").join("..")] {
        assert!(
            !safe_to_purge(&root, InstanceScope::Isolated, Some(&user_home)),
            "{}",
            root.display()
        );
    }
}

#[test]
fn safe_to_purge_refuses_an_ancestor_of_the_user_home() {
    let dir = tempfile::tempdir().unwrap();
    let ours = dir.path().join("users");
    let user_home = ours.join("alice");
    fs::create_dir_all(&user_home).unwrap();
    fs::write(ours.join("config.toml"), "").unwrap();

    assert!(!safe_to_purge(
        &ours,
        InstanceScope::Isolated,
        Some(&user_home)
    ));
}
