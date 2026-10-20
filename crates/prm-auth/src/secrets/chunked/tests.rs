use super::*;
use crate::secrets::InMemorySecretStore;

const CREDMAN_LIMIT: usize = 2_560;

fn store() -> ChunkedSecretStore<InMemorySecretStore> {
    ChunkedSecretStore::new(InMemorySecretStore::new(Some(CREDMAN_LIMIT)))
}

fn key() -> SecretKey {
    SecretKey::entra_refresh(&prm_domain::testing::org("contoso"))
}

fn bytes(len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| u8::try_from(i % 251).unwrap_or(0))
        .collect()
}

#[test]
fn chunked_roundtrip_0_2559_2560_2561_10000() {
    for len in [0, 1, 2_559, 2_560, 2_561, 10_000] {
        let store = store();
        let value = bytes(len);
        store.set(&key(), &value).unwrap();
        assert_eq!(
            store.get(&key()).unwrap().map(|v| v.to_vec()),
            Some(value),
            "len {len}"
        );
    }
}

#[test]
fn chunked_missing_part_corrupt() {
    let store = store();
    store.set(&key(), &bytes(5_000)).unwrap();
    store.inner.delete(&key().part(1)).unwrap();
    assert!(matches!(
        store.get(&key()),
        Err(SecretStoreError::Corrupt(_))
    ));
}

#[test]
fn chunked_checksum_mismatch_corrupt() {
    let store = store();
    store.set(&key(), &bytes(5_000)).unwrap();
    store.inner.set(&key().part(0), &[1, 2, 3]).unwrap();
    assert!(matches!(
        store.get(&key()),
        Err(SecretStoreError::Corrupt(_))
    ));
}

#[test]
fn chunked_overwrite_shorter_removes_stale_parts() {
    let store = store();
    store.set(&key(), &bytes(7_000)).unwrap();
    store.set(&key(), &bytes(3_000)).unwrap();
    assert_eq!(store.inner.keys().len(), 3, "{:?}", store.inner.keys());
    store.set(&key(), &bytes(10)).unwrap();
    assert_eq!(store.inner.keys(), vec![key().as_str().to_owned()]);
}

#[test]
fn chunked_delete_header_first() {
    let store = store();
    store.set(&key(), &bytes(5_000)).unwrap();
    store.delete(&key()).unwrap();
    assert_eq!(store.inner.keys(), Vec::<String>::new());
    assert_eq!(store.get(&key()).unwrap(), None);
}

#[test]
fn chunked_reads_raw_legacy() {
    let store = store();
    store.inner.set(&key(), b"plain-token").unwrap();
    assert_eq!(
        store.get(&key()).unwrap().map(|v| v.to_vec()),
        Some(b"plain-token".to_vec())
    );
}
