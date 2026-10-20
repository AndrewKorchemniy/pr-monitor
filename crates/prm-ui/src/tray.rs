//! The tray icon: the brand mark with a state-colored change bar, a menu whose ids are command
//! keys, left click to toggle the window. Updates are diff-only.

use std::sync::mpsc;

use eframe::egui;
use prm_domain::view::TrayBadge;
use prm_service::AppState;
use prm_theme::{MarkColors, Palette, Rgba8, render_mark};
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

use crate::{commands::CommandId, copy};

/// Tray icon size in pixels.
const TRAY_SIZE: u32 = 32;
/// Window icon size in pixels.
const WINDOW_ICON_SIZE: u32 = 64;

pub(crate) enum TrayMsg {
    Toggle,
    Menu(String),
}

/// The bar color for a badge: muted idle, highlighter attention, danger error, accent syncing.
pub(crate) const fn bar_color(palette: &Palette, badge: TrayBadge) -> Rgba8 {
    match badge {
        TrayBadge::Idle => palette.tray_idle,
        TrayBadge::Attention => palette.status_needs_review_solid,
        TrayBadge::Error => palette.feedback_danger_solid,
        TrayBadge::Syncing => palette.accent_base,
    }
}

/// The window icon (graphite, idle).
pub fn window_icon() -> egui::IconData {
    let theme = crate::theme::fallback();
    let palette = &theme.tokens.palette;
    let colors = MarkColors::from_palette(palette, palette.signature_revision);
    egui::IconData {
        rgba: render_mark(WINDOW_ICON_SIZE, &colors),
        width: WINDOW_ICON_SIZE,
        height: WINDOW_ICON_SIZE,
    }
}

pub(crate) struct TrayController {
    icon: TrayIcon,
    pause: MenuItem,
    sign_in: MenuItem,
    last_badge: Option<TrayBadge>,
    last_theme: u64,
    last_tooltip: String,
    paused: bool,
}

fn item(command: CommandId, text: &str) -> MenuItem {
    MenuItem::with_id(command.key(), text, true, None)
}

impl TrayController {
    pub(crate) fn new(
        ctx: &egui::Context,
        colors: &MarkColors,
    ) -> Result<(Self, mpsc::Receiver<TrayMsg>), String> {
        let pause = item(CommandId::NotificationsPause, copy::TRAY_PAUSE);
        let sign_in = item(CommandId::AuthMenu, copy::TRAY_SIGN_IN);
        sign_in.set_enabled(false);
        let menu = Menu::new();
        menu.append_items(&[
            &item(CommandId::WindowShow, copy::TRAY_OPEN),
            &PredefinedMenuItem::separator(),
            &item(CommandId::RefreshNow, copy::TRAY_REFRESH),
            &pause,
            &sign_in,
            &item(CommandId::SettingsOpen, copy::TRAY_SETTINGS),
            &PredefinedMenuItem::separator(),
            &item(CommandId::AppQuit, copy::TRAY_QUIT),
        ])
        .map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel();
        install_handlers(ctx, &tx);
        let icon = TrayIconBuilder::new()
            .with_id(prm_domain::app::APP_ID)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip(prm_domain::app::APP_DISPLAY_NAME)
            .with_icon(tray_icon(colors)?)
            .build()
            .map_err(|e| e.to_string())?;
        let controller = Self {
            icon,
            pause,
            sign_in,
            last_badge: None,
            last_theme: 0,
            last_tooltip: String::new(),
            paused: false,
        };
        Ok((controller, rx))
    }

    /// Updates only what changed.
    pub(crate) fn sync(&mut self, state: &AppState, colors: &MarkColors, theme_generation: u64) {
        let badge = state.tray_badge();
        if self.last_badge != Some(badge) || self.last_theme != theme_generation {
            if let Ok(icon) = tray_icon(colors) {
                let _ = self.icon.set_icon(Some(icon));
            }
            self.last_badge = Some(badge);
            self.last_theme = theme_generation;
        }
        let tooltip = state.tray_tooltip();
        if tooltip != self.last_tooltip {
            let _ = self.icon.set_tooltip(Some(&tooltip));
            self.last_tooltip = tooltip;
        }
        let paused = state.notifications.paused_until.is_some();
        if paused != self.paused {
            self.pause.set_text(if paused {
                copy::TRAY_RESUME
            } else {
                copy::TRAY_PAUSE
            });
            self.paused = paused;
        }
        let needs_sign_in = state.orgs.iter().any(|o| {
            o.enabled
                && matches!(
                    o.auth.state,
                    prm_service::AuthStateView::NeedsSignIn { .. }
                        | prm_service::AuthStateView::Blocked { .. }
                        | prm_service::AuthStateView::SignedOut
                )
        });
        self.sign_in.set_enabled(needs_sign_in);
    }

    /// The pause item runs "resume" while paused.
    pub(crate) const fn paused(&self) -> bool {
        self.paused
    }
}

fn tray_icon(colors: &MarkColors) -> Result<Icon, String> {
    Icon::from_rgba(render_mark(TRAY_SIZE, colors), TRAY_SIZE, TRAY_SIZE).map_err(|e| e.to_string())
}

/// Tray events arrive on the OS thread: forward them and wake `logic` (even while hidden).
fn install_handlers(ctx: &egui::Context, tx: &mpsc::Sender<TrayMsg>) {
    {
        let (tx, ctx) = (tx.clone(), ctx.clone());
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            // Windows reports both press and release: act on the left release only.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = tx.send(TrayMsg::Toggle);
                ctx.request_repaint();
            }
        }));
    }
    let (tx, ctx) = (tx.clone(), ctx.clone());
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = tx.send(TrayMsg::Menu(event.id.0));
        ctx.request_repaint();
    }));
}

#[cfg(test)]
mod tests;
