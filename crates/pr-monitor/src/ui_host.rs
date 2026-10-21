//! prm-ui's window services on top of prm-platform's desktop shell.

use std::sync::Arc;

use prm_domain::ports::PortErrorKind;
use prm_platform::{DesktopShell, Rgb, TitleBarColors};
use prm_ui::{ChromeColors, UiHost};
use raw_window_handle::HasWindowHandle;

pub(crate) struct PlatformUiHost(pub Arc<dyn DesktopShell>);

const fn rgb([r, g, b]: [u8; 3]) -> Rgb {
    Rgb { r, g, b }
}

/// Dark captions get light system glyphs (the min/max/close buttons).
pub(crate) const fn title_bar(colors: ChromeColors) -> TitleBarColors {
    let [r, g, b] = colors.caption;
    let luma = r as u32 * 299 + g as u32 * 587 + b as u32 * 114;
    TitleBarColors {
        dark: luma < 128_000,
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
        match self.0.set_cloaked(window, cloaked) {
            Ok(()) => {}
            // Only Windows cloaks; elsewhere the window simply shows as it is.
            Err(error) if error.kind == PortErrorKind::Unsupported => {
                tracing::debug!(%error, cloaked, "window cloaking unsupported");
            }
            Err(error) => tracing::warn!(%error, cloaked, "couldn't cloak the window"),
        }
    }

    fn system_animations_enabled(&self) -> Option<bool> {
        self.0.system_animations_enabled()
    }
}

#[cfg(test)]
mod tests;
