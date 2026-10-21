//! Lines and lists that have to fit a small window: the palette's results scroll under a still
//! highlight, the repositories dialog in a very short window and its count under its check box,
//! and the sign-in line with a long name.

use std::sync::Arc;

use eframe::egui::{self, Rect, Role, vec2};
use egui_kittest::{
    Harness, Node,
    kittest::{AccessKitNode, NodeT as _, Queryable as _},
};
use prm_domain::testing::person;
use prm_service::AppState;

use super::{SIZES, lit_row, shown_at};
use crate::{
    copy,
    screens::settings::{Editor, Page},
    testing::{
        demo::demo_state,
        overflow::{assert_clickable, assert_overlays_fit},
        screens::{Fixture, Shown},
    },
};

/// The node named `name` in the dialog opened last (over settings, or the palette).
fn in_top_dialog<'h>(h: &'h Harness<'_, Fixture>, role: Role, name: &str) -> Node<'h> {
    let dialog = h.query_all_by_role(Role::Dialog).last().expect("a dialog");
    let matches = |n: &Node<'_>| {
        let node = n.accesskit_node();
        let text = if node.role() == Role::Label {
            node.value()
        } else {
            node.label()
        };
        node.role() == role && text.as_deref() == Some(name)
    };
    let found = dialog.children_recursive().find(matches);
    found.unwrap_or_else(|| panic!("no {role:?} {name:?} in the dialog"))
}

/// The palette listed a window of rows that followed the highlight: Up moved the whole list
/// rather than the highlight, and a pointer could not reach the rows below.
#[test]
fn the_palette_scrolls_under_a_still_highlight() {
    let size = SIZES[0];
    let mut h = shown_at(Shown::Palette, demo_state(), |_| {}, size);
    let field = |n: &AccessKitNode<'_>| n.data().placeholder() == Some(copy::PALETTE_HINT);
    let field = h.get_by(field).rect();
    let footer = h.get_by_label(copy::PALETTE_FOOTER).rect();
    let shown = |row: Rect| field.bottom() <= row.top() && row.bottom() <= footer.top();
    for _ in 0..14 {
        h.key_press(egui::Key::ArrowDown);
    }
    h.run();
    let lit = lit_row(&h);
    let (name, at) = (lit.accesskit_node().label().unwrap_or_default(), lit.rect());
    assert!(shown(at), "the highlight at {at:?} is in the list");
    h.key_press(egui::Key::ArrowUp);
    h.run();
    let row = in_top_dialog(&h, Role::Button, &name).rect();
    assert_eq!(row, at, "the list holds still");
    let lit = lit_row(&h).rect();
    assert!(
        shown(lit) && lit.bottom() <= at.top(),
        "the highlight moved up"
    );
    let selected = h.state().palette.selected;
    lit_row(&h).hover();
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, -3.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert_eq!(h.state().palette.selected, selected, "the wheel scrolls");
    let row = in_top_dialog(&h, Role::Button, &name).rect();
    assert!(row.top() < at.top(), "the rows below came up: {row:?}");
}

/// The count of chosen repositories, moved under "Watch all projects", sat under the chevrons.
#[test]
fn the_repository_count_lines_up_with_its_check_box() {
    let setup = |f: &mut Fixture| {
        f.settings.page = Page::Organizations;
        f.settings.editing = Some(Editor::Repositories);
    };
    let h = shown_at(Shown::Settings, demo_state(), setup, SIZES[0]);
    let check = in_top_dialog(&h, Role::CheckBox, copy::WATCH_ALL_PROJECTS).rect();
    let count = in_top_dialog(&h, Role::Label, &copy::repos_selected(2, 10)).rect();
    assert!(count.top() >= check.bottom(), "{count:?} under {check:?}");
    assert!(
        (count.left() - check.left()).abs() < 1.0,
        "{count:?} under {check:?}"
    );
}

/// In a window as short as the app allows, the project list scrolled in a box of a few rows
/// inside the scrolling dialog. It now scrolls with the dialog: one scroll bar, every row in
/// reach, and the last one clear of the action bar.
#[test]
fn the_repositories_dialog_fits_a_short_window() {
    let size = vec2(360.0, 420.0);
    let setup = |f: &mut Fixture| {
        f.settings.page = Page::Organizations;
        f.settings.editing = Some(Editor::Repositories);
        // why: a search opens every project, so the list is longer than the dialog.
        "e".clone_into(&mut f.settings.scope_filter.text);
    };
    let mut h = shown_at(Shown::Settings, demo_state(), setup, size);
    assert_overlays_fit(&h, size);
    assert_clickable(&h, &in_top_dialog(&h, Role::Button, copy::DONE), size);
    let dialog = h.query_all_by_role(Role::Dialog).last().expect("a dialog");
    let rows: Vec<String> = dialog
        .children_recursive()
        .filter(|n| n.accesskit_node().role() == Role::CheckBox)
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    let bars = dialog
        .children_recursive()
        .filter(|n| n.accesskit_node().role() == Role::ScrollBar)
        .count();
    assert_eq!(bars, 1, "one scroll bar, the dialog's");
    let last = rows.last().expect("projects are listed");
    in_top_dialog(&h, Role::CheckBox, last).scroll_to_me();
    h.run();
    assert_overlays_fit(&h, size);
    let row = in_top_dialog(&h, Role::CheckBox, last);
    assert_clickable(&h, &row, size);
    let done = in_top_dialog(&h, Role::Button, copy::DONE).rect();
    assert!(
        row.rect().bottom() <= done.top(),
        "{last} is above the action bar"
    );
}

/// Signed in with a long name, so the buttons cannot sit beside the state line.
fn long_name() -> AppState {
    let mut state = demo_state();
    let mut orgs = state.orgs.to_vec();
    orgs[0].auth.me = Some(person(1, "Maya Alexandra Patel-Okafor Whitfield"));
    state.orgs = Arc::from(orgs);
    state
}

/// The buttons were drawn over a state line too long to leave them room, and came in the
/// opposite order on the line below.
#[test]
fn the_sign_in_buttons_never_cover_the_state_line() {
    let sizes = [SIZES[0], SIZES[1], vec2(600.0, 800.0), vec2(1280.0, 760.0)];
    for state in [demo_state as fn() -> AppState, long_name] {
        for size in sizes {
            let setup = |f: &mut Fixture| f.settings.page = Page::Organizations;
            let h = shown_at(Shown::Settings, state(), setup, size);
            assert_overlays_fit(&h, size);
            let change = h.get_by_label(copy::CHANGE_SIGN_IN).rect();
            let sign_out = h.get_by_label(copy::SIGN_OUT).rect();
            assert!(
                change.right() <= sign_out.left(),
                "{size:?}: Sign out comes last"
            );
            assert_eq!(change.top(), sign_out.top(), "{size:?}: on one line");
        }
    }
}
