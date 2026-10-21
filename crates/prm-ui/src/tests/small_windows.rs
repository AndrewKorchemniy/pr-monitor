//! Settings, its dialogs, first run, the palette and the shortcut sheet in phone-sized windows:
//! nothing runs past the window's sides, dialogs lie inside it, and the controls a user needs
//! (Close, the page menu, Save) are there and take a click.

use std::sync::Arc;

use eframe::egui::{self, Role, accesskit::Toggled, vec2};
use egui_kittest::{
    Harness,
    kittest::{AccessKitNode, NodeT as _, Queryable as _},
};
use prm_domain::auth::AuthMode;
use prm_service::{AppState, SearchPhase};

use crate::{
    actions::UiAction,
    copy,
    screens::{
        first_run::Step,
        settings::{Editor, PAGES, Page},
    },
    testing::{
        demo::{demo_state, empty_state},
        overflow::{assert_clickable, assert_overlays_fit, assert_screen_fits},
        screens::{Fixture, Shown, harness_at},
    },
};

/// A phone held upright, and a small tablet-sized window.
const SIZES: [egui::Vec2; 2] = [vec2(360.0, 640.0), vec2(480.0, 720.0)];

fn shown_at(
    shown: Shown,
    state: AppState,
    setup: impl FnOnce(&mut Fixture),
    size: egui::Vec2,
) -> Harness<'static, Fixture> {
    let mut fixture = Fixture::new(shown, state);
    setup(&mut fixture);
    let mut h = harness_at(fixture, size);
    h.run();
    h
}

fn settings_at(page: Page, size: egui::Vec2) -> Harness<'static, Fixture> {
    shown_at(
        Shown::Settings,
        demo_state(),
        |f| f.settings.page = page,
        size,
    )
}

/// The page menu that stands in for the sidebar: "Settings: General".
fn page_menu<'h>(h: &'h Harness<'_, Fixture>, page: Page) -> egui_kittest::Node<'h> {
    let name = format!("{}: {}", copy::SETTINGS, page.title());
    h.get_by(move |n| n.role() == Role::ComboBox && n.label().as_deref() == Some(name.as_str()))
}

/// Every Close button there is (settings' own, and a dialog's over it) can be clicked.
fn assert_close_buttons(h: &Harness<'_, Fixture>, size: egui::Vec2) {
    let closes: Vec<_> = h
        .query_all_by_role_and_label(Role::Button, copy::CLOSE)
        .collect();
    assert!(!closes.is_empty(), "{size:?}: no close button");
    for close in &closes {
        assert_clickable(close, size);
    }
}

#[test]
fn every_settings_page_fits_a_small_window() {
    for size in SIZES {
        for page in PAGES {
            let h = settings_at(page, size);
            assert_overlays_fit(&h, size);
            assert_clickable(&page_menu(&h, page), size);
            assert_close_buttons(&h, size);
            assert!(
                h.query_by_role_and_label(Role::Tab, page.title()).is_none(),
                "{size:?}: the sidebar folds away"
            );
        }
    }
}

/// The sidebar was the only way between pages, and it did not fit.
#[test]
fn the_page_menu_moves_between_pages() {
    for size in SIZES {
        let mut h = settings_at(Page::General, size);
        page_menu(&h, Page::General).click();
        h.run();
        let item = h.get_by_role_and_label(Role::MenuItem, Page::Appearance.title());
        assert_clickable(&item, size);
        assert_overlays_fit(&h, size);
        item.click();
        h.run();
        assert_eq!(h.state().settings.page, Page::Appearance, "{size:?}");
        page_menu(&h, Page::Appearance).focus();
        h.run();
        h.key_press(egui::Key::Enter);
        h.run();
        h.get_by_role_and_label(Role::MenuItem, Page::Notifications.title());
    }
}

#[test]
fn the_close_button_leaves_settings_in_a_small_window() {
    for size in SIZES {
        let mut h = settings_at(Page::Organizations, size);
        h.get_by_role_and_label(Role::Button, copy::CLOSE).click();
        h.run();
        let actions = h.state_mut().take_actions();
        assert!(
            actions.iter().any(|a| matches!(a, UiAction::LeaveSettings)),
            "{size:?}: {actions:?}"
        );
    }
}

/// The about links lived only in the sidebar's foot.
#[test]
fn the_about_links_end_the_general_page_when_the_sidebar_folds() {
    for size in SIZES {
        let mut h = settings_at(Page::General, size);
        h.get_by_label(copy::LICENSES_SHORT).scroll_to_me();
        h.run();
        let licenses = h.get_by_label(copy::LICENSES_SHORT);
        assert_clickable(&licenses, size);
        licenses.click();
        h.run();
        assert_eq!(
            h.state().settings.editing,
            Some(Editor::Licenses),
            "{size:?}"
        );
        h.get_by_label(copy::SOURCE_CODE);
        h.get_by_label(copy::REPORT_PROBLEM);
    }
}

#[test]
fn the_save_bar_fits_and_saves_in_a_small_window() {
    for size in SIZES {
        let mut h = settings_at(Page::General, size);
        h.get_by_label(copy::START_MINIMIZED).scroll_to_me();
        h.run();
        h.get_by_label(copy::START_MINIMIZED).click();
        h.run();
        assert!(h.state().settings.dirty(), "{size:?}");
        assert_overlays_fit(&h, size);
        assert_clickable(&h.get_by_label(copy::DISCARD), size);
        let save = h.get_by_label(copy::SAVE_CHANGES);
        assert_clickable(&save, size);
        save.click();
        h.run();
        let actions = h.state_mut().take_actions();
        assert!(
            actions.iter().any(|a| matches!(a, UiAction::SaveSettings)),
            "{size:?}: {actions:?}"
        );
    }
}

/// A save that was not written explains why: a long line that has to wrap above the button.
#[test]
fn a_failed_save_wraps_its_reason_above_the_button() {
    let reason = copy::changes_not_saved(copy::SAVE_FAILED);
    for size in SIZES {
        let failed = Arc::from(reason.as_str());
        let setup = |f: &mut Fixture| f.settings.save_error = Some(failed);
        let h = shown_at(Shown::Settings, demo_state(), setup, size);
        assert_overlays_fit(&h, size);
        let line = h.get_by_label(&reason).rect();
        let save = h.get_by_label(copy::SAVE_CHANGES);
        assert_clickable(&save, size);
        assert!(
            line.bottom() <= save.rect().top(),
            "{size:?}: {line:?} beside the button"
        );
    }
}

/// The connection with its Advanced fields open, for the organization's own app.
#[test]
fn the_sign_in_area_and_its_advanced_fields_fit() {
    for size in SIZES {
        let setup = |f: &mut Fixture| {
            f.settings.page = Page::Organizations;
            let org = &mut f.settings.draft.orgs[0];
            org.auth = AuthMode::Entra;
            "11111111-2222-3333-4444-555555555555".clone_into(&mut org.entra.client_id);
        };
        let mut h = shown_at(Shown::Settings, demo_state(), setup, size);
        assert_clickable(&h.get_by_label(copy::SIGN_OUT), size);
        assert_clickable(&h.get_by_label(copy::CHANGE_SIGN_IN), size);
        h.get_by_label(copy::ADVANCED).click();
        h.run();
        let client_id =
            |n: &AccessKitNode<'_>| n.data().placeholder() == Some(copy::ENTRA_CLIENT_ID);
        h.get_by(client_id).scroll_to_me();
        h.run();
        assert_overlays_fit(&h, size);
        assert_clickable(&h.get_by(client_id), size);
        assert_clickable(&h.get_by_label(copy::SIGN_IN_ENTRA), size);
    }
}

#[test]
fn the_notification_test_and_its_result_fit() {
    let failed = copy::test_failed("Notifications are turned off for PR Monitor in Windows");
    for size in SIZES {
        let result = Arc::from(failed.as_str());
        let setup = |f: &mut Fixture| {
            f.settings.page = Page::Notifications;
            f.settings.test_result = Some(Err(result));
        };
        let mut h = shown_at(Shown::Settings, demo_state(), setup, size);
        h.get_by_label(copy::SEND_TEST).scroll_to_me();
        h.run();
        assert_overlays_fit(&h, size);
        assert_clickable(&h.get_by_label(copy::SEND_TEST), size);
        assert_clickable(&h.get_by_label(&failed), size);
    }
}

#[test]
fn every_settings_dialog_fits_a_small_window() {
    let editors = [
        Editor::AddOrg,
        Editor::Licenses,
        Editor::Repositories,
        Editor::Tracked,
        Editor::Reviewers,
        Editor::Ignored,
    ];
    for size in SIZES {
        for editor in editors {
            let setup = |f: &mut Fixture| {
                f.settings.page = Page::Organizations;
                f.settings.editing = Some(editor);
            };
            let h = shown_at(Shown::Settings, demo_state(), setup, size);
            assert_overlays_fit(&h, size);
            assert_close_buttons(&h, size);
            if let Some(done) = h.query_by_role_and_label(Role::Button, copy::DONE) {
                assert_clickable(&done, size);
            }
        }
    }
}

/// The people dialog with its dropdown of matches open under the field.
#[test]
fn the_people_dropdown_fits_a_small_window() {
    for size in SIZES {
        let setup = |f: &mut Fixture| {
            f.settings.page = Page::Organizations;
            f.settings.editing = Some(Editor::Tracked);
            let found = super::gallery::matches();
            f.settings
                .pickers
                .tracked
                .show_matches("pri", &SearchPhase::Final, &found);
        };
        let h = shown_at(Shown::Settings, demo_state(), setup, size);
        assert_overlays_fit(&h, size);
        let row = h.get_by_role_and_label(Role::MenuItem, "Priyanka Das");
        assert_clickable(&row, size);
    }
}

#[test]
fn the_token_dialog_fits_a_small_window() {
    for size in SIZES {
        let setup = |f: &mut Fixture| f.token.1 = Some(Arc::from("The token was rejected"));
        let h = shown_at(Shown::Token, demo_state(), setup, size);
        assert_overlays_fit(&h, size);
        assert_clickable(&h.get_by_label(copy::SAVE_TOKEN), size);
        assert_clickable(&h.get_by_label(copy::CANCEL), size);
    }
}

fn first_run_at(step: Step, size: egui::Vec2) -> Harness<'static, Fixture> {
    let state = match step {
        Step::Connect => empty_state(),
        Step::Repositories | Step::People => demo_state(),
    };
    let setup = |f: &mut Fixture| {
        f.first_run.step = step;
        if step != Step::Connect {
            f.first_run.settings.draft.orgs = vec![f.state.config.orgs[0].clone()];
        }
    };
    shown_at(Shown::FirstRun, state, setup, size)
}

#[test]
fn every_first_run_step_fits_a_small_window() {
    let steps = [
        (Step::Connect, copy::CONNECT),
        (Step::Repositories, copy::CONTINUE),
        (Step::People, copy::START_MONITORING),
    ];
    for size in SIZES {
        for (step, next) in steps {
            let mut h = first_run_at(step, size);
            assert_screen_fits(&h, size);
            h.get_by_label(next).scroll_to_me();
            h.run();
            let next = h.get_by_label(next);
            assert_clickable(&next, size);
            // The card sits between even margins: its left inset mirrors its right one.
            let title = h.get_by_label(copy::APP_NAME).rect();
            let right = size.x - next.rect().right();
            assert!(
                (title.left() - right).abs() < 1.0,
                "{size:?} {step:?}: {} pt left, {right} pt right",
                title.left()
            );
            if step != Step::Connect {
                assert_clickable(&h.get_by_label(copy::PREVIOUS), size);
            }
        }
    }
}

#[test]
fn the_palette_fits_a_small_window() {
    for size in SIZES {
        for query in ["", "sign"] {
            let setup = |f: &mut Fixture| query.clone_into(&mut f.palette.query);
            let mut h = shown_at(Shown::Palette, demo_state(), setup, size);
            assert_overlays_fit(&h, size);
            // Moving past the last listed row keeps the highlight in view.
            for _ in 0..14 {
                h.key_press(egui::Key::ArrowDown);
            }
            h.run();
            assert_overlays_fit(&h, size);
            let lit: Vec<_> = h
                .query_all_by_role(Role::Dialog)
                .flat_map(|dialog| dialog.children_recursive().collect::<Vec<_>>())
                .filter(|n| {
                    let node = n.accesskit_node();
                    node.role() == Role::Button && node.toggled() == Some(Toggled::True)
                })
                .collect();
            assert_eq!(lit.len(), 1, "{size:?} {query:?}: one row is highlighted");
            assert_clickable(&lit[0], size);
        }
    }
}

#[test]
fn the_shortcut_sheet_fits_a_small_window() {
    for size in SIZES {
        let h = shown_at(Shown::Shortcuts, demo_state(), |_| {}, size);
        assert_overlays_fit(&h, size);
        assert_close_buttons(&h, size);
    }
}
