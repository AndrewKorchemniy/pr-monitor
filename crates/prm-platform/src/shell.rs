//! `DesktopShell`: window chrome and desktop integration the UI needs from the OS.

use std::path::Path;

use prm_domain::ports::PortError;

use crate::shared::{colors::TitleBarColors, session_kind::SessionInfo};

pub trait DesktopShell: Send + Sync + 'static {
    /// Title bar colors (Windows 11 DWM); `Unsupported` elsewhere.
    fn apply_title_bar(
        &self,
        window: &dyn raw_window_handle::HasWindowHandle,
        colors: &TitleBarColors,
    ) -> Result<(), PortError>;
    /// Hides the window from the screen and taskbar without unmapping it (start hidden).
    fn set_cloaked(
        &self,
        window: &dyn raw_window_handle::HasWindowHandle,
        cloaked: bool,
    ) -> Result<(), PortError>;
    /// Re-probed on every call; cheap.
    fn session(&self) -> SessionInfo;
    /// For tray glyph contrast; `None` when unknown.
    fn taskbar_light_theme(&self) -> Option<bool>;
    /// Opens the folder (or selects the file) in the file manager; only under the data root.
    fn reveal_path(&self, path: &Path) -> Result<(), PortError>;
    fn os_description(&self) -> &str;
    /// The OS "animate controls" preference, when known.
    fn system_animations_enabled(&self) -> Option<bool>;
}
