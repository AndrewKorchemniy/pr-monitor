//! Launch at login via a `LaunchAgent` (macOS) or an XDG autostart entry (Linux). An isolated
//! data root has its own entry, which repeats `--home`.

use std::path::Path;

use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use prm_domain::ports::PortError;
use prm_service::ports::{Autostart, AutostartState};

use crate::common::failed;

pub(crate) struct UnixAutostart {
    launcher: AutoLaunch,
}

impl UnixAutostart {
    /// `name` is the entry name (`autostart_entry_name`); `home` the `--home` override, if any.
    pub(crate) fn new(exe: &Path, name: &str, home: Option<&Path>) -> Result<Self, PortError> {
        let mut args = vec!["--minimized".to_owned()];
        if let Some(home) = home {
            args.push("--home".to_owned());
            args.push(home.to_string_lossy().into_owned());
        }
        let launcher = AutoLaunchBuilder::new()
            .set_app_name(name)
            .set_app_path(&exe.to_string_lossy())
            .set_args(args.as_slice())
            .build()
            .map_err(failed)?;
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
