//! Do-nothing adapters for platforms (or failures) without the real thing.

use std::path::Path;

use prm_domain::{
    notify::Notification,
    ports::{PortError, PortErrorKind},
};
use prm_service::ports::{Autostart, AutostartState, Notifier, NotifierCapability};

use crate::{
    shared::{
        colors::TitleBarColors,
        session_kind::{SessionInfo, SessionKind},
    },
    shell::DesktopShell,
};

fn unsupported() -> PortError {
    PortError::new(PortErrorKind::Unsupported, "not supported on this platform")
}

#[derive(Copy, Clone, Debug, Default)]
pub struct NullNotifier;

impl Notifier for NullNotifier {
    fn show(&self, _notification: &Notification) -> Result<(), PortError> {
        Ok(())
    }

    fn retract(&self, _tag: &str, _group: &str) -> Result<(), PortError> {
        Ok(())
    }

    fn capability(&self) -> NotifierCapability {
        NotifierCapability::Unsupported
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct NullAutostart;

impl Autostart for NullAutostart {
    fn state(&self) -> Result<AutostartState, PortError> {
        Ok(AutostartState::Unsupported)
    }

    fn set_enabled(&self, _enabled: bool) -> Result<(), PortError> {
        Err(unsupported())
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct NoopShell;

impl DesktopShell for NoopShell {
    fn apply_title_bar(
        &self,
        _window: &dyn raw_window_handle::HasWindowHandle,
        _colors: &TitleBarColors,
    ) -> Result<(), PortError> {
        Err(unsupported())
    }

    fn set_cloaked(
        &self,
        _window: &dyn raw_window_handle::HasWindowHandle,
        _cloaked: bool,
    ) -> Result<(), PortError> {
        Err(unsupported())
    }

    fn session(&self) -> SessionInfo {
        SessionInfo {
            kind: SessionKind::Local,
            id: None,
            remote_agent_host: false,
        }
    }

    fn taskbar_light_theme(&self) -> Option<bool> {
        None
    }

    fn reveal_path(&self, _path: &Path) -> Result<(), PortError> {
        Err(unsupported())
    }

    fn os_description(&self) -> &str {
        std::env::consts::OS
    }

    fn system_animations_enabled(&self) -> Option<bool> {
        None
    }
}
