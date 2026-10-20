//! Secrets as DPAPI-encrypted files under the data root (opt-in for non-persistent desktops,
//! where Credential Manager does not survive logoff but the profile folder does).

#![allow(
    unsafe_code,
    reason = "Win32 FFI; each block documents why it is sound"
)]

use std::{fs, io, path::PathBuf};

use prm_auth::{SecretKey, SecretStore, SecretStoreError};
use prm_domain::secret::SecretBytes;
use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    },
    core::w,
};

use crate::shared::secret_names::dpapi_file_name;

/// Mixed into every encryption, so other apps' DPAPI blobs do not decrypt as ours.
const ENTROPY: &[u8] = b"PRMonitor.secrets.v1";

pub struct DpapiFileStore {
    dir: PathBuf,
}

impl DpapiFileStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, key: &SecretKey) -> PathBuf {
        self.dir.join(dpapi_file_name(key))
    }
}

fn unavailable(error: impl std::fmt::Display) -> SecretStoreError {
    SecretStoreError::Unavailable(error.to_string().into())
}

fn blob(bytes: &[u8]) -> Result<CRYPT_INTEGER_BLOB, SecretStoreError> {
    let len = u32::try_from(bytes.len()).map_err(|_| SecretStoreError::TooLong(bytes.len()))?;
    Ok(CRYPT_INTEGER_BLOB {
        cbData: len,
        pbData: bytes.as_ptr().cast_mut(),
    })
}

/// Copies a DPAPI output buffer out (optionally wiping it) and frees it.
fn take(output: CRYPT_INTEGER_BLOB, wipe: bool) -> Vec<u8> {
    if output.pbData.is_null() {
        return Vec::new();
    }
    let len = usize::try_from(output.cbData).unwrap_or(0);
    // SAFETY: DPAPI returned `cbData` initialized bytes at the non-null `pbData`.
    let bytes = unsafe { std::slice::from_raw_parts(output.pbData, len) }.to_vec();
    if wipe {
        // SAFETY: the same buffer, still allocated; zeroing plaintext before it is freed.
        unsafe { std::ptr::write_bytes(output.pbData, 0, len) };
    }
    // SAFETY: DPAPI allocated `pbData` with LocalAlloc; it is freed exactly once here.
    let _ = unsafe { LocalFree(Some(HLOCAL(output.pbData.cast()))) };
    bytes
}

fn protect(plain: &[u8]) -> Result<Vec<u8>, SecretStoreError> {
    let (input, entropy) = (blob(plain)?, blob(ENTROPY)?);
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: `input` and `entropy` describe live slices DPAPI only reads; `output` receives a
    // LocalAlloc buffer that `take` frees.
    unsafe {
        CryptProtectData(
            &raw const input,
            w!("PRMonitor"),
            Some(&raw const entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    }
    .map_err(|e| unavailable(e.message()))?;
    Ok(take(output, false))
}

fn unprotect(sealed: &[u8]) -> Result<SecretBytes, SecretStoreError> {
    let (input, entropy) = (blob(sealed)?, blob(ENTROPY)?);
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: as in `protect`; the plaintext buffer is wiped and freed by `take`.
    unsafe {
        CryptUnprotectData(
            &raw const input,
            None,
            Some(&raw const entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    }
    .map_err(|e| SecretStoreError::Corrupt(e.message().into()))?;
    Ok(SecretBytes::new(take(output, true)))
}

impl SecretStore for DpapiFileStore {
    fn get(&self, key: &SecretKey) -> Result<Option<SecretBytes>, SecretStoreError> {
        match fs::read(self.path(key)) {
            Ok(sealed) => unprotect(&sealed).map(Some),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(unavailable(error)),
        }
    }

    fn set(&self, key: &SecretKey, value: &[u8]) -> Result<(), SecretStoreError> {
        let sealed = protect(value)?;
        fs::create_dir_all(&self.dir).map_err(unavailable)?;
        prm_config::store::atomic_write(&self.path(key), &sealed).map_err(unavailable)
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretStoreError> {
        match fs::remove_file(self.path(key)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(unavailable(error)),
        }
    }

    fn max_value_len(&self) -> Option<usize> {
        None
    }
}
