use std::fs;

use super::*;

#[test]
fn unregister_plan_default_runs_every_step() {
    assert_eq!(plan(InstanceScope::Default), PLATFORM_STEPS.to_vec());
}

/// `--home D:\scratch --unregister` used to delete the default instance's saved sign-ins and
/// its toast identity, which every instance on the account shares.
#[test]
fn unregister_plan_isolated_keeps_shared_steps() {
    let steps = plan(InstanceScope::Isolated);

    assert!(steps.contains(&Step::LaunchAtLogin), "{steps:?}");
    for shared in [
        Step::Notifications,
        Step::UrlProtocol,
        Step::ToastIdentity,
        Step::SavedCredentials,
    ] {
        assert!(!steps.contains(&shared), "{shared:?} in {steps:?}");
    }
}

#[test]
fn unregister_step_is_shared_table() {
    let cases = [
        (Step::LaunchAtLogin, false),
        (Step::Notifications, true),
        (Step::UrlProtocol, true),
        (Step::ToastIdentity, true),
        (Step::SavedCredentials, true),
        (Step::SecretFiles, false),
    ];
    for (step, expected) in cases {
        assert_eq!(step.is_shared(), expected, "{step:?}");
    }
}

/// A spec the way the bin builds one: the user's home, not the `--home` override.
fn spec<'a>(root: &'a Path, user_home: &'a Path) -> UnregisterSpec<'a> {
    UnregisterSpec {
        root,
        scope: InstanceScope::Isolated,
        instance_suffix: "abcd1234",
        purge: true,
        org_ids: &[],
        user_home: Some(user_home),
    }
}

#[test]
fn unregister_purge_deletes_an_isolated_root() {
    let dir = tempfile::tempdir().unwrap();
    let (root, user_home) = (dir.path().join("prm"), dir.path().join("alice"));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("config.toml"), "").unwrap();

    assert_eq!(purge(&spec(&root, &user_home)), Ok(()));
    assert!(!root.exists());
}

#[test]
fn unregister_purge_refuses_the_user_home() {
    let dir = tempfile::tempdir().unwrap();
    let user_home = dir.path().join("alice");
    fs::create_dir_all(&user_home).unwrap();
    fs::write(user_home.join("config.toml"), "").unwrap();
    let root = user_home.join(".");

    assert!(purge(&spec(&root, &user_home)).is_err());
    assert!(user_home.join("config.toml").exists());
}

#[cfg(windows)]
#[test]
fn unregister_secret_files_removes_only_ours() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = dir.path().join("secrets");
    fs::create_dir_all(&secrets).unwrap();
    fs::write(secrets.join("61622f706174.bin"), b"sealed").unwrap();
    fs::write(secrets.join("notes.txt"), b"keep").unwrap();

    assert_eq!(remove_secret_files(&secrets), Ok(()));

    assert!(!secrets.join("61622f706174.bin").exists());
    assert!(secrets.join("notes.txt").exists());
}

#[cfg(windows)]
#[test]
fn unregister_secret_files_removes_the_emptied_folder() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = dir.path().join("secrets");
    fs::create_dir_all(&secrets).unwrap();
    fs::write(secrets.join("61622f706174.bin"), b"sealed").unwrap();

    assert_eq!(remove_secret_files(&secrets), Ok(()));
    assert!(!secrets.exists());
    assert_eq!(
        remove_secret_files(&secrets),
        Ok(()),
        "missing counts as removed"
    );
}
