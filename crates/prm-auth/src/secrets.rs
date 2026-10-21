//! Where PATs and refresh tokens live. OS stores are implemented in prm-platform; this crate
//! has the trait, a chunking adapter (Credential Manager caps values at 2560 bytes) and an
//! in-memory store.

pub mod chunked;
pub mod memory;

use std::sync::Arc;

use prm_domain::{OrgId, secret::SecretBytes};

pub use chunked::ChunkedSecretStore;
pub use memory::InMemorySecretStore;

/// A named slot in the secret store, scoped per org.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SecretKey(String);

impl SecretKey {
    pub fn pat(org: &OrgId) -> Self {
        Self(format!("{org}/pat"))
    }

    pub fn entra_refresh(org: &OrgId) -> Self {
        Self(format!("{org}/entra-rt"))
    }

    /// Every slot an org can have. Uninstall deletes these where the store cannot be listed,
    /// so a new per-org kind must be added here.
    pub fn all_for(org: &OrgId) -> [Self; 2] {
        [Self::pat(org), Self::entra_refresh(org)]
    }

    /// The `index`-th chunk of a value too large for one slot.
    pub fn part(&self, index: usize) -> Self {
        Self(format!("{}#{index}", self.0))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SecretStoreError {
    #[error("secret store unavailable: {0}")]
    Unavailable(Arc<str>),
    #[error("value too long ({0} bytes)")]
    TooLong(usize),
    #[error("stored secret is corrupt: {0}")]
    Corrupt(Arc<str>),
    #[error("{0}")]
    Other(Arc<str>),
}

pub trait SecretStore: Send + Sync + 'static {
    fn get(&self, key: &SecretKey) -> Result<Option<SecretBytes>, SecretStoreError>;
    fn set(&self, key: &SecretKey, value: &[u8]) -> Result<(), SecretStoreError>;
    /// Succeeds when the key is absent.
    fn delete(&self, key: &SecretKey) -> Result<(), SecretStoreError>;
    /// The largest value one slot holds, if limited.
    fn max_value_len(&self) -> Option<usize>;
}

#[cfg(test)]
mod tests;
