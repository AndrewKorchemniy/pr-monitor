//! `SecretStore` over a keyring-core credential store (Credential Manager, Keychain,
//! Secret Service).

use std::{collections::HashMap, sync::Arc};

use keyring_core::{CredentialStore, Entry, Error};
use prm_auth::{SecretKey, SecretStore, SecretStoreError};
use prm_domain::{app::SECRET_SERVICE, secret::SecretBytes};

use crate::shared::secret_names::map_keyring_error;

/// The store is held explicitly (no process-global default store).
pub struct KeyringSecretStore {
    store: Arc<CredentialStore>,
    /// Credential Manager target name prefix (`PRMonitor/`), so uninstall can enumerate.
    target_prefix: Option<&'static str>,
    /// Credential Manager persistence (`Local`: no roaming).
    persistence: Option<&'static str>,
    max: Option<usize>,
}

impl KeyringSecretStore {
    /// `target_prefix` and `persistence` are Credential Manager modifiers (`None` elsewhere);
    /// `max` is the largest value one slot holds, in bytes.
    pub fn new(
        store: Arc<CredentialStore>,
        target_prefix: Option<&'static str>,
        persistence: Option<&'static str>,
        max: Option<usize>,
    ) -> Self {
        Self {
            store,
            target_prefix,
            persistence,
            max,
        }
    }

    fn entry(&self, key: &SecretKey) -> Result<Entry, SecretStoreError> {
        let target = self.target_prefix.map(|p| format!("{p}{}", key.as_str()));
        let mut modifiers: HashMap<&str, &str> = HashMap::new();
        if let Some(target) = target.as_deref() {
            modifiers.insert("target", target);
        }
        if let Some(persistence) = self.persistence {
            modifiers.insert("persistence", persistence);
        }
        let modifiers = (!modifiers.is_empty()).then_some(&modifiers);
        self.store
            .build(SECRET_SERVICE, key.as_str(), modifiers)
            .map_err(|e| map_keyring_error(&e, 0))
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, key: &SecretKey) -> Result<Option<SecretBytes>, SecretStoreError> {
        match self.entry(key)?.get_secret() {
            Ok(value) => Ok(Some(SecretBytes::new(value))),
            Err(Error::NoEntry) => Ok(None),
            Err(error) => Err(map_keyring_error(&error, 0)),
        }
    }

    fn set(&self, key: &SecretKey, value: &[u8]) -> Result<(), SecretStoreError> {
        if self.max.is_some_and(|max| value.len() > max) {
            return Err(SecretStoreError::TooLong(value.len()));
        }
        self.entry(key)?
            .set_secret(value)
            .map_err(|e| map_keyring_error(&e, value.len()))
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretStoreError> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(error) => Err(map_keyring_error(&error, 0)),
        }
    }

    fn max_value_len(&self) -> Option<usize> {
        self.max
    }
}

#[cfg(test)]
mod tests;
