//! Where secrets live (Credential Manager targets, DPAPI file names) and keyring error mapping.

use prm_auth::{SecretKey, SecretStoreError};
use prm_domain::app::CREDMAN_TARGET_PREFIX;

/// `PRMonitor/<key>`: one prefix, so uninstall can find every credential.
pub fn credman_target(key: &SecretKey) -> String {
    format!("{CREDMAN_TARGET_PREFIX}{}", key.as_str())
}

/// Lowercase hex of the key, so any key is a safe file name.
pub fn dpapi_file_name(key: &SecretKey) -> String {
    use std::fmt::Write as _;
    let mut name = String::with_capacity(key.as_str().len() * 2 + 4);
    for byte in key.as_str().bytes() {
        let _ = write!(name, "{byte:02x}");
    }
    name.push_str(".bin");
    name
}

/// `len` is the value length being stored (for `TooLong`).
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "keyring_core::Error is non_exhaustive; unknown variants map to Other"
)]
pub fn map_keyring_error(error: &keyring_core::Error, len: usize) -> SecretStoreError {
    use keyring_core::Error;
    let message = error.to_string();
    match error {
        Error::NoStorageAccess(_) | Error::PlatformFailure(_) | Error::NoDefaultStore => {
            SecretStoreError::Unavailable(message.into())
        }
        Error::TooLong(..) => SecretStoreError::TooLong(len),
        Error::BadEncoding(_)
        | Error::BadDataFormat(..)
        | Error::BadStoreFormat(_)
        | Error::Ambiguous(_) => SecretStoreError::Corrupt(message.into()),
        _ => SecretStoreError::Other(message.into()),
    }
}

#[cfg(test)]
mod tests;
