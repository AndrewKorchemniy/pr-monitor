//! Manual smoke tests against the real OS (registry, Credential Manager, DPAPI, WinRT, WinHTTP).
//! `#[ignore]`d: run with `cargo test -p prm-platform -- --ignored` on a Windows desktop. Each
//! uses a test identity and cleans up after itself.

use prm_auth::{ChunkedSecretStore, SecretKey, SecretStore, SecretStoreError};
use prm_domain::{notify::Notification, testing::org};
use prm_service::ports::{Autostart, AutostartState, Notifier};

use super::{autostart::WinAutostart, credman, dpapi::DpapiFileStore, toast::WinToastNotifier};
use crate::{
    common::keyring_store::KeyringSecretStore,
    shared::toast_xml::{spec_from, toast_xml},
};

const SMOKE_PREFIX: &str = "PRMonitorSmokeTest/";

#[test]
#[ignore = "touches the real OS"]
fn smoke_toast_xml_loads_in_xmldocument() {
    let xml = toast_xml(&spec_from(&Notification::test()));
    super::toast::validate_toast_xml(&xml).unwrap();
}

#[test]
#[ignore = "touches the real OS"]
fn smoke_toast_capability_reported() {
    // Setting() answers ERROR_NOT_FOUND (mapped to Unknown) for an AUMID without a Start-menu
    // identity; this checks the notify thread answers at all, within its timeout.
    let notifier = WinToastNotifier::start(super::toast::POWERSHELL_AUMID).unwrap();
    let capability = notifier.capability();
    tracing::info!(?capability, "toast capability");
}

#[test]
#[ignore = "touches the real OS"]
fn smoke_toast_show_replace_retract() {
    let notifier = WinToastNotifier::start(super::toast::POWERSHELL_AUMID).unwrap();
    let mut toast = Notification::test();
    toast.tag = "smoke".into();
    notifier.show(&toast).unwrap();
    toast.body = "Replaced".into();
    notifier.show(&toast).unwrap();
    notifier.retract("smoke", &toast.group).unwrap();
}

#[test]
#[ignore = "touches the real OS"]
fn smoke_dpapi_roundtrip_tempdir() {
    let dir = tempfile::tempdir().unwrap();
    let store = DpapiFileStore::new(dir.path().join("secrets"));
    let key = SecretKey::pat(&org("smoke"));
    store.set(&key, &[42; 5_000]).unwrap();
    assert_eq!(
        store.get(&key).unwrap().map(|v| v.to_vec()),
        Some(vec![42; 5_000])
    );
    store.delete(&key).unwrap();
    assert_eq!(store.get(&key).unwrap(), None);
}

#[test]
#[ignore = "touches the real OS"]
fn smoke_winhttp_resolve_does_not_panic() {
    let target = url::Url::parse("https://dev.azure.com/").unwrap();
    let decision = super::winhttp::resolve_proxy(&target, crate::proxy::ProxyMode::PacOnly);
    tracing::info!(?decision, "proxy decision");
}

#[test]
#[ignore = "touches the real OS"]
fn smoke_session_info_console_is_local() {
    let info = super::session::session_info();
    assert!(info.id.is_some());
    assert!(
        !info.is_remote(),
        "run from a local console session: {info:?}"
    );
}

fn smoke_keyring() -> KeyringSecretStore {
    let store: std::sync::Arc<keyring_core::CredentialStore> =
        windows_native_keyring_store::Store::new().unwrap();
    KeyringSecretStore::new(store, Some(SMOKE_PREFIX), Some("Local"), Some(2_560))
}

#[test]
#[ignore = "touches the real OS"]
fn smoke_credman_roundtrip_2560_and_too_long() {
    let store = smoke_keyring();
    let key = SecretKey::pat(&org("smoke"));
    store.set(&key, &[1; 2_560]).unwrap();
    assert_eq!(store.get(&key).unwrap().map(|v| v.len()), Some(2_560));
    assert_eq!(
        store.set(&key, &[1; 2_561]),
        Err(SecretStoreError::TooLong(2_561))
    );
    let chunked = ChunkedSecretStore::new(smoke_keyring());
    chunked.set(&key, &[2; 9_000]).unwrap();
    assert_eq!(chunked.get(&key).unwrap().map(|v| v.len()), Some(9_000));
    chunked.delete(&key).unwrap();
}

#[test]
#[ignore = "touches the real OS"]
fn smoke_credman_enumerate_and_delete() {
    let store = smoke_keyring();
    store.set(&SecretKey::pat(&org("enum-a")), b"a").unwrap();
    store.set(&SecretKey::pat(&org("enum-b")), b"b").unwrap();

    let targets = credman::credman_targets(SMOKE_PREFIX).unwrap();
    assert!(targets.len() >= 2, "{targets:?}");
    assert_eq!(credman::delete_targets(&targets).unwrap(), 0);
    assert_eq!(
        credman::credman_targets(SMOKE_PREFIX).unwrap(),
        Vec::<String>::new()
    );
}

#[test]
#[ignore = "touches the real OS"]
fn smoke_autostart_roundtrip_test_value() {
    let exe = std::env::current_exe().unwrap();
    let autostart = WinAutostart::new(exe, None, "PRMonitorSmokeTest".into());
    autostart.set_enabled(true).unwrap();
    assert_eq!(autostart.state().unwrap(), AutostartState::Enabled);
    autostart.set_enabled(false).unwrap();
    assert_eq!(autostart.state().unwrap(), AutostartState::Disabled);
    super::autostart::remove_run_values("PRMonitorSmokeTest").unwrap();
}
