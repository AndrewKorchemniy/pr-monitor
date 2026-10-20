//! The main window's visibility: start hidden, close and minimize to the tray, tray toggling,
//! and presence reports (visible/focused) for the notification policy.

use eframe::egui::{self, Pos2, ViewportCommand};
use prm_domain::Timestamp;
use prm_service::Command;

use crate::link::ServiceLink;

/// A tray click within this long of the window losing focus means "hide" (the click itself
/// stole focus from the window).
const TOGGLE_FOCUS_WINDOW: std::time::Duration = std::time::Duration::from_millis(300);
/// Where a start-hidden window waits on platforms that cannot cloak.
const OFFSCREEN: Pos2 = egui::pos2(-32_000.0, -32_000.0);

/// eframe forces the first frame visible; flip once eframe honors `with_visible(false)`.
pub const EFRAME_HONORS_HIDDEN_ROOT: bool = false;

pub(crate) struct WindowController {
    visible: bool,
    focused: bool,
    gained_focus: bool,
    focus_lost_at: Option<Timestamp>,
    hide_on_first_logic: bool,
    uncloak_pending: bool,
    /// Platforms that cannot cloak start the window off-screen and move it back on show.
    pub hide_offscreen: bool,
    pending_restore: Option<Pos2>,
    pub close_to_tray: bool,
    pub minimize_to_tray: bool,
    reported: Option<(bool, bool)>,
    pub outer_rect: Option<egui::Rect>,
    pub maximized: bool,
}

impl WindowController {
    pub(crate) const fn new(start_hidden: bool, prefs: &prm_config::UiPrefs) -> Self {
        Self {
            visible: !start_hidden,
            focused: !start_hidden,
            gained_focus: false,
            focus_lost_at: None,
            hide_on_first_logic: start_hidden,
            uncloak_pending: false,
            hide_offscreen: false,
            pending_restore: None,
            close_to_tray: prefs.close_to_tray,
            minimize_to_tray: prefs.minimize_to_tray,
            reported: None,
            outer_rect: None,
            maximized: false,
        }
    }

    pub(crate) const fn visible(&self) -> bool {
        self.visible
    }

    pub(crate) const fn focused(&self) -> bool {
        self.visible && self.focused
    }

    /// True once per focus gain (re-scan themes and keymap).
    pub(crate) fn take_gained_focus(&mut self) -> bool {
        std::mem::take(&mut self.gained_focus)
    }

    /// The creator cloaked the window; uncloak once it is hidden.
    pub(crate) fn take_uncloak(&mut self) -> bool {
        std::mem::take(&mut self.uncloak_pending)
    }

    pub(crate) fn begin_frame(
        &mut self,
        ctx: &egui::Context,
        link: &ServiceLink,
        now: Timestamp,
        has_tray: bool,
    ) {
        if self.hide_on_first_logic {
            self.hide_on_first_logic = false;
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            self.visible = false;
            self.uncloak_pending = !self.hide_offscreen;
            if self.hide_offscreen {
                self.pending_restore = Some(egui::pos2(100.0, 100.0));
            }
            ctx.request_repaint();
        }
        let (focused, minimized, outer, maximized) = ctx.input(|i| {
            let v = i.viewport();
            (v.focused, v.minimized, v.outer_rect, v.maximized)
        });
        if let Some(focused) = focused {
            if focused && !self.focused {
                self.gained_focus = true;
            }
            if !focused && self.focused {
                self.focus_lost_at = Some(now);
            }
            self.focused = focused;
        }
        if self.visible {
            if let Some(rect) = outer.filter(|r| r.min != OFFSCREEN) {
                self.outer_rect = Some(rect);
            }
            self.maximized = maximized.unwrap_or(false);
        }
        if self.minimize_to_tray && has_tray && self.visible && minimized == Some(true) {
            self.hide(ctx);
        }
        self.report_presence(link);
    }

    fn report_presence(&mut self, link: &ServiceLink) {
        let presence = (self.visible, self.focused());
        if self.reported != Some(presence) {
            self.reported = Some(presence);
            link.send(Command::UiPresence {
                visible: presence.0,
                focused: presence.1,
            });
        }
    }

    pub(crate) fn show(&mut self, ctx: &egui::Context) {
        if let Some(position) = self.pending_restore.take() {
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(position));
        }
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        self.visible = true;
        ctx.request_repaint();
    }

    pub(crate) fn hide(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        self.visible = false;
    }

    /// Tray left click: hide when the window was just in front, else bring it forward.
    pub(crate) fn toggle(&mut self, ctx: &egui::Context, now: Timestamp) {
        let just_focused = self.focused
            || self
                .focus_lost_at
                .is_some_and(|at| now >= at && now.since(at) < TOGGLE_FOCUS_WINDOW);
        if self.visible && just_focused {
            self.hide(ctx);
        } else {
            self.show(ctx);
        }
    }
}
