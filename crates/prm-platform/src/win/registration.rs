//! Shell identity: the process AUMID, the toast identity key (name and icon in Action Center)
//! and the `prmonitor:` URL protocol. Values are written only when they differ, so a moved
//! portable exe heals itself without churn. Each failure is logged where it happens.

#![allow(
    unsafe_code,
    reason = "one Win32 call: SetCurrentProcessExplicitAppUserModelID"
)]

use std::fs;

use prm_domain::app::{APP_DISPLAY_NAME, AUMID, PROTOCOL_DESCRIPTION, URI_SCHEME};
use windows::{Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID, core::HSTRING};
use windows_registry::{CURRENT_USER, Key};

use crate::{
    identity::{IdentityStatus, RegistrationSpec},
    shared::{registration::RegistrationScope, run_command::protocol_command},
};

fn aumid_key() -> String {
    format!(r"Software\Classes\AppUserModelId\{AUMID}")
}

fn protocol_key() -> String {
    format!(r"Software\Classes\{URI_SCHEME}")
}

/// Writes `value` unless it is already there.
fn upsert(key: &Key, name: &str, value: &str) -> windows_registry::Result<()> {
    match key.get_string(name) {
        Ok(current) if current == value => Ok(()),
        _ => key.set_string(name, value),
    }
}

fn log_failure(step: &str, error: &dyn std::fmt::Display) {
    tracing::warn!(step, error = %error, "shell registration failed");
}

/// Returns the toast identity's status (the only part a caller acts on).
pub(crate) fn ensure(spec: &RegistrationSpec<'_>) -> IdentityStatus {
    let aumid = HSTRING::from(AUMID);
    // SAFETY: `aumid` is NUL-terminated and outlives the call.
    if let Err(error) = unsafe { SetCurrentProcessExplicitAppUserModelID(&aumid) } {
        log_failure("process AUMID", &error.message());
    }
    write_icon(spec);
    let toast = toast_identity(spec);
    protocol(spec);
    toast
}

fn write_icon(spec: &RegistrationSpec<'_>) {
    if fs::read(spec.icon_png_path).is_ok_and(|current| current == spec.icon_png) {
        return;
    }
    if let Err(error) = prm_config::store::atomic_write(spec.icon_png_path, spec.icon_png) {
        log_failure("icon", &error);
    }
}

fn status(result: windows_registry::Result<()>, step: &str) -> IdentityStatus {
    match result {
        Ok(()) => IdentityStatus::Registered,
        Err(error) => {
            log_failure(step, &error.message());
            IdentityStatus::Failed
        }
    }
}

fn toast_identity(spec: &RegistrationSpec<'_>) -> IdentityStatus {
    let path = aumid_key();
    if spec.scope == RegistrationScope::ToastOnlyIfMissing && CURRENT_USER.open(&path).is_ok() {
        return IdentityStatus::Skipped;
    }
    let icon = spec.icon_png_path.to_string_lossy();
    let written = CURRENT_USER.create(&path).and_then(|key| {
        upsert(&key, "DisplayName", APP_DISPLAY_NAME)?;
        upsert(&key, "IconUri", &icon)
    });
    status(written, "toast identity")
}

fn protocol(spec: &RegistrationSpec<'_>) {
    if spec.scope != RegistrationScope::Full {
        return;
    }
    let command = match protocol_command(spec.exe) {
        Ok(command) => command,
        Err(error) => return log_failure("protocol", &error.message),
    };
    let icon = format!("\"{}\",0", spec.exe.display());
    let written = CURRENT_USER.create(protocol_key()).and_then(|key| {
        upsert(&key, "", PROTOCOL_DESCRIPTION)?;
        upsert(&key, "URL Protocol", "")?;
        upsert(&key.create("DefaultIcon")?, "", &icon)?;
        upsert(&key.create(r"shell\open\command")?, "", &command)
    });
    if let Err(error) = written {
        log_failure("protocol", &error.message());
    }
}

/// Deletes a key tree under HKCU; a missing key counts as removed.
fn remove_tree(path: &str) -> Result<(), String> {
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
