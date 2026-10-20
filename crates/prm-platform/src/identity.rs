//! Shell identity at startup: AUMID, toast identity, URL protocol, the icon file. Windows only;
//! other platforms report everything as skipped.

use std::path::Path;

use crate::shared::registration::RegistrationScope;

pub struct RegistrationSpec<'a> {
    pub exe: &'a Path,
    /// Where the toast icon is written (under the data root).
    pub icon_png_path: &'a Path,
    pub icon_png: &'static [u8],
    pub scope: RegistrationScope,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum IdentityStatus {
    #[default]
    Skipped,
    Registered {
        changed: bool,
    },
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RegistrationReport {
    pub process_aumid_set: bool,
    pub toast_identity: IdentityStatus,
    pub protocol: IdentityStatus,
    pub icon_written: bool,
    pub errors: Vec<String>,
}

/// Synchronous (about a millisecond of registry work); call before any window exists.
pub fn ensure_shell_identity(spec: &RegistrationSpec<'_>) -> RegistrationReport {
    #[cfg(windows)]
    let report = crate::win::registration::ensure(spec);
    #[cfg(not(windows))]
    let report = {
        let _ = spec;
        RegistrationReport::default()
    };
    report
}
