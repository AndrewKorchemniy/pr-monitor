//! Where secrets live: DPAPI file names (Credential Manager targets are built by
//! `KeyringSecretStore` from `CREDMAN_TARGET_PREFIX`), and keyring error mapping.

use prm_auth::{SecretKey, SecretStoreError};

/// The folder under the data root that holds the DPAPI secret files.
pub(crate) const SECRETS_DIR_NAME: &str = "secrets";

/// The extension of every DPAPI secret file.
const DPAPI_EXTENSION: &str = ".bin";

/// Lowercase hex of the key, so any key is a safe file name.
pub(crate) fn dpapi_file_name(key: &SecretKey) -> String {
    use std::fmt::Write as _;
    let mut name = String::with_capacity(key.as_str().len() * 2 + DPAPI_EXTENSION.len());
    for byte in key.as_str().bytes() {
        let _ = write!(name, "{byte:02x}");
    }
    name.push_str(DPAPI_EXTENSION);
    name
}

/// Whether a file name is one `dpapi_file_name` makes (so uninstall deletes only our files).
pub(crate) fn is_dpapi_file_name(name: &str) -> bool {
    name.strip_suffix(DPAPI_EXTENSION).is_some_and(|hex| {
        !hex.is_empty()
            && hex.len() % 2 == 0
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// `len` is the value length being stored (for `TooLong`).
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "keyring_core::Error is non_exhaustive; unknown variants map to Other"
)]
pub(crate) fn map_keyring_error(error: &keyring_core::Error, len: usize) -> SecretStoreError {
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
