//! Notifications: on/off, while in front, sound, the per-refresh budget, event toggles, pause
//! and a test notification.

use eframe::egui;
use prm_config::{Config, limits};
use prm_domain::notify::{EventToggles, NotificationPrefs, WhenFocused};
use prm_service::Command;

use crate::{
    actions::UiAction,
    commands::CommandId,
    copy,
    frame::FrameCx,
    theme::UiTheme,
    widgets::{
        self,
        card::section,
        controls::{self, setting_row, toggle_row},
    },
};

const WHEN_FOCUSED: [(WhenFocused, &str); 2] = [
    (WhenFocused::Suppress, copy::DONT_NOTIFY),
    (WhenFocused::Show, copy::NOTIFY_ANYWAY),
];

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
    let prefs = &mut draft.notifications;
    section(ui, &theme, copy::SECTION_DELIVERY, None, |ui| {
        delivery(ui, &theme, prefs);
    });
    ui.add_enabled_ui(prefs.enabled, |ui| {
        section(ui, &theme, copy::SECTION_EVENTS, None, |ui| {
            events(ui, &theme, &mut prefs.events);
        });
    });
    section(ui, &theme, copy::SECTION_TRY, None, |ui| try_out(ui, cx));
}

/// The event toggles in two columns.
fn events(ui: &mut egui::Ui, theme: &UiTheme, events: &mut EventToggles) {
    egui::Grid::new("notify_events")
        .num_columns(2)
        .spacing([24.0, 6.0])
        .show(ui, |ui| {
            for (index, (value, label)) in toggles(events).into_iter().enumerate() {
                controls::checkbox(ui, theme, value, label);
                if index % 2 == 1 {
                    ui.end_row();
                }
            }
        });
}

fn delivery(ui: &mut egui::Ui, theme: &UiTheme, prefs: &mut NotificationPrefs) {
    toggle_row(
        ui,
        theme,
        &mut prefs.enabled,
        (copy::SHOW_NOTIFICATIONS, None),
    );
    ui.add_enabled_ui(prefs.enabled, |ui| {
        ui.separator();
        setting_row(ui, theme, (copy::WHILE_IN_FRONT, None), |ui| {
            controls::segmented(ui, theme, &mut prefs.when_focused, &WHEN_FOCUSED);
        });
        ui.separator();
        toggle_row(ui, theme, &mut prefs.sound, (copy::PLAY_SOUND, None));
        ui.separator();
        toggle_row(
            ui,
            theme,
            &mut prefs.include_drafts,
            (copy::INCLUDE_DRAFTS, None),
        );
        ui.separator();
        let label = copy::max_per_refresh(prefs.max_per_refresh);
        setting_row(ui, theme, (&label, None), |ui| {
            let range = limits::MAX_PER_REFRESH;
            controls::slider(ui, theme, &mut prefs.max_per_refresh, range, &label);
        });
    });
}

/// A test notification, and pausing or resuming notifications for an hour.
fn try_out(ui: &mut egui::Ui, cx: &mut FrameCx<'_>) {
    let theme = cx.theme.clone();
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
