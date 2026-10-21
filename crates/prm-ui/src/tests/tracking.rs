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
    screens::{
        first_run::Step,
        settings::{Editor, Page},
    },
    testing::{
        demo::{everyone_state, quiet_state},
        screens::{Fixture, Shown, harness},
    },
};

/// Nothing open, and no org lists anyone; `include_me` for every org.
fn nobody_listed_and_quiet(include_me: bool) -> AppState {
    let mut state = quiet_state();
    let mut config = (*state.config).clone();
    for org in &mut config.orgs {
        org.people.tracked_authors.clear();
        org.include_me = include_me;
    }
    state.config = Arc::new(config);
    state
}

/// Clicks the empty dashboard's one button and returns the settings section it opens.
fn empty_state_opens(state: AppState, button: &str) -> Option<SettingsSection> {
    let mut h = harness(Fixture::new(Shown::Dashboard, state));
    h.run();
    h.get_by_label(button).click();
    h.run();
    let actions = h.state_mut().take_actions();
    actions.iter().find_map(|a| {
        if let UiAction::OpenSettings(section) = a {
            Some(*section)
        } else {
            None
        }
    })
}

/// With no people listed, the empty dashboard used to say "You're not tracking anyone yet",
/// though everyone in the chosen repositories is tracked.
#[test]
fn an_empty_dashboard_with_nobody_listed_says_nothing_is_open() {
    let mut h = harness(Fixture::new(
        Shown::Dashboard,
        nobody_listed_and_quiet(false),
    ));
    h.run();
    h.get_by_label(copy::EMPTY_NO_PRS);
    h.get_by_role_and_label(Role::Button, copy::EMPTY_EDIT_PEOPLE);
    let opened = empty_state_opens(nobody_listed_and_quiet(false), copy::EMPTY_EDIT_PEOPLE);
    assert_eq!(opened, Some(SettingsSection::People));
}

/// Tracking everyone, the user included, the empty dashboard used to say "Nobody you track"
/// and offer to edit the people, which could only narrow the list.
#[test]
fn an_empty_dashboard_tracking_everyone_offers_the_repositories() {
    let mut h = harness(Fixture::new(
        Shown::Dashboard,
        nobody_listed_and_quiet(true),
    ));
    h.run();
    h.get_by_label(&copy::no_prs_anywhere_body(&copy::organizations(2)));
    assert!(h.query_by_label(copy::EMPTY_EDIT_PEOPLE).is_none());
    let opened = empty_state_opens(nobody_listed_and_quiet(true), copy::EMPTY_CHOOSE_REPOS);
    assert_eq!(opened, Some(SettingsSection::Scope));
}

/// With every organization's watching off, the empty dashboard used to offer to edit the
/// tracked people of "0 organizations".
#[test]
fn an_empty_dashboard_watching_nothing_opens_the_organizations() {
    let mut state = quiet_state();
    let mut config = (*state.config).clone();
    for org in &mut config.orgs {
        org.enabled = false;
    }
    state.config = Arc::new(config);
    let mut orgs = state.orgs.to_vec();
    for org in &mut orgs {
        org.enabled = false;
    }
    state.orgs = orgs.into();
    let mut h = harness(Fixture::new(Shown::Dashboard, state.clone()));
    h.run();
    h.get_by_label(copy::EMPTY_NOTHING_WATCHED);
    let opened = empty_state_opens(state, copy::EMPTY_OPEN_ORGS);
    assert_eq!(opened, Some(SettingsSection::Organizations));
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

/// The edit dialog covers the section's help, though emptying the list there is what makes
/// it track everyone.
#[test]
fn the_tracked_people_dialog_says_an_empty_list_tracks_everyone() {
    let help_shown = |editing: Option<Editor>| {
        let mut fixture = Fixture::new(Shown::Settings, everyone_state());
        fixture.settings.page = Page::Organizations;
        fixture.settings.editing = editing;
        let mut h = harness(fixture);
        h.run();
        h.query_all_by_label(copy::TRACK_FROM_HELP).count()
    };
    assert_eq!(help_shown(Some(Editor::Tracked)), help_shown(None) + 1);
}

/// Empty lists are a complete choice, so the people step offers only "Start monitoring".
#[test]
fn first_run_people_step_explains_an_empty_list_and_starts_monitoring() {
    let state = everyone_state();
    let mut fixture = Fixture::new(Shown::FirstRun, state.clone());
    fixture.first_run.step = Step::People;
    fixture.first_run.settings.draft.orgs = vec![state.config.orgs[0].clone()];
    // why: "Skip for now" used to show only while both lists were empty.
    fixture.first_run.settings.draft.orgs[0]
        .people
        .reviewers
        .clear();
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
