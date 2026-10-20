//! Notifications: on/off, while in front, sound, the per-refresh budget, event toggles, pause
//! and a test notification.

use eframe::egui;
use prm_config::{Config, limits};
use prm_domain::notify::{EventToggles, WhenFocused};
use prm_service::Command;

use crate::{actions::UiAction, commands::CommandId, copy, frame::FrameCx, widgets};

/// Every toggle with its label, in display order.
fn toggles(events: &mut EventToggles) -> [(&mut bool, &'static str); 12] {
    [
        (&mut events.new_pr, "New pull requests"),
        (
            &mut events.ready_for_review,
            "Drafts marked ready for review",
        ),
        (
            &mut events.needs_re_review,
            "Pull requests that need review again",
        ),
        (&mut events.reviewed, "Pull requests reviewed"),
        (
            &mut events.changes_requested,
            "Changes requested or rejected",
        ),
        (&mut events.new_vote, "New approvals"),
        (&mut events.new_comments, "New comments"),
        (&mut events.comments_resolved, "All comments resolved"),
        (&mut events.reactivated, "Pull requests reopened"),
        (&mut events.closed, "Pull requests completed or abandoned"),
        (&mut events.new_push, "New changes pushed"),
        (&mut events.auth_problem, "Sign-in problems"),
    ]
}

pub(crate) fn page(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, draft: &mut Config) {
    let theme = cx.theme.clone();
    let space = theme.tokens.metrics.space.lg;
    let prefs = &mut draft.notifications;
    ui.checkbox(&mut prefs.enabled, copy::SHOW_NOTIFICATIONS);
    ui.add_enabled_ui(prefs.enabled, |ui| {
        widgets::field_label(ui, &theme, copy::WHILE_IN_FRONT);
        ui.radio_value(
            &mut prefs.when_focused,
            WhenFocused::Suppress,
            copy::DONT_NOTIFY,
        );
        ui.radio_value(
            &mut prefs.when_focused,
            WhenFocused::Show,
            copy::NOTIFY_ANYWAY,
        );
        ui.checkbox(&mut prefs.sound, copy::PLAY_SOUND);
        ui.checkbox(&mut prefs.include_drafts, copy::INCLUDE_DRAFTS);
        let n = prefs.max_per_refresh;
        ui.add(
            egui::Slider::new(&mut prefs.max_per_refresh, limits::MAX_PER_REFRESH)
                .text(copy::max_per_refresh(n)),
        );
        ui.add_space(space);
        widgets::field_label(ui, &theme, copy::NOTIFY_ABOUT);
        for (value, label) in toggles(&mut prefs.events) {
            ui.checkbox(value, label);
        }
    });
    ui.add_space(space);
    ui.horizontal(|ui| {
        if ui.button(copy::SEND_TEST).clicked() {
            cx.push(UiAction::Send(Command::TestNotification));
        }
        let paused = cx.state.notifications.paused_until.is_some();
        let (label, command) = if paused {
            (copy::RESUME, CommandId::NotificationsResume)
        } else {
            (copy::PAUSE_HOUR, CommandId::NotificationsPause)
        };
        if ui.button(label).clicked() {
            cx.run(command);
        }
    });
    if let Some(until) = cx.state.notifications.paused_until {
        widgets::muted(
            ui,
            &theme,
            &copy::paused_until(&crate::format::clock(until, cx.offset)),
        );
    }
}
