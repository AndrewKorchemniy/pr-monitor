//! Desktop integration on macOS and Linux (no title-bar theming or cloaking).

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use prm_domain::ports::{PortError, PortErrorKind};
use raw_window_handle::HasWindowHandle;

use crate::{
    shared::{
        colors::TitleBarColors,
        session_kind::{SessionInfo, SessionKind},
    },
    shell::DesktopShell,
};

pub(crate) struct UnixShell {
    root: PathBuf,
    os: String,
}

impl UnixShell {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        }
    }
}

fn unsupported() -> PortError {
    PortError::new(PortErrorKind::Unsupported, "not supported on this platform")
}

impl DesktopShell for UnixShell {
    fn apply_title_bar(
        &self,
        _window: &dyn HasWindowHandle,
        _colors: &TitleBarColors,
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
            remote_agent_host: false,
        }
    }

    fn taskbar_light_theme(&self) -> Option<bool> {
        None
    }

    fn reveal_path(&self, path: &Path) -> Result<(), PortError> {
        let denied = || PortError::new(PortErrorKind::Denied, "outside the data folder");
        let target = path.canonicalize().map_err(|_| denied())?;
        let root = self.root.canonicalize().map_err(|_| denied())?;
        if !target.starts_with(&root) {
            return Err(denied());
        }
        let mut command = if cfg!(target_os = "macos") {
            let mut open = Command::new("open");
            open.arg("-R").arg(&target);
            open
        } else {
            let mut xdg = Command::new("xdg-open");
            xdg.arg(if target.is_dir() {
                target.as_path()
            } else {
                target.parent().unwrap_or(&root)
            });
            xdg
        };
        command
            .spawn()
            .map(drop)
            .map_err(|e| PortError::new(PortErrorKind::Failed, e.to_string()))
    }

    fn os_description(&self) -> &str {
        &self.os
    }

    fn system_animations_enabled(&self) -> Option<bool> {
        None
    }
}
