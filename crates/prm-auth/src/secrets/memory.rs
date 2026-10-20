//! A secret store in process memory: the fallback when no OS store works, and the test store.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
};

use prm_domain::secret::SecretBytes;

use super::{SecretKey, SecretStore, SecretStoreError};

#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    map: Mutex<HashMap<SecretKey, Vec<u8>>>,
    max: Option<usize>,
}

impl InMemorySecretStore {
    /// `max` mimics an OS store's per-value limit (Credential Manager: 2560).
    pub fn new(max: Option<usize>) -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
            max,
        }
    }

    /// Keys currently stored, sorted (for tests).
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.lock().keys().map(|k| k.as_str().to_owned()).collect();
        keys.sort();
        keys
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<SecretKey, Vec<u8>>> {
        self.map.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, key: &SecretKey) -> Result<Option<SecretBytes>, SecretStoreError> {
        Ok(self.lock().get(key).cloned().map(SecretBytes::new))
    }

    fn set(&self, key: &SecretKey, value: &[u8]) -> Result<(), SecretStoreError> {
        if self.max.is_some_and(|max| value.len() > max) {
            return Err(SecretStoreError::TooLong(value.len()));
        }
        self.lock().insert(key.clone(), value.to_vec());
        Ok(())
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretStoreError> {
        self.lock().remove(key);
        Ok(())
    }
}
