//! The dashboard at every width: the table filling a wide window, either side of the card
//! breakpoint, the cards in phone-sized windows, the cards' menus and the peek over them.

use eframe::egui;
use egui_kittest::{Harness, kittest::Queryable as _};
use prm_domain::view::GroupBy;

use super::{Pointer, Scene, nothing, scene};
use crate::{
    copy,
    screens::dashboard::table_breakpoint,
    testing::{
        demo::demo_state,
        screens::{Fixture, HARNESS_MARGIN, Shown},
    },
    theme::{self, adapter},
};
/// A phone-sized window's height, so the cards fill a screen (points).
const TALL: f32 = 760.0;
/// The card the menu scenes select and open a menu on.
const CARD: &str = "Retry 429 responses with Retry-After";

/// The narrowest window, in whole points, that still shows the table.
fn breakpoint() -> f32 {
    let theme = theme::fallback();
    let style = adapter::style(&theme, theme.policy);
    table_breakpoint(&style.spacing, &theme).ceil() + 2.0 * HARNESS_MARGIN
}

fn ungrouped(f: &mut Fixture) {
    f.dash.view.set_group(GroupBy::None);
}

fn select_card(f: &mut Fixture) {
    let key = f.state.rows.iter().find(|r| r.title.as_ref() == CARD);
    if let Some(key) = key.map(|r| r.key.clone()) {
        f.dash.view.select_key(&key);
    }
}

fn peek_card(f: &mut Fixture) {
    select_card(f);
    f.dash.peek.open = true;
}

/// The second row chosen from the keyboard, so it shows the focus ring.
fn keyboard_row(f: &mut Fixture) {
    f.dash.view.nav_edge(false);
    f.dash.view.nav(1);
}

/// The table filling a wide window, the table just above the breakpoint and the cards just
/// below it, then the cards in phone-sized windows, grouped and not, the peek over them, and a
/// row chosen from the keyboard in the cards and the table.
pub(super) fn scenes() -> Vec<Scene> {
    let at = breakpoint();
    let size = |width: f32| egui::vec2(width, TALL);
    let mut all = vec![
        scene("dashboard-1920", Shown::Dashboard, demo_state, nothing)
            .at(egui::vec2(1920.0, 1000.0)),
        scene(
            "dashboard-breakpoint-table",
            Shown::Dashboard,
            demo_state,
            nothing,
        )
        .at(size(at)),
        scene(
            "dashboard-breakpoint-cards",
            Shown::Dashboard,
            demo_state,
            nothing,
        )
        .at(size(at - 1.0)),
        scene("dashboard-cards-480", Shown::Dashboard, demo_state, nothing).at(size(480.0)),
        scene(
            "dashboard-cards-480-ungrouped",
            Shown::Dashboard,
            demo_state,
            ungrouped,
        )
        .at(size(480.0)),
        scene("dashboard-cards-360", Shown::Dashboard, demo_state, nothing).at(size(360.0)),
        scene(
            "dashboard-cards-360-ungrouped",
            Shown::Dashboard,
            demo_state,
            ungrouped,
        )
        .at(size(360.0)),
        scene(
            "dashboard-cards-peek",
            Shown::Dashboard,
            demo_state,
            peek_card,
        )
        .at(size(480.0)),
        scene(
            "dashboard-banners-narrow",
            Shown::Dashboard,
            demo_state,
            super::banner_shapes,
        )
        .at(size(360.0)),
    ];
    all.extend(keyboard_scenes());
    all
}

/// A row chosen from the keyboard, with its focus ring, in the cards and in the table.
fn keyboard_scenes() -> [Scene; 2] {
    [
        scene(
            "dashboard-cards-keyboard",
            Shown::Dashboard,
            demo_state,
            keyboard_row,
        )
        .at(egui::vec2(480.0, TALL)),
        scene(
            "dashboard-table-keyboard",
            Shown::Dashboard,
            demo_state,
            keyboard_row,
        )
        .at(egui::vec2(1280.0, TALL)),
    ]
}

fn open_card_menu(h: &mut Harness<'_, Fixture>) {
    h.get_by_label(CARD).click_secondary();
}

fn open_sort_menu(h: &mut Harness<'_, Fixture>) {
    let name = copy::menu_value(copy::SORT, copy::COLUMN_STATUS);
    h.get_by_label(&name).click();
}

/// A selected card with its context menu open, and the toolbar's sort menu.
pub(super) fn pointer_scenes() -> [(Scene, Pointer); 3] {
    [
        (
            scene(
                "dashboard-cards-menu",
                Shown::Dashboard,
                demo_state,
                select_card,
            )
            .at(egui::vec2(480.0, TALL)),
            open_card_menu,
        ),
        (
            scene(
                "dashboard-cards-menu-360",
                Shown::Dashboard,
                demo_state,
                select_card,
            )
            .at(egui::vec2(360.0, TALL)),
            open_card_menu,
        ),
        (
            scene(
                "dashboard-cards-sort",
                Shown::Dashboard,
                demo_state,
                nothing,
            )
            .at(egui::vec2(360.0, TALL)),
            open_sort_menu,
        ),
    ]
}
