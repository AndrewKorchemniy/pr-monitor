//! Picking the secret backend: the OS keyring by default, DPAPI files on request (non-persistent
//! desktops), memory as the last resort.

use std::path::Path;

use prm_auth::{InMemorySecretStore, SecretKey, SecretStore, SecretStoreError};
use prm_domain::secret::SecretBytes;

use crate::common::keyring_store::KeyringSecretStore;

/// From `PRMONITOR_SECRET_STORE` (case-insensitive); anything else is `Auto`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum SecretBackendPref {
    #[default]
    Auto,
    Keyring,
    Dpapi,
    Memory,
}

impl SecretBackendPref {
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("keyring") => Self::Keyring,
            Some("dpapi") => Self::Dpapi,
            Some("memory") => Self::Memory,
            Some(_) | None => Self::Auto,
        }
    }
}

pub enum PlatformSecretStore {
    Keyring(KeyringSecretStore),
    #[cfg(windows)]
    Dpapi(crate::win::dpapi::DpapiFileStore),
    Memory(InMemorySecretStore),
}

impl PlatformSecretStore {
    /// For diagnostics.
    pub const fn backend(&self) -> &'static str {
        match self {
            Self::Keyring(_) => "keyring",
            #[cfg(windows)]
            Self::Dpapi(_) => "dpapi",
            Self::Memory(_) => "memory",
        }
    }

    fn inner(&self) -> &dyn SecretStore {
        match self {
            Self::Keyring(store) => store,
            #[cfg(windows)]
            Self::Dpapi(store) => store,
            Self::Memory(store) => store,
        }
    }
}

impl SecretStore for PlatformSecretStore {
    fn get(&self, key: &SecretKey) -> Result<Option<SecretBytes>, SecretStoreError> {
        self.inner().get(key)
    }

    fn set(&self, key: &SecretKey, value: &[u8]) -> Result<(), SecretStoreError> {
        self.inner().set(key, value)
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretStoreError> {
        self.inner().delete(key)
    }

    fn max_value_len(&self) -> Option<usize> {
        self.inner().max_value_len()
    }
}

/// Never fails: a backend that cannot start degrades to memory (secrets will not persist).
pub fn secret_store(root: &Path, pref: SecretBackendPref) -> PlatformSecretStore {
    if pref == SecretBackendPref::Memory {
        return PlatformSecretStore::Memory(InMemorySecretStore::default());
    }
    #[cfg(windows)]
    if pref == SecretBackendPref::Dpapi {
        let dir = root.join("secrets");
        return PlatformSecretStore::Dpapi(crate::win::dpapi::DpapiFileStore::new(dir));
    }
    let _ = root;
    match os_keyring() {
        Ok(store) => PlatformSecretStore::Keyring(store),
        Err(error) => {
            tracing::warn!(%error, "no OS secret store; secrets will not persist");
            PlatformSecretStore::Memory(InMemorySecretStore::default())
        }
    }
}

/// Credential Manager stores at most this many bytes per credential.
#[cfg(windows)]
const CREDMAN_MAX_BLOB: usize = 2_560;

/// Credential Manager, local persistence (tokens must not roam), `PRMonitor/` targets.
#[cfg(windows)]
fn os_keyring() -> keyring_core::Result<KeyringSecretStore> {
    let store: std::sync::Arc<keyring_core::CredentialStore> =
        windows_native_keyring_store::Store::new()?;
    Ok(KeyringSecretStore::new(
        store,
        Some(prm_domain::app::CREDMAN_TARGET_PREFIX),
        Some("Local"),
        Some(CREDMAN_MAX_BLOB),
    ))
}

#[cfg(target_os = "macos")]
fn os_keyring() -> keyring_core::Result<KeyringSecretStore> {
    let store: std::sync::Arc<keyring_core::CredentialStore> =
        apple_native_keyring_store::keychain::Store::new()?;
    Ok(KeyringSecretStore::new(store, None, None, None))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn os_keyring() -> keyring_core::Result<KeyringSecretStore> {
    let store: std::sync::Arc<keyring_core::CredentialStore> =
        zbus_secret_service_keyring_store::Store::new()?;
    Ok(KeyringSecretStore::new(store, None, None, None))
}

#[cfg(test)]
mod tests;
