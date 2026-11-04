use prm_domain::testing::{FakeUpdater, UpdaterCall};

use super::*;

fn offer(version: &str) -> UpdateOffer {
    let mut info = UpdateInfo::default();
    info.TargetFullRelease.Version = version.to_owned();
    offer_of(&info)
}

#[test]
fn a_release_becomes_an_offer_with_its_version() {
    assert_eq!(&*offer("0.3.0").version, "0.3.0");
}

#[test]
fn progress_is_a_percent_between_zero_and_one_hundred() {
    let cases = [(-5, 0), (0, 0), (35, 35), (100, 100), (250, 100)];
    for (reported, expected) in cases {
        assert_eq!(percent(reported), expected, "{reported}");
    }
}

#[test]
fn a_side_by_side_home_is_never_updated() {
    let updater = VelopackUpdater::new(InstanceScope::Isolated);

    assert_eq!(updater.availability(), Err(DisabledReason::IsolatedHome));
    assert_eq!(updater.pending(), None);
    assert_eq!(
        updater.check().expect_err("not available").kind,
        PortErrorKind::Unsupported
    );
}

/// The test exe sits in `target/`, with no Update.exe beside it, like `cargo run`.
#[test]
fn an_exe_that_was_not_installed_cannot_update() {
    let updater = VelopackUpdater::new(InstanceScope::Default);

    assert_eq!(updater.availability(), Err(DisabledReason::NotInstalled));
    assert_eq!(updater.pending(), None);
}

fn restart_args() -> Vec<OsString> {
    vec!["--minimized".into()]
}

#[test]
fn a_pending_update_is_applied_at_start_with_the_same_arguments() {
    let updater = FakeUpdater::default();
    updater.set_pending(Some("0.3.0"));

    let handed_over = apply_pending_at_start(&updater, Boot::default(), &restart_args());

    assert!(
        handed_over,
        "the caller exits so the updater can swap files"
    );
    assert_eq!(
        updater.calls(),
        [UpdaterCall::Apply(restart_args(), ApplyMode::Silent)]
    );
}

#[test]
fn nothing_is_applied_at_start_without_a_pending_update() {
    let updater = FakeUpdater::default();

    assert!(!apply_pending_at_start(
        &updater,
        Boot::default(),
        &restart_args()
    ));
    assert_eq!(updater.calls(), []);
}

/// Velopack restarts the app after applying; applying again would loop if a package could not
/// be swapped in.
#[test]
fn a_process_velopack_just_restarted_does_not_apply_again() {
    let updater = FakeUpdater::default();
    updater.set_pending(Some("0.3.0"));
    let boot = Boot { restarted: true };

    assert!(!apply_pending_at_start(&updater, boot, &restart_args()));
    assert_eq!(updater.calls(), []);
}

#[test]
fn a_failed_apply_lets_the_app_start_normally() {
    let updater = FakeUpdater::default();
    updater.set_pending(Some("0.3.0"));
    updater.fail_apply();

    assert!(!apply_pending_at_start(
        &updater,
        Boot::default(),
        &restart_args()
    ));
    assert_eq!(updater.calls().len(), 1);
}

/// The adapter against a real `UpdateManager` over a local feed folder and an installed-looking
/// folder in a tempdir: no network, no Update.exe run.
#[cfg(windows)]
#[test]
fn a_local_feed_offers_a_newer_release_and_downloads_it() {
    use std::fmt::Write as _;

    use sha2::{Digest as _, Sha256};
    use velopack::locator::VelopackLocatorConfig;

    let dir = tempfile::tempdir().expect("tempdir");
    let (root, feed) = (dir.path().join("app"), dir.path().join("feed"));
    let current = root.join("current");
    for folder in [&current, &feed] {
        std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(root.join("Update.exe"), b"").expect("update exe");
    let manifest = "<package><metadata><id>AndrewKorchemniy.PRMonitor</id><version>0.2.4</version>        <mainExe>pr-monitor.exe</mainExe><channel>win-x64</channel></metadata></package>";
    std::fs::write(current.join("sq.version"), manifest).expect("manifest");
    let package = b"not really a package";
    let name = "AndrewKorchemniy.PRMonitor-0.3.0-win-x64-full.nupkg";
    std::fs::write(feed.join(name), package).expect("package");
    let sha256 = Sha256::digest(package)
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        });
    let releases = format!(
        r#"{{"Assets":[{{"PackageId":"AndrewKorchemniy.PRMonitor","Version":"0.3.0","Type":"Full","FileName":"{name}","SHA1":"","SHA256":"{sha256}","Size":{},"NotesMarkdown":"","NotesHtml":""}}]}}"#,
        package.len()
    );
    std::fs::write(feed.join("releases.win-x64.json"), releases).expect("feed");
    let locator = VelopackLocatorConfig {
        RootAppDir: root.clone(),
        UpdateExePath: root.join("Update.exe"),
        PackagesDir: root.join("packages"),
        ManifestPath: current.join("sq.version"),
        CurrentBinaryDir: current,
        IsPortable: false,
    };
    let manager = UpdateManager::new(FileSource::new(&feed), None, Some(locator)).expect("manager");
    let updater = VelopackUpdater {
        backing: Ok(Ready {
            manager,
            found: Mutex::new(None),
        }),
    };

    let offer = updater.check().expect("check").expect("a newer release");
    let reported = Mutex::new(Vec::new());
    updater
        .download(&offer, &|percent| {
            reported.lock().expect("lock").push(percent);
        })
        .expect("download");

    assert_eq!(&*offer.version, "0.3.0");
    assert_eq!(*reported.lock().expect("lock"), [50, 100]);
    assert!(root.join("packages").join(name).exists());
}
