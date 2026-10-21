//! The table's header: sorting, the group chevron, column dividers and resizing, and the
//! column menu.

use eframe::egui::{self, Role, accesskit::Toggled};
use egui_kittest::{
    Harness,
    kittest::{NodeT as _, Queryable as _},
};
use prm_domain::view::{GroupBy, SortKey};

use super::signed_out;
use crate::{
    actions::UiAction,
    copy,
    screens::settings::Page,
    testing::{
        demo::demo_state,
        sample_rows,
        screens::{Fixture, Shown, harness},
        state_with,
    },
    view::{ScrollIntent, columns::ColumnId},
};

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

/// Presses at `from`, moves `dx` along and holds the pointer still for a few frames; returns
/// the stored width under `key` after each of those frames.
fn drag_and_hold(
    h: &mut Harness<'_, Fixture>,
    from: egui::Pos2,
    dx: f32,
    key: &str,
) -> Vec<Option<f32>> {
    let to = from + egui::vec2(dx, 0.0);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.event(egui::Event::PointerMoved(from));
    h.step();
    h.event(button(from, true));
    h.step();
    h.event(egui::Event::PointerMoved(to));
    let mut widths = Vec::new();
    for _ in 0..6 {
        h.step();
        widths.push(h.state().dash.view.widths.get(key).copied());
    }
    h.event(button(to, false));
    h.run();
    widths
}

fn header_rect(h: &Harness<'_, Fixture>, label: &str) -> egui::Rect {
    h.get_by_role_and_label(Role::Button, label).rect()
}

/// Widths were measured from each frame's cell, whose left edge moved as Title gave up room,
/// so a column right of Title kept growing while the pointer held still; the last column had
/// no divider to drag at all.
#[test]
fn dragging_a_divider_follows_the_pointer_and_holds_still() {
    let mut fixture = Fixture::new(Shown::Dashboard, state_with(sample_rows()));
    fixture.dash.view.set_group(GroupBy::None);
    let mut h = harness(fixture);
    h.run();
    // Status sits left of Title, so its strip is its right divider; the others sit right of
    // Title and drag from their left divider. A header's target reaches the divider middles.
    let status = header_rect(&h, copy::COLUMN_STATUS);
    let repo = header_rect(&h, copy::COLUMN_REPO);
    let age = header_rect(&h, copy::COLUMN_AGE);
    let cases = [
        (
            "status",
            egui::pos2(status.right(), status.center().y),
            20.0,
            160.0,
        ),
        (
            "repo",
            egui::pos2(repo.left(), repo.center().y),
            -20.0,
            180.0,
        ),
        ("age", egui::pos2(age.left(), age.center().y), -20.0, 76.0),
    ];
    for (key, from, dx, expected) in cases {
        let widths = drag_and_hold(&mut h, from, dx, key);
        assert!(
            widths.iter().all(|w| *w == Some(expected)),
            "{key}: {widths:?}"
        );
    }
}

/// The resize cursor shown with the pointer at `pos`.
fn cursor_at(h: &mut Harness<'_, Fixture>, pos: egui::Pos2) -> egui::CursorIcon {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
    h.step();
    h.output().platform_output.cursor_icon
}

/// Resize grabbers ran down every row, so a column grew or shrank while the pointer was on a
/// data row; only the header row may resize.
#[test]
fn columns_resize_from_the_header_row_only() {
    let mut fixture = Fixture::new(Shown::Dashboard, state_with(sample_rows()));
    fixture.dash.view.set_group(GroupBy::None);
    let mut h = harness(fixture);
    h.run();
    let divider = header_rect(&h, copy::COLUMN_REPO).left();
    let header_y = header_rect(&h, copy::COLUMN_REPO).center().y;
    let row_y = h.get_by_label("Add retry").rect().center().y;
    let header = egui::pos2(divider, header_y);
    let row = egui::pos2(divider, row_y);
    assert_eq!(cursor_at(&mut h, header), egui::CursorIcon::ResizeColumn);
    assert_ne!(cursor_at(&mut h, row), egui::CursorIcon::ResizeColumn);
    let widths = drag_and_hold(&mut h, row, -20.0, "repo");
    assert!(widths.iter().all(Option::is_none), "{widths:?}");
}

/// The menu belonged to the header cell it was opened from, so hiding that column closed it.
#[test]
fn the_column_menu_stays_open_when_its_own_column_is_hidden() {
    let mut h = harness(Fixture::new(Shown::Dashboard, state_with(sample_rows())));
    h.run();
    h.get_by_role_and_label(Role::Button, copy::COLUMN_AUTHOR)
        .click_secondary();
    h.run();
    h.get_by_role_and_label(Role::MenuItemCheckBox, copy::COLUMN_AUTHOR)
        .click();
    h.run();
    for action in h.state_mut().take_actions() {
        if let UiAction::ToggleColumn(id) = action {
            h.state_mut().dash.view.toggle_column(id);
        }
    }
    h.run();
    assert_eq!(h.state().dash.view.hidden_columns, ["author"]);
    assert!(
        h.query_by_role_and_label(Role::Button, copy::COLUMN_AUTHOR)
            .is_none()
    );
    let check = h.get_by_role_and_label(Role::MenuItemCheckBox, copy::COLUMN_AUTHOR);
    assert_eq!(check.accesskit_node().toggled(), Some(Toggled::False));
}

/// A column the table had no room for still read as shown in the menu, so ticking it seemed
/// to do nothing.
#[test]
fn the_column_menu_marks_columns_a_narrow_table_left_out() {
    let mut h = harness(Fixture::new(Shown::Dashboard, state_with(sample_rows())));
    h.set_size(egui::vec2(720.0, 600.0));
    h.run();
    assert!(
        h.state().dash.view.squeezed.contains(&ColumnId::Repo),
        "{:?}",
        h.state().dash.view.squeezed
    );
    h.get_by_role_and_label(Role::Button, copy::COLUMN_AGE)
        .click_secondary();
    h.run();
    let noted = format!("{}  {}", copy::COLUMN_REPO, copy::COLUMN_NO_ROOM);
    let repo = h.get_by_role_and_label(Role::MenuItemCheckBox, &noted);
    assert_eq!(repo.accesskit_node().toggled(), Some(Toggled::True));
    repo.hover();
    h.run();
    h.get_by_label(copy::COLUMN_SQUEEZED);
}

/// Showing or hiding a column gave the table a new id and so a new scroll area, back at the
/// top.
#[test]
fn changing_the_columns_keeps_the_scroll_offset() {
    let mut h = harness(Fixture::new(Shown::Dashboard, demo_state()));
    h.set_size(egui::vec2(1280.0, 360.0));
    h.run();
    h.state_mut().dash.view.scroll = ScrollIntent::Offset(90.0);
    h.run();
    let before = h.state().dash.view.offset_y;
    assert!(before > 45.0, "the table did not scroll: {before}");
    let changes: [fn(&mut Fixture); 2] = [
        |f| f.dash.view.toggle_column(ColumnId::Reviews),
        |f| f.dash.view.reset_columns(),
    ];
    for change in changes {
        change(h.state_mut());
        h.run();
        let after = h.state().dash.view.offset_y;
        assert!((after - before).abs() < 0.5, "{before} -> {after}");
    }
}
