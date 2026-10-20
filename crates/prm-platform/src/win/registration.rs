//! Shell identity: the process AUMID, the toast identity key (name and icon in Action Center)
//! and the `prmonitor:` URL protocol. Values are written only when they differ, so a moved
//! portable exe heals itself without churn.

#![allow(
    unsafe_code,
    reason = "one Win32 call: SetCurrentProcessExplicitAppUserModelID"
)]

use std::fs;

use prm_domain::app::{APP_DISPLAY_NAME, AUMID, PROTOCOL_DESCRIPTION, URI_SCHEME};
use windows::{Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID, core::HSTRING};
use windows_registry::{CURRENT_USER, Key};

use crate::{
    identity::{IdentityStatus, RegistrationReport, RegistrationSpec},
    shared::{registration::RegistrationScope, run_command::protocol_command},
};

fn aumid_key() -> String {
    format!(r"Software\Classes\AppUserModelId\{AUMID}")
}

fn protocol_key() -> String {
    format!(r"Software\Classes\{URI_SCHEME}")
}

/// Writes `value` unless it is already there; `true` when it changed.
fn upsert(key: &Key, name: &str, value: &str) -> windows_registry::Result<bool> {
    match key.get_string(name) {
        Ok(current) if current == value => Ok(false),
        _ => key.set_string(name, value).map(|()| true),
    }
}

pub(crate) fn ensure(spec: &RegistrationSpec<'_>) -> RegistrationReport {
    let mut report = RegistrationReport::default();
    let aumid = HSTRING::from(AUMID);
    // SAFETY: `aumid` is NUL-terminated and outlives the call.
    report.process_aumid_set = unsafe { SetCurrentProcessExplicitAppUserModelID(&aumid) }.is_ok();
    report.icon_written = write_icon(spec, &mut report.errors);
    report.toast_identity = toast_identity(spec, &mut report.errors);
    report.protocol = protocol(spec, &mut report.errors);
    report
}

fn write_icon(spec: &RegistrationSpec<'_>, errors: &mut Vec<String>) -> bool {
    if fs::read(spec.icon_png_path).is_ok_and(|current| current == spec.icon_png) {
        return false;
    }
    match prm_config::store::atomic_write(spec.icon_png_path, spec.icon_png) {
        Ok(()) => true,
        Err(error) => {
            errors.push(format!("icon: {error}"));
            false
        }
    }
}

fn status(
    result: windows_registry::Result<bool>,
    what: &str,
    errors: &mut Vec<String>,
) -> IdentityStatus {
    match result {
        Ok(changed) => IdentityStatus::Registered { changed },
        Err(error) => {
            errors.push(format!("{what}: {}", error.message()));
            IdentityStatus::Failed
        }
    }
}

fn toast_identity(spec: &RegistrationSpec<'_>, errors: &mut Vec<String>) -> IdentityStatus {
    let path = aumid_key();
    if spec.scope == RegistrationScope::ToastOnlyIfMissing && CURRENT_USER.open(&path).is_ok() {
        return IdentityStatus::Skipped;
    }
    let icon = spec.icon_png_path.to_string_lossy();
    let written = CURRENT_USER.create(&path).and_then(|key| {
        let name = upsert(&key, "DisplayName", APP_DISPLAY_NAME)?;
        let icon = upsert(&key, "IconUri", &icon)?;
        Ok(name || icon)
    });
    status(written, "toast identity", errors)
}

fn protocol(spec: &RegistrationSpec<'_>, errors: &mut Vec<String>) -> IdentityStatus {
    if spec.scope != RegistrationScope::Full {
        return IdentityStatus::Skipped;
    }
    let command = match protocol_command(spec.exe) {
        Ok(command) => command,
        Err(error) => {
            errors.push(format!("protocol: {}", error.message));
            return IdentityStatus::Failed;
        }
    };
    let icon = format!("\"{}\",0", spec.exe.display());
    let root = protocol_key();
    let written = CURRENT_USER.create(&root).and_then(|key| {
        let mut changed = upsert(&key, "", PROTOCOL_DESCRIPTION)?;
        changed |= upsert(&key, "URL Protocol", "")?;
        changed |= upsert(&key.create("DefaultIcon")?, "", &icon)?;
        changed |= upsert(&key.create(r"shell\open\command")?, "", &command)?;
        Ok(changed)
    });
    status(written, "protocol", errors)
}

/// Deletes a key tree under HKCU; a missing key counts as removed.
pub(crate) fn remove_tree(path: &str) -> Result<(), String> {
    match CURRENT_USER.remove_tree(path) {
        Ok(()) => Ok(()),
        Err(error) if super::is_not_found(&error) => Ok(()),
        Err(error) => Err(error.message()),
    }
}

pub(crate) fn remove_protocol() -> Result<(), String> {
    remove_tree(&protocol_key())
}

pub(crate) fn remove_toast_identity() -> Result<(), String> {
    remove_tree(&aumid_key())
}
