//! Screens rendered headlessly with `egui_kittest`: what a user sees and what clicking does.
//! Accessibility checks are on and a missing glyph fails the test.

use std::sync::Arc;

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use prm_domain::view::StatusFilter;
use prm_service::{AppState, AuthStateView, Command, UiError};

use crate::{
    actions::UiAction,
    commands::{CommandId, palette::PaletteItem},
    copy,
    screens::settings::{PAGES, Page},
    testing::{
        org_status, sample_rows,
        screens::{Fixture, Shown, harness},
        state_with,
    },
};

mod gallery;

fn signed_out() -> AppState {
    let needs = AuthStateView::NeedsSignIn {
        error: UiError::new("Signed out"),
    };
    AppState {
        orgs: Arc::from([org_status("contoso", needs)]),
        ..AppState::default()
    }
}

#[test]
fn the_dashboard_lists_pull_requests_and_counts() {
    let mut h = harness(Fixture::new(Shown::Dashboard, state_with(sample_rows())));
    h.run();
    for title in [
        "Fix token refresh",
        "Paging terminator",
        "Add retry",
        "Draft work",
    ] {
        h.get_by_label(title);
    }
    h.get_by_label("2 Needs review");
    h.get_by_label("1 Active comments");
}

#[test]
fn clicking_a_count_chip_asks_for_that_status_filter() {
    let mut h = harness(Fixture::new(Shown::Dashboard, state_with(sample_rows())));
    h.run();
    h.get_by_label("1 Active comments").click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::SetStatus(StatusFilter::ActiveComments))),
        "{actions:?}"
    );
}

#[test]
fn clicking_a_row_selects_it_and_a_header_sorts() {
    let mut h = harness(Fixture::new(Shown::Dashboard, state_with(sample_rows())));
    h.run();
    h.get_by_label("Add retry").click();
    h.get_by_label("Title").click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::Select(k) if *k == crate::testing::reviewed())),
        "{actions:?}"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::Sort(prm_domain::view::SortKey::Title))),
        "{actions:?}"
    );
}

#[test]
fn a_signed_out_dashboard_offers_sign_in() {
    let mut h = harness(Fixture::new(Shown::Dashboard, signed_out()));
    h.run();
    h.get_by_label(copy::EMPTY_SIGNED_OUT);
    h.get_by_label(copy::EMPTY_SIGN_IN).click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::Modal(crate::route::ModalKind::AuthMenu))),
        "{actions:?}"
    );
}

#[test]
fn every_settings_page_renders() {
    for page in PAGES {
        let mut fixture = Fixture::new(Shown::Settings, signed_out());
        fixture.settings.page = page;
        let mut h = harness(fixture);
        h.run();
        // The sidebar entry and the page heading.
        assert_eq!(h.query_all_by_label(page.title()).count(), 2, "{page:?}");
    }
}

#[test]
fn editing_settings_enables_save() {
    let mut fixture = Fixture::new(Shown::Settings, signed_out());
    fixture.settings.page = Page::General;
    let mut h = harness(fixture);
    h.run();
    h.get_by_label(copy::START_MINIMIZED).click();
    h.run();
    assert!(h.state().settings.dirty());
    h.get_by_label(copy::SAVE_CHANGES).click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        actions.iter().any(|a| matches!(a, UiAction::SaveSettings)),
        "{actions:?}"
    );
}

#[test]
fn first_run_rejects_a_bad_url_and_connects_a_good_one() {
    let mut h = harness(Fixture::new(Shown::FirstRun, AppState::default()));
    h.run();
    h.get_by_label(copy::APP_NAME);
    h.state_mut().first_run.settings.new_org_url = "not a url at all".to_owned();
    h.get_by_label(copy::CONNECT).click();
    h.run();
    h.get_by_label(copy::BAD_ORG_URL);
    h.state_mut().first_run.settings.new_org_url = "https://dev.azure.com/contoso/".to_owned();
    h.get_by_label(copy::CONNECT).click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::Send(Command::ApplyConfig(_)))),
        "{actions:?}"
    );
    assert_eq!(
        h.state().first_run.step,
        crate::screens::first_run::Step::Repositories
    );
}

#[test]
fn the_palette_runs_the_highlighted_command_on_enter() {
    let mut fixture = Fixture::new(Shown::Palette, state_with(sample_rows()));
    fixture.palette.query = "refresh now".to_owned();
    let mut h = harness(fixture);
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(
        h.state().palette_outcome,
        Some(PaletteItem::Command(CommandId::RefreshNow))
    );
}
