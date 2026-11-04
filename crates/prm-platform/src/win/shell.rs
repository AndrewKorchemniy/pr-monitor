//! Window chrome (DWM title-bar colors, cloaking) and desktop facts on Windows.

#![allow(
    unsafe_code,
    reason = "Win32 FFI; each block documents why it is sound"
)]

use prm_domain::ports::{PortError, PortErrorKind};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::{
    Win32::{
        Foundation::{E_INVALIDARG, HWND, RECT},
        Graphics::Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_CLOAK, DWMWA_COLOR_DEFAULT,
            DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
            DWMWINDOWATTRIBUTE, DwmSetWindowAttribute,
        },
        Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromRect},
        UI::WindowsAndMessaging::{
            SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
        },
    },
    core::BOOL,
};
use windows_registry::LOCAL_MACHINE;

use super::port_error;
use crate::{
    common::failed,
    shared::{
        colors::{Rgb, WindowChrome, colorref},
        os_desc::{OsFacts, format_os_description},
        session_kind::SessionInfo,
    },
    shell::{DesktopShell, corner_preference, title_bar_probe},
};

pub(crate) struct WinShell {
    os: String,
}

impl WinShell {
    pub(crate) fn new() -> Self {
        Self {
            os: os_description(),
        }
    }
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "RawWindowHandle is non_exhaustive; every non-Win32 handle is unsupported"
)]
fn hwnd(window: &dyn HasWindowHandle) -> Result<HWND, PortError> {
    let handle = window.window_handle().map_err(failed)?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Ok(HWND(win32.hwnd.get() as *mut core::ffi::c_void)),
        _ => Err(PortError::new(
            PortErrorKind::Unsupported,
            "not a Win32 window",
        )),
    }
}

fn set_attribute<T>(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: &T) -> Result<(), PortError> {
    let size = u32::try_from(size_of::<T>()).unwrap_or(0);
    // SAFETY: `hwnd` is a live top-level window of this process; the pointer and size describe
    // `value`, which outlives the call.
    let result =
        unsafe { DwmSetWindowAttribute(hwnd, attribute, (&raw const *value).cast(), size) };
    match result {
        Ok(()) => Ok(()),
        // Windows 10 does not support the caption/text/border colors or the corner preference:
        // not an error for us.
        Err(error) if error.code() == E_INVALIDARG => {
            tracing::debug!(attribute = attribute.0, "window attribute not supported");
            Ok(())
        }
        Err(error) => Err(port_error(&error)),
    }
}

fn color_or_default(color: Option<Rgb>) -> u32 {
    color.map_or(DWMWA_COLOR_DEFAULT, colorref)
}

impl DesktopShell for WinShell {
    fn apply_chrome(
        &self,
        window: &dyn HasWindowHandle,
        chrome: &WindowChrome,
    ) -> Result<(), PortError> {
        let hwnd = hwnd(window)?;
        // why: every attribute is tried, so one failing never leaves the corner shape unset.
        [
            set_attribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &BOOL::from(chrome.dark),
            ),
            set_attribute(hwnd, DWMWA_CAPTION_COLOR, &color_or_default(chrome.caption)),
            set_attribute(hwnd, DWMWA_TEXT_COLOR, &color_or_default(chrome.text)),
            set_attribute(hwnd, DWMWA_BORDER_COLOR, &color_or_default(chrome.border)),
            set_attribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner_preference(chrome.rounded),
            ),
        ]
        .into_iter()
        .collect()
    }

    fn set_cloaked(&self, window: &dyn HasWindowHandle, cloaked: bool) -> Result<(), PortError> {
        set_attribute(hwnd(window)?, DWMWA_CLOAK, &BOOL::from(cloaked))
    }

    fn session(&self) -> SessionInfo {
        super::session::session_info()
    }

    fn os_description(&self) -> &str {
        &self.os
    }

    fn system_animations_enabled(&self) -> Option<bool> {
        let mut enabled = BOOL::default();
        // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL through the pointer to a local.
        unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some((&raw mut enabled).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .ok()
        .map(|()| enabled.as_bool())
    }

    fn title_bar_visible(&self, position: [f32; 2], width: f32) -> Option<bool> {
        let [x, y] = position;
        let [left, top, right, bottom] = title_bar_probe(x, y, width);
        let probe = RECT {
            left,
            top,
            right,
            bottom,
        };
        // SAFETY: the pointer is to a local RECT that outlives the call; the API only reads it.
        let monitor = unsafe { MonitorFromRect(&raw const probe, MONITOR_DEFAULTTONULL) };
        Some(!monitor.is_invalid())
    }
}

fn os_description() -> String {
    let key = LOCAL_MACHINE.open(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let text = |name: &str| key.as_ref().ok().and_then(|k| k.get_string(name).ok());
    let product = text("ProductName");
    let display_version = text("DisplayVersion");
    let build = text("CurrentBuildNumber").and_then(|b| b.parse().ok());
    let ubr = key.as_ref().ok().and_then(|k| k.get_u32("UBR").ok());
    format_os_description(&OsFacts {
        product: product.as_deref(),
        display_version: display_version.as_deref(),
        build,
        ubr,
        arch: std::env::consts::ARCH,
    })
}
