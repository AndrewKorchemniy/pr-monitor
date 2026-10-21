//! Single instance: the first process becomes primary; later ones forward and exit. Every
//! primary holds an exclusive lock on `<root>/instance.lock`, the guard for its data folder;
//! on Windows it also owns a session-local named mutex.

use std::{
    fs::{File, OpenOptions, TryLockError},
    io,
    path::Path,
};

use prm_domain::ports::PortError;
#[cfg(windows)]
use prm_domain::ports::PortErrorKind;

use crate::{
    common::failed,
    shared::instance_key::{InstanceKey, InstanceScope, derive},
};

/// The outcome of `acquire_instance`.
pub enum Acquired {
    /// This process owns the instance until the lock is dropped.
    Primary(InstanceLock),
    /// Another process owns it: forward the request to it.
    Secondary,
}

/// Held for the primary's lifetime; dropping it releases the instance.
pub struct InstanceLock {
    // why: fields drop in declaration order. The file lock goes first, so a launch that wins
    // the mutex never finds the file still locked by the primary it replaces.
    _file: File,
    #[cfg(windows)]
    _mutex: crate::win::instance::OwnedMutex,
}

/// Windows: a default-scope mutex owned by another user (runas) falls back to the isolated
/// scope; one owned by this user's elevated primary on the same data folder is an error.
pub fn acquire_instance(
    root: &Path,
    scope: InstanceScope,
) -> Result<(Acquired, InstanceKey), PortError> {
    #[cfg(windows)]
    let acquired = acquire_windows(root, scope);
    #[cfg(not(windows))]
    let acquired = acquire_unix(root, scope);
    acquired
}

/// The data folder's lock file, whether we hold it.
enum FileLock {
    Held(File),
    Busy,
}

/// Opens (creating the folder and file if needed) and tries to lock `<root>/instance.lock`.
fn lock_file(root: &Path, key: &InstanceKey) -> Result<FileLock, PortError> {
    std::fs::create_dir_all(root).map_err(failed)?;
    let opened = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&key.lock_file);
    let file = match opened {
        Ok(file) => file,
        // why: an elevated primary may have created it; read access is enough to lock it.
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            File::open(&key.lock_file).map_err(failed)?
        }
        Err(error) => return Err(failed(error)),
    };
    match file.try_lock() {
        Ok(()) => Ok(FileLock::Held(file)),
        Err(TryLockError::WouldBlock) => Ok(FileLock::Busy),
        Err(TryLockError::Error(error)) => Err(failed(error)),
    }
}

#[cfg(windows)]
fn acquire_windows(
    root: &Path,
    scope: InstanceScope,
) -> Result<(Acquired, InstanceKey), PortError> {
    use crate::win::instance::{MutexOutcome, create_mutex};
    let key = derive(root, scope, crate::process::session_id());
    match create_mutex(&key.mutex_name)? {
        MutexOutcome::Owned(mutex) => match lock_file(root, &key)? {
            FileLock::Held(file) => {
                let lock = InstanceLock {
                    _file: file,
                    _mutex: mutex,
                };
                Ok((Acquired::Primary(lock), key))
            }
            FileLock::Busy => Err(PortError::new(
                PortErrorKind::Denied,
                "another PR Monitor is already using this data folder",
            )),
        },
        MutexOutcome::Exists => Ok((Acquired::Secondary, key)),
        MutexOutcome::AccessDenied => access_denied(root, scope, &key),
    }
}

/// The mutex exists but we may not open it: another user (runas) or this user's elevated
/// primary owns it. Only the elevated primary shares our data folder, and it holds its lock;
/// its pipe rejects us too, so there is nobody to forward to.
#[cfg(windows)]
fn access_denied(
    root: &Path,
    scope: InstanceScope,
    key: &InstanceKey,
) -> Result<(Acquired, InstanceKey), PortError> {
    match (lock_file(root, key)?, scope) {
        (FileLock::Busy, _) => Err(PortError::new(
            PortErrorKind::Denied,
            "PR Monitor is already running as administrator",
        )),
        (FileLock::Held(probe), InstanceScope::Default) => {
            // why: released before the isolated attempt locks the same file again.
            drop(probe);
            acquire_windows(root, InstanceScope::Isolated)
        }
        (FileLock::Held(_), InstanceScope::Isolated) => Err(PortError::new(
            PortErrorKind::Denied,
            "another user owns this instance",
        )),
    }
}

#[cfg(not(windows))]
fn acquire_unix(root: &Path, scope: InstanceScope) -> Result<(Acquired, InstanceKey), PortError> {
    let key = derive(root, scope, None);
    match lock_file(root, &key)? {
        FileLock::Held(file) => Ok((Acquired::Primary(InstanceLock { _file: file }), key)),
        FileLock::Busy => Ok((Acquired::Secondary, key)),
    }
}

#[cfg(test)]
mod tests;
