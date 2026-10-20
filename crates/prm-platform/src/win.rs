//! Windows adapters. Every `unsafe` block makes one Win32 call and states why it is sound.

pub(crate) mod autostart;
pub(crate) mod console;
pub(crate) mod credman;
pub(crate) mod dpapi;
pub(crate) mod instance;
pub(crate) mod process;
pub(crate) mod registration;
pub(crate) mod session;
pub(crate) mod shell;
pub(crate) mod toast;
pub(crate) mod winhttp;

#[cfg(test)]
mod smoke;

use prm_domain::ports::{PortError, PortErrorKind};

/// A Win32/WinRT error as a port error (message only; codes go to the log).
pub(crate) fn port_error(error: &windows::core::Error) -> PortError {
    PortError::new(PortErrorKind::Failed, error.message())
}

/// `HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND)`: a registry key or value that is not there.
pub(crate) fn is_not_found(error: &windows::core::Error) -> bool {
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    error.code() == windows::core::HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)
}
