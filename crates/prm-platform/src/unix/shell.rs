//! Desktop integration on macOS and Linux (no title-bar theming or cloaking).

use prm_domain::ports::PortError;
use raw_window_handle::HasWindowHandle;

use crate::{
    common::unsupported,
    shared::{
        colors::WindowChrome,
        session_kind::{SessionInfo, SessionKind},
    },
    shell::DesktopShell,
};

pub(crate) struct UnixShell {
    os: String,
}

impl UnixShell {
    pub(crate) fn new() -> Self {
        Self {
            os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        }
    }
}

impl DesktopShell for UnixShell {
    fn apply_chrome(
        &self,
        _window: &dyn HasWindowHandle,
        _chrome: &WindowChrome,
    ) -> Result<(), PortError> {
        Err(unsupported())
    }

    fn set_cloaked(&self, _window: &dyn HasWindowHandle, _cloaked: bool) -> Result<(), PortError> {
        Err(unsupported())
    }

    fn session(&self) -> SessionInfo {
        let ssh = std::env::var_os("SSH_CONNECTION").is_some()
            || std::env::var_os("SSH_CLIENT").is_some();
        SessionInfo {
            kind: if ssh {
                SessionKind::OtherRemote
            } else {
                SessionKind::Local
            },
            id: None,
        }
    }

    fn os_description(&self) -> &str {
        &self.os
    }

    fn system_animations_enabled(&self) -> Option<bool> {
        None
    }
}
