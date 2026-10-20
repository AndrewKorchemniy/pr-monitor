//! `PrMonitorApp`: owns the UI state, drains service events in `logic` (which runs even while
//! hidden), draws in `ui`, and runs the actions widgets queued.

mod commands;
mod events;
mod overlays;
mod run;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

use eframe::egui;
use prm_config::{AppPaths, Config};
use prm_domain::{Timestamp, ports::Clock, uri::AppUri};
use prm_service::{AppState, Command};
use time::UtcOffset;

use crate::{
    actions::Actions,
    commands::{
        keymap::{Keymap, KeymapIssue},
        palette::PaletteState,
    },
    deps::{BuildInfo, ChromeColors, UiHost, UiInit},
    link::{ServiceLink, StateCheck},
    notices::NoticeStack,
    render::{
        AdapterFacts, RenderEnv, RendererKind, adapter_facts,
        hover_bands::HoverBands,
        policy::{RenderFacts, RenderPolicy},
        repaint::{RepaintInputs, next_repaint},
    },
    route::{Overlay, Screen},
    screens::{dashboard::DashboardUi, first_run::FirstRun, settings::SettingsState},
    theme::{ThemeRuntime, fonts, selection_from},
    tray::{TrayController, TrayMsg},
    ui_state::{PeekLayout, UiStateSync, collect},
    view::{DashboardView, changes::ChangeLog},
    window::WindowController,
};

/// A `FrameCx` over the app's fields (a macro so the other fields stay borrowable).
macro_rules! frame_cx {
    ($app:ident, $ctx:expr) => {
        crate::frame::FrameCx {
            theme: $app.theme.current($ctx).clone(),
            state: &$app.state,
            now: $app.clock.now(),
            offset: $app.offset,
            keymap: &$app.keymap,
            changes: &$app.changes,
            actions: &mut $app.actions,
        }
    };
}
use frame_cx;

pub(crate) struct PrMonitorApp {
    link: ServiceLink,
    state: Arc<AppState>,
    host: Box<dyn UiHost>,
    clock: Arc<dyn Clock>,
    offset: UtcOffset,
    paths: AppPaths,
    theme: ThemeRuntime,
    render: RenderEnv,
    adapter: Option<AdapterFacts>,
    keymap: Keymap,
    window: WindowController,
    tray: Option<(TrayController, mpsc::Receiver<TrayMsg>)>,
    screen: Screen,
    overlay: Overlay,
    dash: DashboardUi,
    settings: Option<SettingsState>,
    first_run: Option<FirstRun>,
    palette: PaletteState,
    pat_input: String,
    pat_error: Option<Arc<str>>,
    notices: NoticeStack,
    changes: ChangeLog,
    actions: Actions,
    ui_sync: UiStateSync,
    quit_requested: Arc<AtomicBool>,
    quitting: bool,
    hover_bands: Arc<HoverBands>,
    on_first_frame: Option<Box<dyn FnOnce()>>,
    activation: Option<AppUri>,
    build: BuildInfo,
    licenses: &'static str,
    service_stopped: bool,
    chrome: Option<ChromeColors>,
    applied_config: Option<Arc<Config>>,
}

/// Builds the app inside eframe's creator (the window exists; the first frame has not run).
pub fn create_app(cc: &eframe::CreationContext<'_>, init: UiInit) -> Box<dyn eframe::App> {
    fonts::install(&cc.egui_ctx);
    let mut link = ServiceLink::new(init.commands, init.state, init.events);
    let state = link.latest();
    let adapter = adapter_facts(cc);
    log_graphics(init.render, adapter.as_ref());
    if init.start_hidden {
        init.host.set_cloaked(cc, true);
    }
    let policy = resolve_policy(
        init.render,
        adapter.as_ref(),
        init.host.as_ref(),
        &state.config,
    );
    let theme = ThemeRuntime::new(&init.paths.themes, &state.config.ui, policy);
    let tray = create_tray(&cc.egui_ctx, &theme, &state);
    let window = WindowController::new(init.start_hidden, &state.config.ui);
    let row_pitch = theme.current(&cc.egui_ctx).tokens.metrics.row.row_height;
    let mut app = PrMonitorApp {
        link,
        host: init.host,
        offset: init.clock.local_offset(),
        clock: init.clock,
        paths: init.paths,
        theme,
        render: init.render,
        adapter,
        keymap: Keymap::defaults(),
        window,
        tray,
        screen: Screen::Dashboard,
        overlay: Overlay::None,
        dash: restore_dashboard(&state, row_pitch),
        settings: None,
        first_run: state.first_run.then(|| FirstRun::new(state.config.clone())),
        palette: PaletteState::default(),
        pat_input: String::new(),
        pat_error: None,
        notices: NoticeStack::default(),
        changes: ChangeLog::default(),
        actions: Actions::default(),
        ui_sync: UiStateSync::new((*state.ui_state).clone()),
        quit_requested: init.quit_requested,
        quitting: false,
        hover_bands: init.hover_bands,
        on_first_frame: Some(init.on_first_frame),
        activation: init.activation,
        build: init.build,
        licenses: init.licenses,
        service_stopped: false,
        chrome: None,
        applied_config: Some(state.config.clone()),
        state,
    };
    app.reload_keymap();
    cc.egui_ctx.set_zoom_factor(app.state.config.ui.font_scale);
    Box::new(app)
}

/// The dashboard as the user left it (filters, sort, selection, peek).
fn restore_dashboard(state: &AppState, row_pitch: f32) -> DashboardUi {
    let ui = &state.ui_state;
    let view = DashboardView::new(ui, state.rows.clone(), state.rows_generation, row_pitch);
    let peek = PeekLayout {
        open: ui.peek_open,
        width: ui.peek_width,
    };
    DashboardUi::new(view, peek)
}

/// One line a support request can quote: the GPU, the renderer and why it was picked.
fn log_graphics(render: RenderEnv, adapter: Option<&AdapterFacts>) {
    tracing::info!(
        adapter = ?adapter,
        renderer = render.renderer.as_arg(),
        fallback_from = render.fallback_from.map(RendererKind::as_arg),
        remote_session = render.remote_session,
        "graphics ready"
    );
}

fn resolve_policy(
    render: RenderEnv,
    adapter: Option<&AdapterFacts>,
    host: &dyn UiHost,
    config: &Config,
) -> RenderPolicy {
    let facts = RenderFacts {
        renderer: render.renderer,
        software_adapter: adapter.is_some_and(|a| a.software),
        remote_session: render.remote_session,
        os_animations: host.system_animations_enabled(),
    };
    RenderPolicy::resolve(config.ui.low_power, config.ui.animations, facts)
}

fn load_keymap(paths: &AppPaths) -> (Keymap, Vec<KeymapIssue>) {
    match std::fs::read_to_string(&paths.keymap) {
        Ok(text) => Keymap::defaults().with_overrides(&text),
        Err(_) => (Keymap::defaults(), Vec::new()),
    }
}

fn create_tray(
    ctx: &egui::Context,
    theme: &ThemeRuntime,
    state: &AppState,
) -> Option<(TrayController, mpsc::Receiver<TrayMsg>)> {
    match TrayController::new(ctx, &theme.mark_colors(ctx, state.tray_badge())) {
        Ok(tray) => Some(tray),
        Err(error) => {
            tracing::warn!(%error, "tray icon unavailable");
            None
        }
    }
}

impl eframe::App for PrMonitorApp {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let now = self.clock.now();
        if self.quit_requested.load(Ordering::Relaxed) {
            self.quit(ctx);
        }
        self.drain_events(ctx, now);
        self.poll_state(ctx);
        self.drain_tray(ctx, now);
        self.window
            .begin_frame(ctx, &self.link, now, self.tray.is_some());
        self.handle_close(ctx);
        if let Some(uri) = self.activation.take() {
            self.activate(ctx, uri);
        }
        if self.window.take_uncloak() {
            self.host.set_cloaked(frame, false);
        }
        if self.window.take_gained_focus() {
            self.theme.poll_user_dir();
            self.reload_keymap();
        }
        self.theme.apply(ctx);
        self.theme.install_frame(ctx);
        self.sync_tray(ctx);
        self.notices.prune(now);
        self.hover_bands.publish(
            self.dash.geometry,
            self.theme.policy().low_power && self.window.visible(),
        );
        let current = collect(
            &self.dash.view,
            &self.window,
            self.dash.peek,
            &self.state.ui_state,
        );
        self.ui_sync.tick(now, current, &self.link);
        self.schedule_repaint(ctx, now);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let chrome = self.theme.chrome(&ctx);
        if self.chrome != Some(chrome) {
            self.host.apply_title_bar(frame, chrome);
            self.chrome = Some(chrome);
        }
        self.dispatch_keys(&ctx);
        self.draw(ui);
        self.run_actions(&ctx);
        if let Some(callback) = self.on_first_frame.take() {
            callback();
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let current = collect(
            &self.dash.view,
            &self.window,
            self.dash.peek,
            &self.state.ui_state,
        );
        self.ui_sync.flush(current, &self.link);
        self.link.send(Command::Shutdown);
    }
}

impl PrMonitorApp {
    /// Re-reads `keymap.toml`; problems show as notices.
    fn reload_keymap(&mut self) {
        let (keymap, issues) = load_keymap(&self.paths);
        self.keymap = keymap;
        let now = self.clock.now();
        for issue in issues {
            self.notices.info(&issue.0, now);
        }
    }

    fn poll_state(&mut self, ctx: &egui::Context) {
        match self.link.check() {
            StateCheck::Unchanged => {}
            StateCheck::Changed => {
                let state = self.link.latest();
                self.on_state(ctx, state);
            }
            StateCheck::Stopped => self.on_service_stopped(),
        }
    }

    fn on_state(&mut self, ctx: &egui::Context, state: Arc<AppState>) {
        if state.rows_generation != self.dash.view.rows_generation() {
            self.dash
                .view
                .on_rows(state.rows.clone(), state.rows_generation);
        }
        let config_changed = self
            .applied_config
            .as_ref()
            .is_none_or(|c| !Arc::ptr_eq(c, &state.config));
        self.state = state;
        if config_changed {
            self.on_config(ctx);
        }
    }

    fn on_config(&mut self, ctx: &egui::Context) {
        let config = self.state.config.clone();
        let prefs = &config.ui;
        self.window.close_to_tray = prefs.close_to_tray;
        self.window.minimize_to_tray = prefs.minimize_to_tray;
        self.theme.set_selection(selection_from(prefs));
        let policy = resolve_policy(
            self.render,
            self.adapter.as_ref(),
            self.host.as_ref(),
            &config,
        );
        self.theme.set_policy(policy);
        ctx.set_zoom_factor(prefs.font_scale);
        self.dash.view.row_pitch = prm_theme::Density::metrics(match prefs.density {
            prm_config::schema::Density::Comfortable => prm_theme::Density::Comfortable,
            prm_config::schema::Density::Compact => prm_theme::Density::Compact,
        })
        .row_height;
        if let Some(settings) = &mut self.settings {
            settings.on_applied(config.clone());
        }
        self.applied_config = Some(config);
    }

    fn on_service_stopped(&mut self) {
        if !self.service_stopped {
            self.service_stopped = true;
            self.notices.push(
                &prm_service::Notice {
                    severity: prm_config::Severity::Error,
                    text: Arc::from(crate::copy::SERVICE_STOPPED),
                    org: None,
                    action: None,
                    ttl: std::time::Duration::from_hours(24),
                },
                self.clock.now(),
            );
        }
    }

    fn drain_tray(&mut self, ctx: &egui::Context, now: Timestamp) {
        let messages: Vec<TrayMsg> = match &self.tray {
            Some((_, rx)) => rx.try_iter().collect(),
            None => return,
        };
        for message in messages {
            match message {
                TrayMsg::Toggle => self.window.toggle(ctx, now),
                TrayMsg::Menu(key) => self.tray_menu(ctx, &key),
            }
        }
    }

    fn sync_tray(&mut self, ctx: &egui::Context) {
        if let Some((tray, _)) = &mut self.tray {
            let colors = self.theme.mark_colors(ctx, self.state.tray_badge());
            tray.sync(&self.state, &colors, self.theme.generation);
        }
    }

    /// Close goes to the tray when configured (and a tray exists); otherwise quits.
    fn handle_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.quitting {
            return;
        }
        if self.window.close_to_tray && self.tray.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide(ctx);
        } else {
            self.quitting = true;
        }
    }

    fn hide(&mut self, ctx: &egui::Context) {
        self.window.hide(ctx);
        let current = collect(
            &self.dash.view,
            &self.window,
            self.dash.peek,
            &self.state.ui_state,
        );
        self.ui_sync.flush(current, &self.link);
    }

    fn quit(&mut self, ctx: &egui::Context) {
        if !self.quitting {
            self.quitting = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn schedule_repaint(&self, ctx: &egui::Context, now: Timestamp) {
        let deadlines = [
            self.notices.next_expiry(),
            self.ui_sync.deadline(),
            self.state.notifications.paused_until,
        ];
        let input = RepaintInputs {
            visible: self.window.visible(),
            focused: self.window.focused(),
            low_power: self.theme.policy().low_power,
            now: Some(now),
            next_refresh: self.state.refresh.next_due,
            deadlines: deadlines.into_iter().flatten().collect(),
        };
        if let Some(after) = next_repaint(&input) {
            ctx.request_repaint_after(after);
        }
    }
}
