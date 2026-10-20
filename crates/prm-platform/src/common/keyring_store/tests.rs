use prm_auth::ChunkedSecretStore;
use prm_domain::testing::org;

use super::*;

const CREDMAN_LIMIT: usize = 2_560;

fn sample() -> KeyringSecretStore {
    let store: Arc<CredentialStore> = keyring_core::sample::Store::new().unwrap();
    KeyringSecretStore::new(store, None, None, Some(CREDMAN_LIMIT))
}

fn key() -> SecretKey {
    SecretKey::pat(&org("contoso"))
}

#[test]
fn keyring_store_roundtrip_sample_store() {
    let store = sample();
    assert_eq!(store.get(&key()).unwrap(), None);

    store.set(&key(), b"token-1").unwrap();
    assert_eq!(
        store.get(&key()).unwrap().map(|v| v.to_vec()),
        Some(b"token-1".to_vec())
    );
    store.set(&key(), b"token-2").unwrap();
    assert_eq!(
        store.get(&key()).unwrap().map(|v| v.to_vec()),
        Some(b"token-2".to_vec())
    );

    store.delete(&key()).unwrap();
    assert_eq!(store.get(&key()).unwrap(), None);
    store.delete(&key()).unwrap();
}

#[test]
fn keyring_store_enforces_max() {
    let store = sample();
    let too_long = vec![7_u8; CREDMAN_LIMIT + 1];
    assert_eq!(
        store.set(&key(), &too_long),
        Err(SecretStoreError::TooLong(CREDMAN_LIMIT + 1))
    );
}

#[test]
fn chunked_over_keyring_sample_0_2559_2560_2561_10000() {
    let store = ChunkedSecretStore::new(sample());
    for len in [0, 2_559, 2_560, 2_561, 10_000] {
        let value: Vec<u8> = (0..len).map(|i| u8::try_from(i % 251).unwrap()).collect();
        store.set(&key(), &value).unwrap();
        assert_eq!(
            store.get(&key()).unwrap().map(|v| v.to_vec()),
            Some(value),
            "len {len}"
        );
    }
}
