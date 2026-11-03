//! The update line of About, in the sidebar's foot and in the folded layout: the status shows,
//! its actions take a click, and the setting is in General.

use eframe::egui;
use egui_kittest::{Harness, kittest::Queryable as _};
use prm_domain::{
    auth::SettingsSection,
    update::{DisabledReason, UpdateStage, UpdateStatus},
};
use prm_service::{AppState, Command};

use super::{SIZES, shown_at};
use crate::{
    actions::UiAction,
    copy,
    screens::settings::SettingsState,
    testing::{
        demo::demo_state,
        overflow::assert_clickable,
        screens::{Fixture, Shown, WINDOW},
    },
};

/// A narrow window (About ends General) and a wide one (About is the sidebar's foot).
fn sizes() -> [egui::Vec2; 3] {
    [SIZES[0], SIZES[1], WINDOW]
}

fn about(status: UpdateStatus, size: egui::Vec2) -> Harness<'static, Fixture> {
    let state = AppState {
        update: status,
        ..demo_state()
    };
    let setup = |f: &mut Fixture| {
        f.settings = SettingsState::new(f.state.config.clone(), SettingsSection::About);
    };
    shown_at(Shown::Settings, state, setup, size)
}

#[test]
fn the_status_is_shown_with_its_actions() {
    let cases = [
        (
            UpdateStatus::UpToDate,
            copy::UPDATE_UP_TO_DATE,
            copy::CHECK_NOW,
        ),
        (
            UpdateStatus::Ready("0.3.0".into()),
            "Version 0.3.0 is ready",
            copy::RESTART_TO_UPDATE,
        ),
        (
            UpdateStatus::Failed(UpdateStage::Download),
            copy::UPDATE_DOWNLOAD_FAILED,
            copy::RELEASES_PAGE,
        ),
    ];
    for size in sizes() {
        for (status, text, action) in &cases {
            let h = about(status.clone(), size);
            h.get_by_label(text).scroll_to_me();
            let node = h.get_by_label(action);
            assert_clickable(&h, &node, size);
        }
    }
}

#[test]
fn check_now_and_restart_send_their_commands() {
    let cases = [
        (UpdateStatus::UpToDate, copy::CHECK_NOW, "CheckForUpdates"),
        (
            UpdateStatus::Ready("0.3.0".into()),
            copy::RESTART_TO_UPDATE,
            "ApplyUpdate",
        ),
    ];
    for size in sizes() {
        for (status, label, command) in &cases {
            let mut h = about(status.clone(), size);
            h.get_by_label(label).click();
            h.run();
            let sent = h.state_mut().take_actions();
            assert!(
                sent.iter().any(|a| matches!(
                    a,
                    UiAction::Send(c) if format!("{c:?}") == *command
                )),
                "{size:?} {label}: {sent:?}"
            );
        }
    }
}

#[test]
fn releases_page_opens_the_repository_releases() {
    let mut h = about(UpdateStatus::Failed(UpdateStage::Check), SIZES[0]);
    h.get_by_label(copy::RELEASES_PAGE).scroll_to_me();
    h.get_by_label(copy::RELEASES_PAGE).click();
    h.run();

    let opened = h.state_mut().take_actions().into_iter().find_map(|a| {
        if let UiAction::Send(Command::OpenUrl(url)) = a {
            Some(url)
        } else {
            None
        }
    });
    assert_eq!(
        opened.map(String::from).as_deref(),
        Some("https://github.com/AndrewKorchemniy/pr-monitor/releases")
    );
}

#[test]
fn an_app_that_cannot_update_says_why_and_hides_the_setting() {
    let reasons = [
        (DisabledReason::NotInstalled, copy::UPDATE_NOT_INSTALLED),
        (DisabledReason::IsolatedHome, copy::UPDATE_ISOLATED_HOME),
    ];
    for (reason, text) in reasons {
        let h = about(UpdateStatus::Disabled(reason), WINDOW);
        h.get_by_label(text);
        assert!(h.query_by_label(copy::AUTO_UPDATE).is_none(), "{reason:?}");
    }
    let h = about(UpdateStatus::Idle, WINDOW);
    assert!(h.query_by_label(copy::AUTO_UPDATE).is_some());
}
