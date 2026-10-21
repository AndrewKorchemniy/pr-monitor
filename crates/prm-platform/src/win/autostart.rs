//! Launch at login: the HKCU `Run` value, plus Task Manager's "Startup apps" approval.

use std::path::PathBuf;

use prm_domain::ports::PortError;
use prm_service::ports::{Autostart, AutostartState};
use windows_registry::{CURRENT_USER, Type};

use super::{is_not_found, port_error};
use crate::shared::run_command::{
    STARTUP_APPROVED_ENABLED, format_run_command, parse_run_command, startup_approved_enabled,
};

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

/// Deletes one HKCU value; a missing key or value counts as removed. (`Key::open` is
/// read-only, and deleting a value needs write access.)
fn remove_value(path: &str, name: &str) -> Result<(), PortError> {
    let removed = CURRENT_USER
        .options()
        .read()
        .write()
        .open(path)
        .and_then(|key| key.remove_value(name));
    match removed {
        Ok(()) => Ok(()),
        Err(error) if is_not_found(&error) => Ok(()),
        Err(error) => Err(port_error(&error)),
    }
}

/// Removes the `Run` value and its approval flag (uninstall); missing ones count as removed.
pub(crate) fn remove_run_values(value_name: &str) -> Result<(), PortError> {
    remove_value(RUN, value_name)?;
    remove_value(APPROVED, value_name)
}

pub(crate) struct WinAutostart {
    exe: PathBuf,
    home: Option<PathBuf>,
    value_name: String,
}

impl WinAutostart {
    pub(crate) fn new(exe: PathBuf, home: Option<PathBuf>, value_name: String) -> Self {
        Self {
            exe,
            home,
            value_name,
        }
    }

    /// The command line in our `Run` value, if there is one.
    fn run_command(&self) -> Option<String> {
        CURRENT_USER
            .open(RUN)
            .and_then(|key| key.get_string(&self.value_name))
            .ok()
    }

    fn approval(&self) -> Option<Vec<u8>> {
        CURRENT_USER
            .open(APPROVED)
            .and_then(|key| key.get_value(&self.value_name))
            .ok()
            .map(|value| value.to_vec())
    }

    /// The Run value points at another copy of the exe: rewrite it. `true` when it changed.
    pub(crate) fn heal_path(&self) -> Result<bool, PortError> {
        let Some(command) = self.run_command() else {
            return Ok(false);
        };
        if parse_run_command(&command).is_some_and(|exe| exe == self.exe) {
            return Ok(false);
        }
        self.write_run_value()?;
        Ok(true)
    }

    fn write_run_value(&self) -> Result<(), PortError> {
        let command = format_run_command(&self.exe, self.home.as_deref())?;
        CURRENT_USER
            .create(RUN)
            .and_then(|key| key.set_string(&self.value_name, &command))
            .map_err(|e| port_error(&e))
    }
}

impl Autostart for WinAutostart {
    fn state(&self) -> Result<AutostartState, PortError> {
        if self.run_command().is_none() {
            return Ok(AutostartState::Disabled);
        }
        Ok(if startup_approved_enabled(self.approval().as_deref()) {
            AutostartState::Enabled
        } else {
            AutostartState::DisabledByUser
        })
    }

    fn set_enabled(&self, enabled: bool) -> Result<(), PortError> {
        if !enabled {
            return remove_value(RUN, &self.value_name);
        }
        self.write_run_value()?;
        // The user turned it on in the app: clear a "disabled in Task Manager" flag too.
        if !startup_approved_enabled(self.approval().as_deref()) {
            CURRENT_USER
                .create(APPROVED)
                .and_then(|key| {
                    key.set_bytes(&self.value_name, Type::Bytes, &STARTUP_APPROVED_ENABLED)
                })
                .map_err(|e| port_error(&e))?;
        }
        Ok(())
    }
}
