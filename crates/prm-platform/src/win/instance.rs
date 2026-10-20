//! The single-instance mutex (session-local, owned for the life of the primary).

#![allow(
    unsafe_code,
    reason = "Win32 FFI; each block documents why it is sound"
)]

use prm_domain::ports::PortError;
use windows::{
    Win32::{
        Foundation::{CloseHandle, E_ACCESSDENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE},
        System::Threading::{CreateMutexW, ReleaseMutex},
    },
    core::HSTRING,
};

use super::port_error;

pub(crate) enum MutexOutcome {
    Owned(OwnedMutex),
    /// Another process of ours holds it.
    Exists,
    /// Another user in this session holds it (runas).
    AccessDenied,
}

/// Released and closed on drop. Not `Send`: a mutex is owned by the thread that created it.
pub(crate) struct OwnedMutex(HANDLE);

impl Drop for OwnedMutex {
    fn drop(&mut self) {
        // SAFETY: we own the handle and hold the mutex (created with initial ownership).
        let _ = unsafe { ReleaseMutex(self.0) };
        // SAFETY: we own the handle and close it exactly once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub(crate) fn create_mutex(name: &str) -> Result<MutexOutcome, PortError> {
    let wide = HSTRING::from(name);
    // SAFETY: `wide` is NUL-terminated and outlives the call; null attributes give the default
    // DACL; we own the returned handle.
    let created = unsafe { CreateMutexW(None, true, &wide) };
    match created {
        Ok(handle) => {
            // SAFETY: reads this thread's last error immediately after the call.
            let last = unsafe { GetLastError() };
            if last == ERROR_ALREADY_EXISTS {
                // SAFETY: the handle is ours; we never held this existing mutex.
                let _ = unsafe { CloseHandle(handle) };
                return Ok(MutexOutcome::Exists);
            }
            Ok(MutexOutcome::Owned(OwnedMutex(handle)))
        }
        Err(error) if error.code() == E_ACCESSDENIED => Ok(MutexOutcome::AccessDenied),
        Err(error) => Err(port_error(&error)),
    }
}
