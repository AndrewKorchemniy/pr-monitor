//! The main window's visibility: start hidden, close and minimize to the tray, tray toggling,
//! presence reports (visible/focused) for the notification policy, and the placement to save.

use eframe::egui::{self, Pos2, Rect, ViewportCommand};
use prm_config::WindowRect;
use prm_domain::Timestamp;
use prm_service::Command;

use crate::link::ServiceLink;

/// A tray click within this long of the window losing focus means "hide" (the click itself
/// stole focus from the window).
const TOGGLE_FOCUS_WINDOW: std::time::Duration = std::time::Duration::from_millis(300);
/// Where a start-hidden window waits on platforms that cannot cloak (unzoomed points).
pub(crate) const OFFSCREEN: Pos2 = egui::pos2(-32_000.0, -32_000.0);
/// Where an off-screen window goes when the monitor size is unknown (unzoomed points).
const FALLBACK_POSITION: Pos2 = egui::pos2(100.0, 100.0);

/// eframe forces the first frame visible; flip once eframe honors `with_visible(false)`.
pub const EFRAME_HONORS_HIDDEN_ROOT: bool = false;

/// How the window starts (from `UiInit`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct WindowLaunch {
    pub start_hidden: bool,
    /// The window waits at `OFFSCREEN` instead of cloaked, and moves back on first show.
    pub hide_offscreen: bool,
    /// Where it moves back to (unzoomed points); `None` centers it.
    pub restore_position: Option<Pos2>,
}

/// Where a window parked off-screen goes on its first show.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Restore {
    Saved(Pos2),
    Centered,
}

pub(crate) struct WindowController {
    visible: bool,
    focused: bool,
    gained_focus: bool,
    focus_lost_at: Option<Timestamp>,
    hide_on_first_logic: bool,
    uncloak_pending: bool,
    launch: WindowLaunch,
    pending_restore: Option<Restore>,
    pub close_to_tray: bool,
    pub minimize_to_tray: bool,
    reported: Option<(bool, bool)>,
    /// Outer position and inner size, in unzoomed points (what `ViewportBuilder` restores).
    placement: Option<Rect>,
    maximized: bool,
}

impl WindowController {
    pub(crate) const fn new(launch: WindowLaunch, prefs: &prm_config::UiPrefs) -> Self {
        Self {
            visible: !launch.start_hidden,
            focused: !launch.start_hidden,
            gained_focus: false,
            focus_lost_at: None,
            hide_on_first_logic: launch.start_hidden,
            uncloak_pending: false,
            launch,
            pending_restore: None,
            close_to_tray: prefs.close_to_tray,
            minimize_to_tray: prefs.minimize_to_tray,
            reported: None,
            placement: None,
            maximized: false,
        }
    }

    pub(crate) const fn visible(&self) -> bool {
        self.visible
    }

    pub(crate) const fn focused(&self) -> bool {
        self.visible && self.focused
    }

    /// The last on-screen placement, as saved in the UI state and restored at launch.
    pub(crate) fn placement(&self) -> Option<WindowRect> {
        self.placement.map(|rect| WindowRect {
            x: rect.min.x,
            y: rect.min.y,
            w: rect.width(),
            h: rect.height(),
            maximized: self.maximized,
        })
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
            self.uncloak_pending = !self.launch.hide_offscreen;
            if self.launch.hide_offscreen {
                let restore = self.launch.restore_position;
                self.pending_restore = Some(restore.map_or(Restore::Centered, Restore::Saved));
            }
            ctx.request_repaint();
        }
        let (focused, minimized, maximized) = ctx.input(|i| {
            let v = i.viewport();
            (v.focused, v.minimized, v.maximized)
        });
        if let Some(focused) = focused {
            self.track_focus(focused, now);
        }
        if self.visible {
            self.track_placement(ctx);
            self.maximized = maximized.unwrap_or(false);
        }
        if self.minimize_to_tray && has_tray && self.visible && minimized == Some(true) {
            self.hide(ctx);
        }
        self.report_presence(link);
    }

    fn track_focus(&mut self, focused: bool, now: Timestamp) {
        if focused && !self.focused {
            self.gained_focus = true;
        }
        if !focused && self.focused {
            self.focus_lost_at = Some(now);
        }
        self.focused = focused;
    }

    fn track_placement(&mut self, ctx: &egui::Context) {
        let (outer, inner) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().inner_rect));
        if let (Some(outer), Some(inner)) = (outer, inner)
            && let Some(placement) = placement_of(outer, inner, ctx.zoom_factor())
        {
            self.placement = Some(placement);
        }
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
        if let Some(restore) = self.pending_restore.take() {
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(restore_position(
                ctx, restore,
            )));
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

/// Outer position plus inner size in unzoomed points (egui reports zoomed points; the window
/// is created at zoom 1 with an inner size). `None` while parked at `OFFSCREEN`.
fn placement_of(outer: Rect, inner: Rect, zoom: f32) -> Option<Rect> {
    let unzoom = |p: Pos2| (p.to_vec2() * zoom).to_pos2();
    let min = unzoom(outer.min);
    let parked = min.distance(OFFSCREEN) < 1.0;
    (!parked).then(|| Rect::from_min_size(min, inner.size() * zoom))
}

/// `OuterPosition` is in zoomed points.
fn restore_position(ctx: &egui::Context, restore: Restore) -> Pos2 {
    let zoom = ctx.zoom_factor().max(f32::EPSILON);
    let zoomed = |p: Pos2| (p.to_vec2() / zoom).to_pos2();
    match restore {
        Restore::Saved(position) => zoomed(position),
        Restore::Centered => {
            let (monitor, outer) =
                ctx.input(|i| (i.viewport().monitor_size, i.viewport().outer_rect));
            match (monitor, outer) {
                (Some(monitor), Some(outer)) => ((monitor - outer.size()) / 2.0)
                    .max(egui::Vec2::ZERO)
                    .to_pos2(),
                (None, _) | (_, None) => zoomed(FALLBACK_POSITION),
            }
        }
    }
}

#[cfg(test)]
mod tests;
