//! prm-ui's window services on top of prm-platform's desktop shell.

use std::sync::Arc;

use prm_platform::{DesktopShell, Rgb, TitleBarColors};
use prm_theme::{Rgba8, contrast::is_dark};
use prm_ui::{ChromeColors, UiHost};
use raw_window_handle::HasWindowHandle;

pub(crate) struct PlatformUiHost(pub Arc<dyn DesktopShell>);

const fn rgb([r, g, b]: [u8; 3]) -> Rgb {
    Rgb { r, g, b }
}

/// Dark captions get light system glyphs (the min/max/close buttons), by the theme's own
/// definition of a dark background.
pub(crate) fn title_bar(colors: ChromeColors) -> TitleBarColors {
    let [r, g, b] = colors.caption;
    TitleBarColors {
        dark: is_dark(Rgba8::rgb(r, g, b)),
        caption: Some(rgb(colors.caption)),
        text: Some(rgb(colors.text)),
        border: Some(rgb(colors.border)),
    }
}

impl UiHost for PlatformUiHost {
    fn apply_title_bar(&self, window: &dyn HasWindowHandle, colors: ChromeColors) {
        if let Err(error) = self.0.apply_title_bar(window, &title_bar(colors)) {
            tracing::debug!(%error, "title bar colors not applied");
        }
    }

    fn set_cloaked(&self, window: &dyn HasWindowHandle, cloaked: bool) {
        if let Err(error) = self.0.set_cloaked(window, cloaked) {
            tracing::warn!(%error, cloaked, "couldn't cloak the window");
        }
    }

    fn system_animations_enabled(&self) -> Option<bool> {
        self.0.system_animations_enabled()
    }
}

#[cfg(test)]
mod tests;
