//! Screens rendered headlessly with `egui_kittest`: what a user sees and what clicking does.
//! Accessibility checks are on and a missing glyph fails the test.

use std::sync::Arc;

use eframe::egui::{self, Role, accesskit::Toggled};
use egui_kittest::{
    Harness,
    kittest::{NodeT as _, Queryable as _},
};
use prm_domain::view::{GroupBy, SortKey, StatusFilter};
use prm_service::{AppState, AuthStateView, Command, UiError};

use crate::{
    actions::UiAction,
    commands::{CommandId, palette::PaletteItem},
    copy,
    screens::settings::{PAGES, Page},
    testing::{
        demo::demo_state,
        org_status, sample_rows,
        screens::{Fixture, Shown, WINDOW, harness},
        state_with,
    },
    view::{ScrollIntent, columns::ColumnId},
};

mod gallery;
mod people;
mod status_bar;
mod tracking;

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
fn clicking_a_status_count_asks_for_that_status_filter() {
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
fn status_counts_report_which_filter_is_on() {
    let mut fixture = Fixture::new(Shown::Dashboard, state_with(sample_rows()));
    fixture.dash.view.set_status(StatusFilter::ActiveComments);
    let mut h = harness(fixture);
    h.run();
    for (label, on) in [
        ("2 Needs review", false),
        ("1 Active comments", true),
        ("1 Reviewed", false),
    ] {
        let toggled = h.get_by_label(label).accesskit_node().toggled();
        let expected = if on { Toggled::True } else { Toggled::False };
        assert_eq!(toggled, Some(expected), "{label}");
    }
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

/// A save that was applied but not written used to show only as a passing message.
#[test]
fn a_save_that_was_not_written_keeps_the_save_bar_with_the_reason() {
    let mut fixture = Fixture::new(Shown::Settings, signed_out());
    let reason = copy::changes_not_saved(copy::SAVE_FAILED);
    fixture.settings.save_error = Some(Arc::from(reason.as_str()));
    let mut h = harness(fixture);
    h.run();

    h.get_by_label(&reason);
    assert!(
        h.query_by_label(copy::DISCARD).is_none(),
        "nothing to discard"
    );
    h.get_by_label(copy::SAVE_CHANGES).click();
    h.run();

    let actions = h.state_mut().take_actions();
    assert!(
        actions.iter().any(|a| matches!(a, UiAction::SaveSettings)),
        "{actions:?}"
    );
}

#[test]
fn the_test_notification_result_shows_beside_its_button() {
    let mut fixture = Fixture::new(Shown::Settings, signed_out());
    fixture.settings.page = Page::Notifications;
    let failed = copy::test_failed("Notifications are off");
    fixture.settings.test_result = Some(Err(Arc::from(failed.as_str())));
    let mut h = harness(fixture);
    h.run();
    h.get_by_label(&failed);

    // why: the test button sits at the bottom of the page, below the fold.
    h.get_by_label(copy::SEND_TEST).scroll_to_me();
    h.run();
    h.get_by_label(copy::SEND_TEST).click();
    h.run();
    assert!(
        h.query_by_label(&failed).is_none(),
        "a new test clears the result"
    );
    h.state_mut().settings.test_result = Some(Ok(()));
    h.run();
    h.get_by_label(copy::TEST_SENT);
}

/// "Send a test notification" from the palette opens this page scrolled to the button, so the
/// result shows without scrolling.
#[test]
fn a_test_sent_from_the_palette_shows_its_button() {
    let mut fixture = Fixture::new(Shown::Settings, signed_out());
    fixture.settings.page = Page::Notifications;
    fixture.settings.reveal_test = true;
    let mut h = harness(fixture);
    h.run();
    h.run();

    let button = h.get_by_label(copy::SEND_TEST).rect();
    let window = egui::Rect::from_min_size(egui::Pos2::ZERO, WINDOW);
    assert!(window.contains_rect(button), "{button:?} is off screen");
    assert!(!h.state().settings.reveal_test, "it scrolls once");
}

/// A failed project list used to leave the picker saying it was loading, forever.
#[test]
fn a_failed_project_list_says_so_and_offers_a_reload() {
    let mut state = demo_state();
    let mut orgs = state.orgs.to_vec();
    orgs[0].catalog = None;
    orgs[0].catalog_error = Some(UiError::new("Can't reach Azure DevOps"));
    state.orgs = orgs.into();
    let mut fixture = Fixture::new(Shown::Settings, state);
    fixture.settings.page = Page::Organizations;
    fixture.settings.editing = Some(crate::screens::settings::Editor::Repositories);
    let mut h = harness(fixture);
    h.run();

    assert_eq!(h.query_all_by_label(copy::PROJECTS_FAILED).count(), 2);
    assert!(h.query_by_label(copy::LOADING_PROJECTS).is_none());
    h.get_by_label(copy::REFRESH_LIST).click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, UiAction::Send(Command::LoadCatalog { force: true, .. }))),
        "{actions:?}"
    );
}

#[test]
fn a_stopped_service_shows_a_banner_on_the_dashboard() {
    let mut fixture = Fixture::new(Shown::Dashboard, state_with(sample_rows()));
    fixture.dash.show_service_stopped();
    let mut h = harness(fixture);
    h.run();
    h.get_by_label(copy::SERVICE_STOPPED);
    assert!(
        h.query_by_label(copy::CLOSE).is_none(),
        "it cannot be dismissed"
    );
}

/// Setup cannot finish without the service, so first run says so too.
#[test]
fn a_stopped_service_shows_a_banner_during_first_run() {
    let mut fixture = Fixture::new(Shown::FirstRun, AppState::default());
    fixture.dash.show_service_stopped();
    let mut h = harness(fixture);
    h.run();
    h.get_by_label(copy::SERVICE_STOPPED);
    h.get_by_label(copy::APP_NAME);
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
    let repo = h.get_by_role_and_label(Role::MenuItemCheckBox, copy::COLUMN_REPO);
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
