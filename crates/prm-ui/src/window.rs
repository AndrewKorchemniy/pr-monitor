//! The main window's visibility: start hidden, close and minimize to the tray, tray toggling,
//! and presence reports (visible/focused) for the notification policy.

mod reveal;

use eframe::egui::{self, Pos2, ViewportCommand};
use prm_domain::Timestamp;
use prm_service::Command;

use self::reveal::Reveal;
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
    /// A show waiting for `take_show`: `Some(true)` when the window was hidden.
    show_request: Option<bool>,
    reveal: Reveal,
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
            show_request: None,
            reveal: if start_hidden {
                Reveal::Idle
            } else {
                Reveal::Launching
            },
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

    /// True once a shown window has had a themed frame presented: uncloak it now.
    pub(crate) fn take_uncloak(&mut self) -> bool {
        std::mem::take(&mut self.uncloak_pending)
    }

    /// A ui pass ran; once it is presented the window can be uncloaked.
    pub(crate) fn painted(&mut self, ctx: &egui::Context) {
        self.reveal.painted();
        if self.reveal.needs_frame() {
            ctx.request_repaint();
        }
    }

    /// Sends the show asked for since the last `logic`. True means cloak the window first: it
    /// was hidden, and it stays cloaked until [`Self::take_uncloak`]. Call from `logic`, after
    /// `begin_frame`; the commands run after `logic` returns, so the cloak lands before them.
    pub(crate) fn take_show(&mut self, ctx: &egui::Context) -> bool {
        let Some(from_hidden) = self.show_request.take() else {
            return false;
        };
        if let Some(position) = self.pending_restore.take() {
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(position));
        }
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        let cloak = from_hidden && !self.hide_offscreen;
        if cloak {
            self.reveal = Reveal::Showing;
        }
        ctx.request_repaint();
        cloak
    }

    pub(crate) fn begin_frame(
        &mut self,
        ctx: &egui::Context,
        link: &ServiceLink,
        now: Timestamp,
        has_tray: bool,
    ) {
        self.uncloak_pending = self.reveal.begin_logic();
        if self.reveal.needs_frame() {
            ctx.request_repaint();
        }
        if self.hide_on_first_logic {
            // why: the window stays cloaked while hidden, so its next show cannot flash.
            self.hide_on_first_logic = false;
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            self.visible = false;
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
        if self.visible && minimized == Some(true) {
            self.uncloak_pending |= self.reveal.minimized();
            if self.minimize_to_tray && has_tray {
                self.hide(ctx);
            }
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

    /// Asks for the window in front; `take_show` sends it from the next `logic`, which may be
    /// this frame's, so the cloak can land before the window is shown.
    pub(crate) fn show(&mut self, ctx: &egui::Context) {
        let from_hidden = self.show_request.unwrap_or(false) || !self.visible;
        self.show_request = Some(from_hidden);
        self.visible = true;
        ctx.request_repaint();
    }

    /// Hides the window; one still being revealed stays cloaked, and the next show reveals it.
    pub(crate) fn hide(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        self.visible = false;
        self.show_request = None;
        self.reveal = Reveal::Idle;
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
