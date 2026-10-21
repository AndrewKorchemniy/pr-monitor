//! Finding and deleting every PR Monitor credential (uninstall), by target-name prefix.

#![allow(
    unsafe_code,
    reason = "Win32 FFI; each block documents why it is sound"
)]

use std::collections::HashMap;

use prm_domain::{app::SECRET_SERVICE, ports::PortError};
use windows::{
    Win32::{
        Foundation::ERROR_NOT_FOUND,
        Security::Credentials::{CREDENTIALW, CredEnumerateW, CredFree},
    },
    core::{HRESULT, HSTRING},
};

use super::port_error;

/// Target names of every generic credential starting with `prefix`.
pub(crate) fn credman_targets(prefix: &str) -> Result<Vec<String>, PortError> {
    let filter = HSTRING::from(format!("{prefix}*"));
    let mut count = 0_u32;
    let mut creds: *mut *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: `filter` is NUL-terminated; both out-pointers are locals; the returned array is
    // freed with CredFree below.
    let listed = unsafe { CredEnumerateW(&filter, None, &raw mut count, &raw mut creds) };
    match listed {
        Ok(()) => {}
        Err(error) if error.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0) => {
            return Ok(Vec::new());
        }
        Err(error) => return Err(port_error(&error)),
    }
    let len = usize::try_from(count).unwrap_or(0);
    // SAFETY: CredEnumerateW returned `count` valid pointers, alive until CredFree.
    let list = unsafe { std::slice::from_raw_parts(creds, len) };
    let mut targets = Vec::with_capacity(len);
    for &cred in list {
        // SAFETY: each pointer refers to a CREDENTIALW alive until CredFree.
        let name = unsafe { (*cred).TargetName };
        // SAFETY: TargetName is a NUL-terminated string owned by the same allocation.
        if let Ok(target) = unsafe { name.to_string() } {
            targets.push(target);
        }
    }
    // SAFETY: frees the array CredEnumerateW allocated, exactly once, after the last use.
    unsafe { CredFree(creds.cast()) };
    Ok(targets)
}

/// Deletes each credential by target name. Returns how many failed.
pub(crate) fn delete_targets(targets: &[String]) -> Result<usize, PortError> {
    let store = windows_native_keyring_store::Store::new().map_err(crate::common::failed)?;
    let mut failures = 0;
    for target in targets {
        let modifiers = HashMap::from([("target", target.as_str())]);
        let deleted = keyring_core::api::CredentialStoreApi::build(
            store.as_ref(),
            SECRET_SERVICE,
            "",
            Some(&modifiers),
        )
        .and_then(|entry| entry.delete_credential());
        if deleted.is_err() {
            failures += 1;
        }
    }
    Ok(failures)
}
