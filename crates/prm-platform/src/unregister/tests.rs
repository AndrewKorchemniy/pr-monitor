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
        config_root: root,
        legacy_root: None,
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

/// The local root is also the install folder: a purge must not touch what the app runs from.
#[test]
fn unregister_purge_keeps_velopack_entries() {
    let dir = tempfile::tempdir().unwrap();
    let (root, user_home) = (dir.path().join("prm"), dir.path().join("alice"));
    for sub in ["current", "packages", "logs", "themes"] {
        fs::create_dir_all(root.join(sub)).unwrap();
        fs::write(root.join(sub).join("f"), "").unwrap();
    }
    for file in [
        "Update.exe",
        "PR Monitor.exe",
        "sq.version",
        ".portable",
        "config.toml",
    ] {
        fs::write(root.join(file), "").unwrap();
    }

    assert_eq!(purge(&spec(&root, &user_home)), Ok(()));

    for kept in [
        "current/f",
        "packages/f",
        "Update.exe",
        "PR Monitor.exe",
        "sq.version",
        ".portable",
    ] {
        assert!(root.join(kept).exists(), "{kept}");
    }
    for gone in ["logs", "themes", "config.toml"] {
        assert!(!root.join(gone).exists(), "{gone}");
    }
}

/// Settings live in their own folder: the purge removes it, and the legacy folder too.
#[test]
fn unregister_purge_removes_the_config_and_legacy_folders() {
    let dir = tempfile::tempdir().unwrap();
    let user_home = dir.path().join("alice");
    let local = dir.path().join("local").join("pr-monitor");
    let config = dir.path().join("roaming").join("pr-monitor");
    let legacy = dir.path().join("local").join("PRMonitor");
    for folder in [&local, &config, &legacy] {
        fs::create_dir_all(folder).unwrap();
        fs::write(folder.join("f"), "").unwrap();
    }
    fs::create_dir_all(local.join("current")).unwrap();
    let mut spec = spec(&local, &user_home);
    spec.scope = InstanceScope::Default;
    spec.config_root = &config;
    spec.legacy_root = Some(&legacy);

    assert_eq!(purge(&spec), Ok(()));

    assert!(!config.exists());
    assert!(!legacy.exists());
    assert!(!local.join("f").exists());
    assert!(local.join("current").exists());
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
