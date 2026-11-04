//! The Required column: off until chosen, then a chip per required group named by its tooltip;
//! the same chips on cards and in the peek.

use eframe::egui::{self, Role};
use egui_kittest::{Harness, kittest::Queryable as _};

use crate::{
    actions::UiAction,
    copy,
    testing::{
        demo::{demo_state, now},
        screens::{Fixture, Shown, WINDOW, harness_at},
    },
    view::columns::{ColumnCheck, ColumnId},
};

/// The demo pull request with two required groups, one approved and one not reviewed.
const SUMMARY: &str =
    "IPCO Repo Approver - CF: approved via Sam Whitfield\nPlatform Owners: no review yet";

const CARDS: egui::Vec2 = egui::vec2(480.0, 620.0);

fn dashboard(size: egui::Vec2, column: bool, peek: bool) -> Harness<'static, Fixture> {
    let mut fixture = Fixture::new(Shown::Dashboard, demo_state());
    fixture.now = now();
    if column {
        fixture.dash.view.shown_columns.push("required".to_owned());
    }
    fixture.dash.peek.open = peek;
    let with_groups = fixture
        .state
        .rows
        .iter()
        .find(|r| r.required_groups.len() > 1);
    if let Some(key) = with_groups.map(|r| r.key.clone()) {
        fixture.dash.view.select_key(&key);
    }
    let mut h = harness_at(fixture, size);
    h.run();
    h
}

fn chips(h: &Harness<'_, Fixture>) -> usize {
    h.query_all_by_role_and_label(Role::Label, SUMMARY).count()
}

#[test]
fn the_column_is_off_until_it_is_chosen() {
    assert_eq!(chips(&dashboard(WINDOW, false, false)), 0);
    assert_eq!(chips(&dashboard(WINDOW, true, false)), 1);
}

/// Rows with required groups draw one summary each; every other row's cell is blank.
#[test]
fn only_pull_requests_with_required_groups_draw_a_chip() {
    let h = dashboard(WINDOW, true, false);
    let rows = &h.state().state.rows;
    let with_groups = rows
        .iter()
        .filter(|r| !r.required_groups.is_empty())
        .count();
    assert!(0 < with_groups && with_groups < rows.len());
    let summaries = rows
        .iter()
        .filter(|r| !r.required_groups.is_empty())
        .map(|r| {
            let lines: Vec<_> = r.required_groups.iter().map(copy::group_status).collect();
            lines.join("\n")
        });
    let drawn = summaries
        .filter(|s| h.query_by_role_and_label(Role::Label, s).is_some())
        .count();
    assert_eq!(drawn, with_groups);
}

#[test]
fn the_column_menu_offers_the_column() {
    let mut h = dashboard(WINDOW, false, false);
    h.get_by_label(copy::COLUMN_AGE).click_secondary();
    h.run();
    h.get_by_role_and_label(Role::MenuItemCheckBox, copy::COLUMN_REQUIRED)
        .click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::ToggleColumn(ColumnId::Required))),
        "{actions:?}"
    );
}

#[test]
fn cards_show_the_chips_only_when_the_column_is_on() {
    assert_eq!(chips(&dashboard(CARDS, false, false)), 0);
    assert_eq!(chips(&dashboard(CARDS, true, false)), 1);
}

/// Cards draw the chips themselves, so the checklist does not call the column left out.
#[test]
fn the_checklist_does_not_call_the_column_left_out_while_cards_show_it() {
    let h = dashboard(CARDS, true, false);
    let choices = h.state().dash.view.column_choices();
    let required = crate::view::columns::column(ColumnId::Required).unwrap();
    assert_eq!(choices.check(&required), ColumnCheck::On);
}

#[test]
fn the_peek_names_each_group_and_where_its_review_stands() {
    let h = dashboard(WINDOW, false, true);
    h.get_by_label(copy::REQUIRED_GROUPS);
    h.get_by_label("approved via Sam Whitfield");
    h.get_by_label("no review yet");
    h.get_by_role_and_label(
        Role::Image,
        "IPCO Repo Approver - CF: approved via Sam Whitfield",
    );
}
