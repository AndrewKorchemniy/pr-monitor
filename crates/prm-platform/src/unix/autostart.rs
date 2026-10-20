//! Launch at login via a LaunchAgent (macOS) or an XDG autostart entry (Linux).

use std::path::Path;

use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use prm_domain::ports::{PortError, PortErrorKind};
use prm_service::ports::{Autostart, AutostartState};

pub(crate) struct UnixAutostart {
    launcher: AutoLaunch,
}

fn failed(error: impl std::fmt::Display) -> PortError {
    PortError::new(PortErrorKind::Failed, error.to_string())
}

impl UnixAutostart {
    pub(crate) fn new(exe: &Path) -> Result<Self, String> {
        let launcher = AutoLaunchBuilder::new()
            .set_app_name(prm_domain::app::APP_ID)
            .set_app_path(&exe.to_string_lossy())
            .set_args(&["--minimized"])
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { launcher })
    }
}

impl Autostart for UnixAutostart {
    fn state(&self) -> Result<AutostartState, PortError> {
        if !AutoLaunch::is_support() {
            return Ok(AutostartState::Unsupported);
        }
        let enabled = self.launcher.is_enabled().map_err(failed)?;
        Ok(if enabled {
            AutostartState::Enabled
        } else {
            AutostartState::Disabled
        })
    }

    fn set_enabled(&self, enabled: bool) -> Result<(), PortError> {
        if enabled {
            self.launcher.enable().map_err(failed)
        } else {
            self.launcher.disable().map_err(failed)
        }
    }
}
