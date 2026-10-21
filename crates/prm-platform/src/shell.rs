//! `DesktopShell`: window chrome and desktop integration the UI needs from the OS.

use prm_domain::ports::PortError;

use crate::shared::{colors::TitleBarColors, session_kind::SessionInfo};

/// The OS side of the window: chrome, cloaking, session and display facts. One per process.
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
    /// A one-line OS name and version for the log and diagnostics; computed once.
    fn os_description(&self) -> &str;
    /// The OS "animate controls" preference, when known.
    fn system_animations_enabled(&self) -> Option<bool>;
    /// Whether a saved window rect (in points) still has its title bar on a connected monitor;
    /// `None` when the OS cannot tell.
    fn rect_visible(&self, x: f32, y: f32, w: f32, h: f32) -> Option<bool> {
        let _ = (x, y, w, h);
        None
    }
}

/// The `(left, top, right, bottom)` pixel rect to look for a monitor under: the title-bar strip
/// of a saved rect, stretched to cover every display scale from 1 to 3, because the saved rect
/// is in points and monitors are in pixels. Lenient on purpose: it only has to rule out a rect
/// left behind by a disconnected monitor.
#[cfg(any(windows, test))]
pub(crate) fn title_bar_probe(x: f32, y: f32, w: f32) -> [i32; 4] {
    /// Widest title-bar strip probed.
    const PROBE_WIDTH: f32 = 200.0;
    /// Height of the title-bar strip.
    const PROBE_HEIGHT: f32 = 30.0;
    /// The largest display scale assumed when converting points to pixels.
    const MAX_SCALE: f32 = 3.0;
    let strip = w.clamp(0.0, PROBE_WIDTH);
    let (left, right) = (x.min(x * MAX_SCALE), x.max(x * MAX_SCALE) + strip);
    let (top, bottom) = (y.min(y * MAX_SCALE), y.max(y * MAX_SCALE) + PROBE_HEIGHT);
    [left, top, right, bottom].map(pixel)
}

#[cfg(any(windows, test))]
#[expect(
    clippy::cast_possible_truncation,
    reason = "saturating float-to-int cast; callers pass plausible rects"
)]
fn pixel(value: f32) -> i32 {
    value.round() as i32
}

#[cfg(test)]
mod tests;
