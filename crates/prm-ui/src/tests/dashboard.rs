//! The dashboard at every window width: the table fills the window, saved widths never push it
//! past the edge, a narrow window lists cards, and the toolbar, banners and peek fit from the
//! smallest window up.

use std::collections::BTreeMap;

use eframe::egui::{self, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT as _, Queryable as _},
};
use prm_domain::view::GroupBy;

use crate::{
    commands::CommandId,
    copy,
    render::hover_bands::TableGeometry,
    screens::dashboard::table_breakpoint,
    testing::{
        demo::{demo_state, now},
        screens::{Fixture, HARNESS_MARGIN, NARROW, Shown, WINDOW, harness_at},
    },
    theme::{self, adapter},
};

/// The demo dashboard at the demo's "now", grouped by `group`, in a window of `size` points.
fn demo(group: GroupBy, size: egui::Vec2) -> Harness<'static, Fixture> {
    let mut fixture = Fixture::new(Shown::Dashboard, demo_state());
    fixture.now = now();
    fixture.dash.view.set_group(group);
    let mut h = harness_at(fixture, size);
    h.run();
    h
}

/// Resizes the window and lets it settle: a `Ui`'s response keeps last frame's rect, so one
/// more frame catches up.
fn resize(h: &mut Harness<'_, Fixture>, size: egui::Vec2) {
    h.set_size(size);
    h.run();
    h.run_steps(1);
}

/// The dashboard's right edge: the harness draws it inside a margin of its own.
fn dashboard_right(h: &Harness<'_, Fixture>) -> f32 {
    let theme = theme::fallback();
    let settings = h.get_by_label(CommandId::SettingsOpen.title()).rect();
    settings.right() + adapter::frame_chrome(&theme).total_margin().right
}

#[test]
fn the_harness_margin_is_the_one_the_tests_allow_for() {
    let h = demo(GroupBy::Status, WINDOW);
    let margin = WINDOW.x - dashboard_right(&h);
    assert!((margin - HARNESS_MARGIN).abs() < 0.5, "{margin}");
}

/// Where the list's rows end: inside the body's right margin and the scroll bar's lane.
fn list_end(h: &Harness<'_, Fixture>) -> f32 {
    let theme = theme::fallback();
    let style = adapter::style(&theme, theme.policy);
    let margin = adapter::frame_table(&theme).total_margin().right;
    dashboard_right(h) - margin - style.spacing.scroll.allocated_width()
}

/// Half the gap between columns, which a header cell's target reaches into.
fn half_gap() -> f32 {
    let theme = theme::fallback();
    adapter::style(&theme, theme.policy).spacing.item_spacing.x / 2.0
}

/// The narrowest dashboard that still shows the table (its peek closed), in points.
fn breakpoint() -> f32 {
    let theme = theme::fallback();
    let style = adapter::style(&theme, theme.policy);
    table_breakpoint(&style.spacing, &theme)
}

/// Every label and button lies inside a window `width` points wide.
fn assert_inside(h: &Harness<'_, Fixture>, width: f32, case: &str) {
    for node in h.root().children_recursive() {
        let role = node.accesskit_node().role();
        if !matches!(role, Role::Label | Role::Button | Role::ComboBox) {
            continue;
        }
        let rect = node.rect();
        assert!(
            rect.left() >= -0.5 && rect.right() <= width + 0.5,
            "{case}: {role:?} {:?} at {rect:?} is outside {width} pt",
            node.accesskit_node().label()
        );
    }
}

/// The table's rows and header both reach the list's right edge.
fn assert_fills(h: &Harness<'_, Fixture>, case: &str) {
    let end = list_end(h);
    // why: the newest demo PR is 3h old, and Age, right-aligned, is the last column.
    let row = h.get_by_label("3h").rect().right();
    let header = h
        .get_by_role_and_label(Role::Button, copy::COLUMN_AGE)
        .rect();
    assert!(
        (row - end).abs() <= 2.0,
        "{case}: rows end at {row}, the list at {end}"
    );
    let header = header.right() - half_gap();
    assert!(
        (header - end).abs() <= 2.0,
        "{case}: header ends at {header}, not {end}"
    );
}

/// The title kept the width it was first given (`egui_extras` keeps a resizable column's
/// width), so in a wider window the rows stopped short of the right edge (issue 9).
#[test]
fn the_table_fills_the_window_as_it_grows_and_shrinks() {
    for group in [GroupBy::Status, GroupBy::None] {
        let mut h = demo(group, WINDOW);
        for width in [1280.0, 1920.0, 2560.0, 1000.0, 1280.0] {
            resize(&mut h, egui::vec2(width, WINDOW.y));
            let case = format!("{group:?} at {width} pt");
            assert_fills(&h, &case);
            assert_inside(&h, width, &case);
        }
    }
}

/// Widths dragged in a wide window made the table wider than a narrower one.
#[test]
fn large_saved_widths_shrink_to_fit_and_stay_saved() {
    let keys = [
        "status", "number", "repo", "author", "reviews", "comments", "age",
    ];
    let huge: BTreeMap<String, f32> = keys.iter().map(|k| ((*k).to_owned(), 700.0)).collect();
    let mut h = demo(GroupBy::None, WINDOW);
    h.state_mut().dash.view.widths.clone_from(&huge);
    for width in [1280.0, 1000.0, 1920.0, 720.0] {
        resize(&mut h, egui::vec2(width, WINDOW.y));
        let case = format!("saved widths at {width} pt");
        assert_fills(&h, &case);
        assert_inside(&h, width, &case);
    }
    assert_eq!(h.state().dash.view.widths, huge, "the saved widths stay");
}

#[test]
fn below_the_breakpoint_the_dashboard_lists_cards_with_a_sort_menu() {
    let at = breakpoint();
    assert!(
        (640.0..=700.0).contains(&at),
        "the breakpoint moved to {at} pt"
    );
    let window = at.ceil() + 2.0 * HARNESS_MARGIN;
    let sort = copy::menu_value(copy::SORT, copy::COLUMN_STATUS);
    let h = demo(GroupBy::Status, egui::vec2(window, WINDOW.y));
    assert!(
        h.query_by_role_and_label(Role::Button, copy::COLUMN_TITLE)
            .is_some()
    );
    assert!(h.query_by_label(&sort).is_none(), "the header sorts");
    let h = demo(GroupBy::Status, egui::vec2(window - 1.0, WINDOW.y));
    assert!(
        h.query_by_role_and_label(Role::Button, copy::COLUMN_TITLE)
            .is_none()
    );
    h.get_by_role_and_label(Role::ComboBox, &sort);
    h.get_by_label("Fix token refresh when the az session expires");
    assert_inside(&h, window - 1.0, "cards");
}

/// The toolbar's controls, left to right.
fn toolbar_controls(h: &Harness<'_, Fixture>) -> Vec<egui::Rect> {
    let search = h.get_by_role(Role::TextInput).rect();
    let group = copy::menu_value(copy::GROUP, "Status");
    let sort = copy::menu_value(copy::SORT, copy::COLUMN_STATUS);
    let mut rects = vec![search, h.get_by_label(&group).rect()];
    rects.extend(h.query_by_label(&sort).map(|n| n.rect()));
    for command in [CommandId::RefreshNow, CommandId::SettingsOpen] {
        rects.push(h.get_by_label(command.title()).rect());
    }
    rects.sort_by(|a, b| a.left().total_cmp(&b.left()));
    rects
}

#[test]
fn the_toolbar_fits_from_the_smallest_window_up() {
    let mut h = demo(GroupBy::Status, NARROW);
    let mut width = NARROW.x;
    while width <= WINDOW.x {
        resize(&mut h, egui::vec2(width, NARROW.y));
        let rects = toolbar_controls(&h);
        let apart = rects.windows(2).all(|p| p[0].right() <= p[1].left() + 0.5);
        let inside = rects.iter().all(|r| r.left() >= 0.0 && r.right() <= width);
        let one_line = rects
            .iter()
            .all(|r| (r.center().y - rects[0].center().y).abs() < 1.0);
        assert!(apart && inside && one_line, "at {width} pt: {rects:?}");
        width += 20.0;
    }
}

#[test]
fn in_the_smallest_window_group_and_sort_show_only_their_icons() {
    let h = demo(GroupBy::Status, NARROW);
    let theme = theme::fallback();
    for (label, value) in [(copy::GROUP, "Status"), (copy::SORT, copy::COLUMN_STATUS)] {
        let name = copy::menu_value(label, value);
        let button = h.get_by_role_and_label(Role::ComboBox, &name).rect();
        assert!(
            (button.width() - theme.control_height()).abs() < 0.5,
            "{name}: {button:?}"
        );
    }
    let search = h.get_by_role(Role::TextInput).rect();
    assert!(
        search.width() >= 100.0,
        "the filter keeps room to type: {search:?}"
    );
}

#[test]
fn a_narrow_window_shows_the_peek_over_the_list_with_a_close_button() {
    let mut fixture = Fixture::new(Shown::Dashboard, demo_state());
    fixture.now = now();
    fixture.dash.peek.open = true;
    if let Some(key) = fixture.state.rows.first().map(|r| r.key.clone()) {
        fixture.dash.view.select_key(&key);
    }
    let mut h = harness_at(fixture, egui::vec2(600.0, WINDOW.y));
    h.run();
    let close = h.get_by_role_and_label(Role::Button, copy::CLOSE).rect();
    assert!(
        close.right() > 560.0,
        "the peek spans the window: {close:?}"
    );
    assert!(
        h.query_by_label("Bump windows-rs to 0.62").is_none(),
        "the list is covered"
    );
    assert_inside(&h, 600.0, "peek over the list");
    resize(&mut h, WINDOW);
    h.get_by_label("Bump windows-rs to 0.62");
    h.get_by_role_and_label(Role::Button, copy::CLOSE);
}

#[test]
fn closing_the_peek_works_without_a_selection() {
    let mut fixture = Fixture::new(Shown::Dashboard, demo_state());
    fixture.dash.peek.open = true;
    fixture.dash.view.selected = None;
    let mut h = harness_at(fixture, WINDOW);
    h.run();
    h.get_by_role_and_label(Role::Button, copy::CLOSE).click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, crate::actions::UiAction::Run(CommandId::PrPeek))),
        "{actions:?}"
    );
}

#[test]
fn banners_wrap_and_keep_their_buttons_in_the_window() {
    for width in [NARROW.x, 480.0, 720.0, WINDOW.x] {
        let mut fixture = Fixture::new(Shown::Dashboard, demo_state());
        super::gallery::banner_shapes(&mut fixture);
        let mut h = harness_at(fixture, egui::vec2(width, WINDOW.y));
        h.run();
        for label in [
            "Sign in with Microsoft",
            "Choose how to sign in",
            copy::CLOSE,
        ] {
            let rect = h.get_by_label(label).rect();
            assert!(
                rect.left() >= 0.0 && rect.right() <= width,
                "{label} at {width} pt: {rect:?}"
            );
        }
        h.get_by_label("Can't reach Azure DevOps. Showing the last data.");
        assert_inside(&h, width, "banners");
    }
}

/// The hover bands were spaced a row's height apart, not its pitch (height plus spacing), so a
/// few rows down a move into the next row stayed in the old band and its hover lagged.
#[test]
fn the_hover_bands_follow_the_rows_as_drawn() {
    let theme = theme::fallback();
    let style = adapter::style(&theme, theme.policy);
    let pitch = theme.tokens.metrics.row.row_height + style.spacing.item_spacing.y;
    let milli = |pitch| TableGeometry::new([0.0; 4], 0.0, pitch, 1.0).pitch_milli;
    let table = demo(GroupBy::Status, WINDOW);
    let bands = table.state().dash.geometry.map(|g| g.pitch_milli);
    assert_eq!(bands, Some(milli(pitch)));
    let narrow = egui::vec2(480.0, WINDOW.y);
    let cards = demo(GroupBy::None, narrow);
    let card = cards.state().dash.view.pitch.pr;
    assert!(card > pitch, "a card is taller than a row");
    let bands = cards.state().dash.geometry.map(|g| g.pitch_milli);
    assert_eq!(bands, Some(milli(card)));
    let grouped = demo(GroupBy::Status, narrow);
    assert_eq!(
        grouped.state().dash.geometry,
        None,
        "cards and the shorter headers between them have no single pitch"
    );
}

/// Tab went from the filter to Settings, then Refresh, Sort and Group: the buttons were added
/// right to left, the reverse of how the toolbar reads.
#[test]
fn tab_moves_through_the_toolbar_in_reading_order() {
    let mut h = demo(GroupBy::Status, egui::vec2(480.0, WINDOW.y));
    h.get_by_role(Role::TextInput).focus();
    h.run();
    let names = [
        copy::menu_value(copy::GROUP, "Status"),
        copy::menu_value(copy::SORT, copy::COLUMN_STATUS),
        CommandId::RefreshNow.title().to_owned(),
        CommandId::SettingsOpen.title().to_owned(),
    ];
    for name in names {
        h.key_press(egui::Key::Tab);
        h.run();
        assert!(h.get_by_label(&name).is_focused(), "Tab reaches {name}");
    }
}

/// Where the peek's content starts: its "Open in browser" button sits at its left edge.
fn peek_left(h: &Harness<'_, Fixture>) -> f32 {
    h.get_by_label(copy::OPEN_IN_BROWSER).rect().left()
}

/// A narrow window cut the docked peek to fit and saved the cut width, so it stayed narrow once
/// the window was wide again.
#[test]
fn the_peek_keeps_its_saved_width_through_a_narrow_window() {
    let mut fixture = Fixture::new(Shown::Dashboard, demo_state());
    fixture.now = now();
    fixture.dash.peek.open = true;
    fixture.dash.peek.width = Some(600.0);
    if let Some(key) = fixture.state.rows.first().map(|r| r.key.clone()) {
        fixture.dash.view.select_key(&key);
    }
    let wide = egui::vec2(1600.0, WINDOW.y);
    let mut h = harness_at(fixture, wide);
    h.run();
    let left = peek_left(&h);
    resize(&mut h, egui::vec2(800.0, WINDOW.y));
    let cut = 800.0 - peek_left(&h);
    assert!(cut < 1600.0 - left - 100.0, "the peek fits the window");
    resize(&mut h, wide);
    assert!(
        (peek_left(&h) - left).abs() < 0.5,
        "the peek is as wide again"
    );
    assert_eq!(h.state().dash.peek.width, Some(600.0));
}

/// The first pull request's title and number, and the list's first group header.
fn first_card(h: &Harness<'_, Fixture>) -> (egui::Rect, egui::Rect, egui::Rect) {
    let view = &h.state().dash.view;
    let first = (0..view.display.rows.len()).find_map(|i| view.row_at(i));
    let (title, number) = first.map_or_else(Default::default, |r| {
        (r.title.to_string(), r.number_text.to_string())
    });
    let header = h.get_by_role_and_label(Role::Button, "Needs review").rect();
    (
        h.get_by_label(&title).rect(),
        h.get_by_label(&number).rect(),
        header,
    )
}

/// Banners wrap tall in a narrow window; expanded, they left the list a strip too short for a
/// card, and more of them would have pushed into the status bar.
#[test]
fn expanded_banners_leave_the_list_room_for_a_card() {
    for (size, last) in [(NARROW, "Sign in with Microsoft"), (WINDOW, copy::CLOSE)] {
        let mut fixture = Fixture::new(Shown::Dashboard, demo_state());
        super::gallery::banner_shapes(&mut fixture);
        fixture.dash.banners_expanded = true;
        let mut h = harness_at(fixture, size);
        h.run();
        let theme = theme::fallback();
        let summary = h.state().state.summary;
        let count = copy::status_count(summary.needs_review, prm_domain::PrStatus::NeedsReview);
        let status = h.get_by_label(&count).rect().center().y;
        let list_bottom = status - theme.tokens.metrics.row.statusbar_height / 2.0;
        let (title, number, header) = first_card(&h);
        assert!(
            title.top() >= header.bottom() && number.bottom() <= list_bottom,
            "at {size:?} the first card ({title:?}, {number:?}) is in the list ({header:?} to \
             {list_bottom})"
        );
        let button = h.get_by_label(last).rect();
        assert!(
            button.bottom() <= header.top(),
            "at {size:?} the banners show {last} ({button:?}) above the list ({header:?})"
        );
    }
}

/// The keyboard focus rings painted this frame, each with its clip.
fn focus_rings(h: &Harness<'_, Fixture>) -> Vec<(egui::Rect, egui::Rect)> {
    let theme = h.state().theme.current(&h.ctx).clone();
    let focus = theme.tokens.metrics.stroke.focus;
    h.output()
        .shapes
        .iter()
        .filter_map(|clipped| {
            let egui::Shape::Rect(ring) = &clipped.shape else {
                return None;
            };
            let stroke = ring.stroke;
            let focused =
                stroke.color == theme.colors.border_focus && (stroke.width - focus).abs() < 0.01;
            focused.then_some((ring.rect, clipped.clip_rect))
        })
        .collect()
}

/// The ring round the row chosen from the keyboard took the row's response, which reaches
/// past the list, so it ran off the window with no right side.
#[test]
fn the_focus_ring_stays_inside_the_list() {
    for width in [480.0, WINDOW.x] {
        let mut h = demo(GroupBy::None, egui::vec2(width, WINDOW.y));
        h.state_mut().dash.view.nav_edge(false);
        h.run();
        let rings = focus_rings(&h);
        let end = list_end(&h);
        assert_eq!(rings.len(), 1, "at {width} pt: {rings:?}");
        for (ring, clip) in rings {
            let case = format!("at {width} pt the ring {ring:?} (clip {clip:?})");
            assert!(
                ring.left() >= clip.left() && ring.right() <= clip.right(),
                "{case}"
            );
            assert!(ring.right() >= end - 1.5, "{case} stops short of {end}");
            assert!(
                ring.right() <= dashboard_right(&h),
                "{case} passes the window"
            );
        }
    }
}
