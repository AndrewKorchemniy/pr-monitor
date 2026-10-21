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

/// An in-memory store whose writes to one key fail once armed.
#[derive(Debug, Default)]
struct FailingStore {
    inner: InMemorySecretStore,
    fail_on: std::sync::Mutex<Option<SecretKey>>,
}

impl SecretStore for FailingStore {
    fn get(&self, key: &SecretKey) -> Result<Option<SecretBytes>, SecretStoreError> {
        self.inner.get(key)
    }

    fn set(&self, key: &SecretKey, value: &[u8]) -> Result<(), SecretStoreError> {
        if self.fail_on.lock().unwrap().as_ref() == Some(key) {
            return Err(SecretStoreError::Unavailable("locked".into()));
        }
        self.inner.set(key, value)
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretStoreError> {
        self.inner.delete(key)
    }
}

/// A rotated refresh token whose second part failed to write used to leave the old header
/// over mixed parts: every later read was `Corrupt` and the user was never asked to sign in.
#[test]
fn chunked_failed_overwrite_reads_absent() {
    let store = ChunkedSecretStore::new(FailingStore::default());
    store.set(&key(), &bytes(7_000)).unwrap();
    *store.inner.fail_on.lock().unwrap() = Some(key().part(1));
    let new_value: Vec<u8> = bytes(7_000).into_iter().rev().collect();
    assert!(store.set(&key(), &new_value).is_err());
    assert_eq!(store.get(&key()).unwrap(), None);
}

#[test]
fn chunked_oversized_header_len_is_corrupt_not_abort() {
    let store = store();
    let header = format!("{CHUNK_MAGIC}1:{}:0000000000000000", usize::MAX);
    store.inner.set(&key(), header.as_bytes()).unwrap();
    store.inner.set(&key().part(0), b"abc").unwrap();
    assert!(matches!(
        store.get(&key()),
        Err(SecretStoreError::Corrupt(_))
    ));
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
