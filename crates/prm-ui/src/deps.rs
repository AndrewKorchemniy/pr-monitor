//! What the bin hands the UI: the service channels, the OS host, and launch facts. prm-ui never
//! depends on prm-platform; the bin implements `UiHost` on top of it.

use std::sync::{Arc, atomic::AtomicBool, mpsc};

use prm_config::AppPaths;
use prm_domain::{ports::Clock, uri::AppUri};
use prm_service::{AppEvent, AppState, CommandSender};

use crate::render::{RenderEnv, hover_bands::HoverBands};

/// Title-bar colors (DWM caption, text and border; the dark flag is egui's job).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ChromeColors {
    pub caption: [u8; 3],
    pub text: [u8; 3],
    pub border: [u8; 3],
}

/// OS services the window needs (implemented by the bin over prm-platform).
pub trait UiHost: 'static {
    fn apply_title_bar(
        &self,
        window: &dyn raw_window_handle::HasWindowHandle,
        colors: ChromeColors,
    );
    fn set_cloaked(&self, window: &dyn raw_window_handle::HasWindowHandle, cloaked: bool);
    fn system_animations_enabled(&self) -> Option<bool>;
}

/// Tests and platforms without window chrome.
#[derive(Copy, Clone, Debug, Default)]
pub struct NoopHost;

impl UiHost for NoopHost {
    fn apply_title_bar(
        &self,
        _window: &dyn raw_window_handle::HasWindowHandle,
        _colors: ChromeColors,
    ) {
    }

    fn set_cloaked(&self, _window: &dyn raw_window_handle::HasWindowHandle, _cloaked: bool) {}

    fn system_animations_enabled(&self) -> Option<bool> {
        None
    }
}

/// Build facts for About.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BuildInfo {
    pub version: &'static str,
    pub git_sha: &'static str,
    pub target: &'static str,
}

pub struct UiInit {
    pub commands: CommandSender,
    pub state: tokio::sync::watch::Receiver<Arc<AppState>>,
    pub events: mpsc::Receiver<AppEvent>,
    pub host: Box<dyn UiHost>,
    pub clock: Arc<dyn Clock>,
    pub paths: AppPaths,
    pub render: RenderEnv,
    pub start_hidden: bool,
    /// `--activate` on first launch, handled in the first `logic`.
    pub activation: Option<AppUri>,
    /// Set by the handoff Quit request or `--unregister`; polled every `logic`.
    pub quit_requested: Arc<AtomicBool>,
    pub hover_bands: Arc<HoverBands>,
    /// Called once, when the first frame has been painted (marks the renderer good).
    pub on_first_frame: Box<dyn FnOnce()>,
    pub build: BuildInfo,
    /// Third-party license texts (plain text).
    pub licenses: &'static str,
}
