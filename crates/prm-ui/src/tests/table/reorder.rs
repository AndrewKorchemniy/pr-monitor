//! The header's cells fill the header row, the PR cell is one target, and a drag on a cell moves
//! its column.

use eframe::egui::{self, Key, Modifiers, Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT as _, Queryable as _},
};
use prm_domain::view::{GroupBy, SortKey};

use super::{click_at, header_rect};
use crate::{
    actions::UiAction,
    copy,
    render::policy::RenderPolicy,
    testing::{
        sample_rows,
        screens::{Fixture, Shown, harness},
        state_with,
    },
    view::columns::{ColumnUnit, DEFAULT_ORDER},
};

fn dashboard() -> Harness<'static, Fixture> {
    let mut fixture = Fixture::new(Shown::Dashboard, state_with(sample_rows()));
    fixture.dash.view.set_group(GroupBy::None);
    let mut h = harness(fixture);
    h.run();
    h
}

/// The headers a default table shows, left to right.
const HEADERS: [&str; 7] = [
    copy::COLUMN_NUMBER,
    copy::COLUMN_TITLE,
    copy::COLUMN_REPO,
    copy::COLUMN_AUTHOR,
    copy::REVIEWS,
    copy::COLUMN_COMMENTS,
    copy::COLUMN_AGE,
];

/// The shown headers, sorted by where they lie.
fn header_order(h: &Harness<'_, Fixture>) -> Vec<&'static str> {
    let mut found: Vec<_> = HEADERS
        .iter()
        .filter_map(|label| {
            let node = h.query_by_role_and_label(Role::Button, label)?;
            Some((node.rect().left(), *label))
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, label)| label).collect()
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Presses at `from`, moves to `to` in a few steps and, with `release`, lets go there.
fn drag(h: &mut Harness<'_, Fixture>, (from, to): (egui::Pos2, egui::Pos2), release: bool) {
    h.event(egui::Event::PointerMoved(from));
    h.step();
    h.event(button(from, true));
    h.step();
    for step in 1..=6 {
        let pos = from + (to - from) * (step as f32 / 6.0);
        h.event(egui::Event::PointerMoved(pos));
        h.step();
    }
    if release {
        h.event(button(to, false));
        h.step();
        h.run();
    }
}

fn centre(h: &Harness<'_, Fixture>, label: &str) -> egui::Pos2 {
    header_rect(h, label).center()
}

/// The x of the first thing in the rows with this text.
fn row_x(h: &Harness<'_, Fixture>, text: &str) -> f32 {
    h.query_all_by_label(text)
        .map(|n| n.rect().left())
        .fold(f32::INFINITY, f32::min)
}

/// Between two cells' targets was a gap no click reached, and right of Age the trailing margin
/// and the scroll bar's lane were dead; the left edge had the margin's own blank cell.
#[test]
fn header_cells_tile_the_header_and_the_last_reaches_the_right_edge() {
    let h = dashboard();
    let cells: Vec<egui::Rect> = HEADERS.iter().map(|l| header_rect(&h, l)).collect();
    for pair in cells.windows(2) {
        assert_eq!(pair[0].right(), pair[1].left(), "{pair:?}");
    }
    for cell in &cells {
        assert_eq!(cell.y_range(), cells[0].y_range());
    }
    // Rows start at the list's left edge, and the header at the same place.
    let rows_left = h.get_by_label("Fix token refresh").rect().left();
    assert!(cells[0].left() < rows_left, "{cells:?} vs {rows_left}");
    let list = h.get_by_label(crate::commands::CommandId::SettingsOpen.title());
    let edge = list.rect().right();
    let last = cells.last().map_or(0.0, egui::Rect::right);
    assert!(
        last > edge,
        "the last header cell ends at {last}, past the toolbar's {edge}"
    );
}

/// A click in the blank right of Age sorts by age, and a click left of the PR label or over
/// the flags sorts by number: there are no ghost cells.
#[test]
fn clicking_the_blank_ends_of_the_header_sorts_the_cell_they_belong_to() {
    let mut h = dashboard();
    let age = header_rect(&h, copy::COLUMN_AGE);
    let pr = header_rect(&h, copy::COLUMN_NUMBER);
    let y = age.center().y;
    for (x, key) in [
        (age.right() - 1.0, SortKey::Age),
        (pr.left() + 1.0, SortKey::Number),
        (pr.right() - 1.0, SortKey::Number),
    ] {
        let actions = click_at(&mut h, egui::pos2(x, y));
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, UiAction::Sort(k) if *k == key)),
            "x = {x}: {actions:?}"
        );
    }
}

#[test]
fn reviews_and_required_headers_sort() {
    let mut fixture = Fixture::new(Shown::Dashboard, state_with(sample_rows()));
    fixture.dash.view.set_group(GroupBy::None);
    fixture.dash.view.shown_columns.push("required".to_owned());
    let mut h = harness(fixture);
    h.run();
    for (label, key) in [
        (copy::REVIEWS, SortKey::Reviews),
        (copy::COLUMN_REQUIRED, SortKey::Required),
    ] {
        let at = centre(&h, label);
        let actions = click_at(&mut h, at);
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, UiAction::Sort(k) if *k == key)),
            "{label}: {actions:?}"
        );
    }
}

/// Dragging Repository right past Author's middle moves it after Author, saves that, and the
/// rows follow the header; a click without a drag only sorts.
#[test]
fn dragging_a_header_past_another_moves_the_column_and_the_rows_follow() {
    let mut h = dashboard();
    assert!(row_x(&h, "api") < row_x(&h, "Alice Smith"));
    let author = header_rect(&h, copy::COLUMN_AUTHOR);
    let from = centre(&h, copy::COLUMN_REPO);
    let to = from + egui::vec2(author.width() / 2.0 + 20.0, 0.0);
    drag(&mut h, (from, to), true);
    let order = h.state().dash.view.column_order.clone();
    let at = |unit| order.iter().position(|u| *u == unit);
    assert_eq!(at(ColumnUnit::Author).map(|i| i + 1), at(ColumnUnit::Repo));
    assert_eq!(
        header_order(&h),
        [
            copy::COLUMN_NUMBER,
            copy::COLUMN_TITLE,
            copy::COLUMN_AUTHOR,
            copy::COLUMN_REPO,
            copy::REVIEWS,
            copy::COLUMN_COMMENTS,
            copy::COLUMN_AGE
        ]
    );
    assert!(row_x(&h, "Alice Smith") < row_x(&h, "api"));
    let actions = h.state_mut().take_actions();
    assert!(
        !actions.iter().any(|a| matches!(a, UiAction::Sort(_))),
        "a drag does not sort: {actions:?}"
    );
    let mut saved = prm_config::UiState::default();
    h.state().dash.view.write_ui_state(&mut saved);
    assert_eq!(saved.column_order.len(), 9, "{:?}", saved.column_order);
}

/// While the header is held the table is already laid out in the order a drop would save,
/// header and rows alike, and the saved order is still the old one.
#[test]
fn the_table_lays_out_in_the_tentative_order_while_a_header_is_held() {
    let mut h = dashboard();
    let age = header_rect(&h, copy::COLUMN_AGE);
    let from = centre(&h, copy::COLUMN_TITLE);
    let to = egui::pos2(age.center().x, from.y);
    drag(&mut h, (from, to), false);
    assert!(h.state().dash.view.reorder.is_some());
    assert_eq!(h.state().dash.view.column_order, DEFAULT_ORDER);
    assert_eq!(header_order(&h).last(), Some(&copy::COLUMN_TITLE));
    // The lifted cell follows the pointer, so it is not drawn at its place in the row.
    let lifted = header_rect(&h, copy::COLUMN_TITLE);
    assert!(row_x(&h, "Fix token refresh") > row_x(&h, "api"));
    assert!(lifted.width() > 0.0);
    h.event(button(to, false));
    h.step();
    h.run();
    assert!(h.state().dash.view.reorder.is_none());
    // Dropped after Age; the hidden Organization column keeps its place after it.
    let order = h.state().dash.view.column_order.clone();
    let at = |unit| order.iter().position(|u| *u == unit);
    assert_eq!(at(ColumnUnit::Age).map(|i| i + 1), at(ColumnUnit::Title));
}

/// Esc puts the held header back; letting go afterwards changes nothing.
#[test]
fn cancelling_a_drag_restores_the_order() {
    let mut h = dashboard();
    let before = header_order(&h);
    let from = centre(&h, copy::COLUMN_REPO);
    let to = from + egui::vec2(260.0, 0.0);
    drag(&mut h, (from, to), false);
    assert_ne!(header_order(&h), before);
    h.state_mut().dash.view.cancel_column_drag();
    h.run();
    assert_eq!(header_order(&h), before);
    h.event(button(to, false));
    h.run();
    assert_eq!(header_order(&h), before);
    assert_eq!(h.state().dash.view.column_order, DEFAULT_ORDER);
}

/// A press on a divider resizes; it never lifts the header.
#[test]
fn a_divider_drag_resizes_and_does_not_move_a_column() {
    let mut h = dashboard();
    let repo = header_rect(&h, copy::COLUMN_REPO);
    let from = egui::pos2(repo.left(), repo.center().y);
    let to = from + egui::vec2(-30.0, 0.0);
    drag(&mut h, (from, to), true);
    assert_eq!(h.state().dash.view.column_order, DEFAULT_ORDER);
    assert!(h.state().dash.view.widths.contains_key("repo"));
    assert_eq!(header_order(&h), HEADERS);
}

/// The PR group moves as one unit, and the change bar's margin stays at the left edge.
#[test]
fn the_pr_group_moves_whole_and_the_margin_stays_put() {
    let mut h = dashboard();
    let first_x = row_x(&h, "Fix token refresh");
    let pr_x = row_x(&h, "!101");
    let from = centre(&h, copy::COLUMN_NUMBER);
    let to = egui::pos2(header_rect(&h, copy::COLUMN_REPO).center().x, from.y);
    drag(&mut h, (from, to), true);
    let order = h.state().dash.view.column_order.clone();
    assert_eq!(order.first(), Some(&ColumnUnit::Title));
    assert_ne!(order.first(), Some(&ColumnUnit::Pr));
    // Title leads now, right after the margin, and the PR group follows it.
    let title = row_x(&h, "Fix token refresh");
    assert!(title < 40.0, "{title} (it was at {first_x})");
    assert!(
        row_x(&h, "!101") > title,
        "{} vs {title} (was {pr_x})",
        row_x(&h, "!101")
    );
}

/// Alt with an arrow key on a focused header, and the column menu's items, move a column
/// without the pointer.
#[test]
fn the_keyboard_and_the_menu_move_a_column() {
    let mut h = dashboard();
    h.get_by_role_and_label(Role::Button, copy::COLUMN_AUTHOR)
        .focus();
    h.run();
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowLeft);
    h.run();
    let order = h.state().dash.view.column_order.clone();
    let at = |unit| order.iter().position(|u| *u == unit);
    assert!(at(ColumnUnit::Author) < at(ColumnUnit::Repo), "{order:?}");
    h.get_by_role_and_label(Role::Button, copy::COLUMN_TITLE)
        .click_secondary();
    h.run();
    h.get_by_role_and_label(Role::MenuItem, &copy::move_column_right("Title"))
        .click();
    h.run();
    let order = h.state().dash.view.column_order.clone();
    let at = |unit| order.iter().position(|u| *u == unit);
    assert!(at(ColumnUnit::Title) > at(ColumnUnit::Author), "{order:?}");
    assert_eq!(h.state().dash.view.column_order.len(), 9);
}

/// The ends of the row have nothing to move to.
#[test]
fn the_menu_cannot_move_the_first_column_left() {
    let mut h = dashboard();
    h.get_by_role_and_label(Role::Button, copy::COLUMN_NUMBER)
        .click_secondary();
    h.run();
    let name = copy::move_column_left("PR");
    let item = h.get_by_role_and_label(Role::MenuItem, &name);
    assert!(item.accesskit_node().is_disabled());
    h.get_by_role_and_label(Role::MenuItem, copy::SORT_BY_STATUS)
        .click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::Sort(SortKey::Status))),
        "{actions:?}"
    );
}

/// Reset columns clears a moved order along with widths and visibility.
#[test]
fn reset_columns_restores_the_default_order() {
    let mut h = dashboard();
    let from = centre(&h, copy::COLUMN_REPO);
    drag(&mut h, (from, from + egui::vec2(260.0, 0.0)), true);
    assert_ne!(h.state().dash.view.column_order, DEFAULT_ORDER);
    h.state_mut().dash.view.reset_columns();
    h.run();
    assert_eq!(h.state().dash.view.column_order, DEFAULT_ORDER);
    assert_eq!(header_order(&h), HEADERS);
}

/// With animations on, columns that change places slide into them instead of jumping; with
/// them off (as in the other tests) they are there at once.
#[test]
fn columns_slide_when_animations_are_on_and_the_slide_ends() {
    let mut fixture = Fixture::new(Shown::Dashboard, state_with(sample_rows()));
    fixture.dash.view.set_group(GroupBy::None);
    fixture.theme.set_policy(RenderPolicy {
        low_power: false,
        animations: true,
    });
    let mut h = harness(fixture);
    h.run();
    let before = row_x(&h, "api");
    h.state_mut().dash.view.step_column(
        (ColumnUnit::Repo, crate::view::columns::Step::Right),
        |_| true,
    );
    h.step();
    let mid = row_x(&h, "api");
    let settled = {
        h.run();
        row_x(&h, "api")
    };
    assert!(settled > before, "{before} -> {settled}");
    assert!(mid < settled, "mid-slide {mid}, settled {settled}");
}
