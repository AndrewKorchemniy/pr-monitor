//! Screens rendered headlessly with `egui_kittest`: what a user sees and what clicking does.
//! Accessibility checks are on and a missing glyph fails the test.

use std::sync::Arc;

use eframe::egui::{self, Role};
use egui_kittest::{Harness, kittest::Queryable as _};
use prm_domain::view::{SortKey, StatusFilter};
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
    view::columns::ColumnId,
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
            .any(|a| matches!(a, UiAction::Sort(SortKey::Title))),
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

/// A primary click at `pos` the way a mouse sends it: a move, a press and a release, each in
/// its own frame; returns the actions it queued.
fn click_at(h: &mut Harness<'_, Fixture>, pos: egui::Pos2) -> Vec<UiAction> {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.step();
    }
    h.run();
    h.state_mut().take_actions()
}

/// Only the header's text took clicks; its sort arrow and the rest of the cell did nothing, so
/// flipping a sort often took a second click.
#[test]
fn one_click_anywhere_in_a_header_cell_sorts_and_the_next_reverses() {
    let mut h = harness(Fixture::new(Shown::Dashboard, state_with(sample_rows())));
    h.run();
    let header = h.get_by_label(copy::COLUMN_REPO).rect();
    // Far right of the word, still inside the cell.
    let pos = egui::pos2(header.right() - 12.0, header.center().y);
    for descending in [false, true] {
        let actions = click_at(&mut h, pos);
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, UiAction::Sort(SortKey::Repo))),
            "{actions:?}"
        );
        h.state_mut().dash.view.sort_by(SortKey::Repo);
        h.run();
        let sort = h.state().dash.view.sort;
        assert_eq!((sort.key, sort.descending), (SortKey::Repo, descending));
    }
}

/// The margin column's resize grabber sat on the group chevron and took the click.
#[test]
fn clicking_a_group_header_chevron_toggles_the_group() {
    let mut h = harness(Fixture::new(Shown::Dashboard, state_with(sample_rows())));
    h.run();
    let group = h
        .get_by_role_and_label(Role::Button, "Active comments")
        .rect();
    let actions = click_at(&mut h, egui::pos2(group.left() + 12.0, group.center().y));
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::ToggleGroup(1))),
        "{actions:?}"
    );
}

/// Resize grabbers ran down every row, so a click on a column divider did not select the row.
#[test]
fn clicking_a_row_on_a_column_divider_selects_it() {
    let mut h = harness(Fixture::new(Shown::Dashboard, state_with(sample_rows())));
    h.run();
    let row = h.get_by_label("Add retry").rect();
    // The header target reaches the middle of the gap: the Title | Repository divider.
    let divider = h.get_by_label(copy::COLUMN_REPO).rect().left();
    let actions = click_at(&mut h, egui::pos2(divider, row.center().y));
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::Select(k) if *k == crate::testing::reviewed())),
        "{actions:?}"
    );
}

#[test]
fn the_column_menu_toggles_optional_columns_but_not_title() {
    let mut h = harness(Fixture::new(Shown::Dashboard, state_with(sample_rows())));
    h.run();
    h.get_by_label(copy::COLUMN_AUTHOR).click_secondary();
    h.run();
    h.get_by_role_and_label(Role::MenuItemCheckBox, copy::REVIEWS)
        .click();
    h.run();
    h.get_by_role_and_label(Role::MenuItemCheckBox, copy::COLUMN_TITLE)
        .click();
    h.run();
    let actions = h.state_mut().take_actions();
    let toggled: Vec<_> = actions
        .iter()
        .filter_map(|a| {
            if let UiAction::ToggleColumn(id) = a {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(toggled, [ColumnId::Reviews], "{actions:?}");
}

#[test]
fn the_settings_column_checklist_toggles_columns() {
    let mut fixture = Fixture::new(Shown::Settings, signed_out());
    fixture.settings.page = Page::General;
    let mut h = harness(fixture);
    h.run();
    h.get_by_role_and_label(Role::CheckBox, copy::COLUMN_FLAGS)
        .click_accesskit();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::ToggleColumn(ColumnId::Flags))),
        "{actions:?}"
    );
}
