//! Single instance: the first process becomes primary; later ones forward and exit.

use std::path::Path;

use prm_domain::ports::{PortError, PortErrorKind};

use crate::shared::instance_key::{InstanceKey, InstanceScope, derive};

pub enum Acquired {
    Primary(InstanceLock),
    Secondary,
}

/// Held for the primary's lifetime; dropping it releases the instance.
pub struct InstanceLock {
    #[cfg(windows)]
    _mutex: crate::win::instance::OwnedMutex,
    #[cfg(not(windows))]
    _file: std::fs::File,
}

/// Windows: a default-scope mutex owned by another user (runas) falls back to the isolated scope.
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

#[cfg(windows)]
fn acquire_windows(
    root: &Path,
    scope: InstanceScope,
) -> Result<(Acquired, InstanceKey), PortError> {
    use crate::win::instance::{MutexOutcome, create_mutex};
    let key = derive(root, scope, crate::process::session_id());
    match create_mutex(&key.mutex_name)? {
        MutexOutcome::Owned(mutex) => Ok((Acquired::Primary(InstanceLock { _mutex: mutex }), key)),
        MutexOutcome::Exists => Ok((Acquired::Secondary, key)),
        MutexOutcome::AccessDenied if scope == InstanceScope::Default => {
            acquire_windows(root, InstanceScope::Isolated)
        }
        MutexOutcome::AccessDenied => Err(PortError::new(
            PortErrorKind::Denied,
            "another user owns this instance",
        )),
    }
}

#[cfg(not(windows))]
fn acquire_unix(root: &Path, scope: InstanceScope) -> Result<(Acquired, InstanceKey), PortError> {
    use std::fs::{OpenOptions, TryLockError};
    let key = derive(root, scope, None);
    let failed = |e: std::io::Error| PortError::new(PortErrorKind::Failed, e.to_string());
    std::fs::create_dir_all(root).map_err(failed)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&key.lock_file)
        .map_err(failed)?;
    match file.try_lock() {
        Ok(()) => Ok((Acquired::Primary(InstanceLock { _file: file }), key)),
        Err(TryLockError::WouldBlock) => Ok((Acquired::Secondary, key)),
        Err(TryLockError::Error(error)) => Err(failed(error)),
    }
}

#[cfg(test)]
mod tests;
