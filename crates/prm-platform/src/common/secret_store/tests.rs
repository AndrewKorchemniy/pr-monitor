use prm_domain::testing::org;

use super::*;

#[test]
fn secret_backend_pref_parse() {
    let cases = [
        (None, SecretBackendPref::Auto),
        (Some("dpapi"), SecretBackendPref::Dpapi),
        (Some(" DPAPI "), SecretBackendPref::Dpapi),
        (Some("Keyring"), SecretBackendPref::Keyring),
        (Some("memory"), SecretBackendPref::Memory),
        (Some("vault"), SecretBackendPref::Auto),
    ];
    for (value, expected) in cases {
        assert_eq!(SecretBackendPref::parse(value), expected, "{value:?}");
    }
}

#[test]
fn platform_secret_store_memory_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    let store = secret_store(dir.path(), SecretBackendPref::Memory);
    assert_eq!(store.backend(), "memory");
    let key = SecretKey::pat(&org("contoso"));

    store.set(&key, b"secret").unwrap();

    assert_eq!(
        store.get(&key).unwrap().map(|v| v.to_vec()),
        Some(b"secret".to_vec())
    );
    store.delete(&key).unwrap();
    assert_eq!(store.get(&key).unwrap(), None);
}

#[cfg(windows)]
#[test]
fn platform_secret_store_dpapi_selected_by_pref() {
    let dir = tempfile::tempdir().unwrap();
    let store = secret_store(dir.path(), SecretBackendPref::Dpapi);
    assert_eq!(store.backend(), "dpapi");
    assert_eq!(store.max_value_len(), None);
}
