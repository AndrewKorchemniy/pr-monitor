//! Opening the window: native options from prm-ui, then `create_app` inside eframe's creator.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

use prm_config::{AppPaths, WindowRect};
use prm_domain::ports::Clock;
use prm_platform::DesktopShell;
use prm_service::{AppEvent, ServiceHandle};
use prm_ui::{HoverBands, LaunchSpec, RenderEnv, UiInit};

use crate::{
    activation::UiWaker,
    assets, build_info,
    renderer::{MarkerFile, RendererChoice},
    ui_host::PlatformUiHost,
};

/// Coordinates beyond this are left over from a disconnected monitor.
const MAX_COORDINATE: f32 = 16_384.0;

/// Everything `run` needs from startup.
pub(crate) struct LaunchCtx<'a> {
    pub choice: RendererChoice,
    pub fallback_from: Option<prm_ui::RendererKind>,
    pub start_hidden: bool,
    pub remote_session: bool,
    pub window: Option<WindowRect>,
    pub paths: &'a AppPaths,
    pub shell: Arc<dyn DesktopShell>,
    pub clock: Arc<dyn Clock>,
    pub handle: &'a ServiceHandle,
    pub events: mpsc::Receiver<AppEvent>,
    pub waker: Arc<UiWaker>,
    pub quit: Arc<AtomicBool>,
    pub marker: Arc<MarkerFile>,
    pub init_phase: Arc<AtomicBool>,
}

/// A saved window rect worth restoring (sane size, not parked far off-screen).
pub(crate) fn rect_plausible(rect: &WindowRect) -> bool {
    let coords_ok = rect.x.abs() < MAX_COORDINATE && rect.y.abs() < MAX_COORDINATE;
    let size_ok =
        rect.w >= 200.0 && rect.h >= 150.0 && rect.w < MAX_COORDINATE && rect.h < MAX_COORDINATE;
    coords_ok && size_ok
}

pub(crate) fn run(ctx: LaunchCtx<'_>) -> Result<(), eframe::Error> {
    let spec = LaunchSpec {
        renderer: ctx.choice.kind,
        start_hidden: ctx.start_hidden,
        hide_offscreen: !cfg!(windows),
        window: ctx.window,
        rect_on_screen: ctx.window.as_ref().is_some_and(rect_plausible),
        icon: Arc::new(prm_ui::window_icon()),
    };
    let marker = Arc::clone(&ctx.marker);
    let init_phase = Arc::clone(&ctx.init_phase);
    let init = UiInit {
        commands: ctx.handle.sender(),
        state: ctx.handle.state(),
        events: ctx.events,
        host: Box::new(PlatformUiHost(ctx.shell)),
        clock: ctx.clock,
        paths: ctx.paths.clone(),
        render: RenderEnv {
            renderer: ctx.choice.kind,
            remote_session: ctx.remote_session,
            fallback_from: ctx.fallback_from,
        },
        start_hidden: ctx.start_hidden,
        activation: None,
        quit_requested: ctx.quit,
        hover_bands: Arc::new(HoverBands::default()),
        on_first_frame: Box::new(move || {
            marker.mark_ok();
            init_phase.store(false, Ordering::Relaxed);
        }),
        build: build_info::BUILD,
        licenses: assets::THIRD_PARTY_LICENSES,
    };
    let waker = ctx.waker;
    eframe::run_native(
        prm_domain::app::APP_DISPLAY_NAME,
        prm_ui::native_options(&spec),
        Box::new(move |cc| {
            waker.set(cc.egui_ctx.clone());
            cc.egui_ctx.request_repaint();
            Ok(prm_ui::create_app(cc, init))
        }),
    )
}

#[cfg(test)]
mod tests;
