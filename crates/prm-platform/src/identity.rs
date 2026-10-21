//! Shell identity at startup: AUMID, toast identity, URL protocol, the icon file. Windows only;
//! other platforms report everything as skipped. Failures are logged where they happen.

use std::path::Path;

use crate::shared::registration::RegistrationScope;

/// What `ensure_shell_identity` registers, and for which exe.
pub struct RegistrationSpec<'a> {
    pub exe: &'a Path,
    /// Where the toast icon is written (under the data root).
    pub icon_png_path: &'a Path,
    pub icon_png: &'static [u8],
    pub scope: RegistrationScope,
}

/// Whether toasts can show under our own name. `Failed` makes the notifier fall back to
/// another app's identity, so toasts still appear.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum IdentityStatus {
    /// Not attempted (another OS, or already registered by the installed app).
    #[default]
    Skipped,
    Registered,
    Failed,
}

/// Synchronous (about a millisecond of registry work); call before any window exists.
/// Returns the toast identity's status; every failure is logged.
pub fn ensure_shell_identity(spec: &RegistrationSpec<'_>) -> IdentityStatus {
    #[cfg(windows)]
    let status = crate::win::registration::ensure(spec);
    #[cfg(not(windows))]
    let status = {
        let _ = spec;
        IdentityStatus::Skipped
    };
    status
}
