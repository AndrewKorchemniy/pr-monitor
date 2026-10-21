//! An empty people list tracks everyone: the empty dashboard, the settings People section and
//! first run's people step say so.

use std::sync::Arc;

use eframe::egui::Role;
use egui_kittest::kittest::Queryable as _;
use prm_domain::auth::SettingsSection;
use prm_service::{AppState, Command};

use crate::{
    actions::UiAction,
    copy,
    screens::{first_run::Step, settings::Page},
    testing::{
        demo::{everyone_state, quiet_state},
        screens::{Fixture, Shown, harness},
    },
};

/// Nothing open, and no org lists anyone or includes the signed-in user.
fn nobody_listed_and_quiet() -> AppState {
    let mut state = quiet_state();
    let mut config = (*state.config).clone();
    for org in &mut config.orgs {
        org.people.tracked_authors.clear();
        org.include_me = false;
    }
    state.config = Arc::new(config);
    state
}

/// With no people listed, the empty dashboard used to say "You're not tracking anyone yet",
/// though everyone in the chosen repositories is tracked.
#[test]
fn an_empty_dashboard_with_nobody_listed_says_nothing_is_open() {
    let mut h = harness(Fixture::new(Shown::Dashboard, nobody_listed_and_quiet()));
    h.run();
    h.get_by_label(copy::EMPTY_NO_PRS);
    h.get_by_role_and_label(Role::Button, copy::EMPTY_EDIT_PEOPLE)
        .click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::OpenSettings(SettingsSection::People))),
        "{actions:?}"
    );
}

#[test]
fn settings_people_says_an_empty_list_tracks_everyone() {
    let mut fixture = Fixture::new(Shown::Settings, everyone_state());
    fixture.settings.page = Page::Organizations;
    fixture.settings.scroll_to = Some(SettingsSection::People);
    let mut h = harness(fixture);
    h.run();
    h.get_by_label(copy::TRACK_FROM_HELP);
    h.get_by_label(copy::EVERYONE_BUT_ME);

    h.get_by_role_and_label(Role::CheckBox, copy::INCLUDE_ME)
        .click();
    h.run();

    h.get_by_label(copy::EVERYONE);
    assert!(h.query_by_label(copy::EVERYONE_BUT_ME).is_none());
}

/// Empty lists are a complete choice, so the people step offers only "Start monitoring".
#[test]
fn first_run_people_step_explains_an_empty_list_and_starts_monitoring() {
    let state = everyone_state();
    let mut fixture = Fixture::new(Shown::FirstRun, state.clone());
    fixture.first_run.step = Step::People;
    fixture.first_run.settings.draft.orgs = vec![state.config.orgs[0].clone()];
    let mut h = harness(fixture);
    h.run();
    h.get_by_label(copy::TRACK_FROM_HELP);
    assert!(h.query_by_label("Skip for now").is_none());

    h.get_by_label(copy::START_MONITORING).click();
    h.run();

    let actions = h.state_mut().take_actions();
    let applied = actions.iter().find_map(|a| {
        if let UiAction::Send(Command::ApplyConfig(config)) = a {
            Some(config)
        } else {
            None
        }
    });
    let tracked = applied.map(|config| config.orgs[0].people.tracked_authors.len());
    assert_eq!(tracked, Some(0), "{actions:?}");
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::Send(Command::RefreshNow))),
        "{actions:?}"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::FinishFirstRun)),
        "{actions:?}"
    );
}
