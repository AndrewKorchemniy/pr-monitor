//! Getting around settings in a small window: settings opened for About shows it, Tab reaches
//! the page menu before Close, titles come before their close buttons, and the org tabs stay on
//! one line.

use eframe::egui::{self, Role};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use prm_domain::auth::SettingsSection;

use super::{SIZES, page_menu, settings_at, shown_at};
use crate::{
    copy,
    screens::settings::{Editor, Page, SettingsState, orgs::new_profile},
    testing::{
        demo::demo_state,
        overflow::{assert_clickable, assert_overlays_fit},
        screens::{Fixture, Shown},
    },
};

/// The About command opened General at its top, far above the about section that ends it.
#[test]
fn settings_opened_for_about_shows_the_about_section() {
    for size in SIZES {
        let setup = |f: &mut Fixture| {
            f.settings = SettingsState::new(f.state.config.clone(), SettingsSection::About);
        };
        let h = shown_at(Shown::Settings, demo_state(), setup, size);
        assert_eq!(h.state().settings.page, Page::General, "{size:?}");
        let header = page_menu(&h, Page::General).rect();
        for link in [
            copy::LICENSES_SHORT,
            copy::SOURCE_CODE,
            copy::REPORT_PROBLEM,
        ] {
            let node = h.get_by_label(link);
            assert_clickable(&h, &node, size);
            assert!(
                node.rect().top() >= header.bottom(),
                "{size:?}: {link} is under the header"
            );
        }
    }
}

/// Close was added first so a long title could not push it out, which put it first in Tab
/// order too, ahead of the page menu to its left.
#[test]
fn tab_reaches_the_page_menu_before_close() {
    for size in SIZES {
        let mut h = settings_at(Page::General, size);
        h.key_press(egui::Key::Tab);
        h.run();
        assert!(page_menu(&h, Page::General).is_focused(), "{size:?}");
    }
}

/// Screen readers met a dialog's Close button before its title.
#[test]
fn titles_come_before_their_close_buttons() {
    let editors = [
        Editor::AddOrg,
        Editor::Licenses,
        Editor::Repositories,
        Editor::Tracked,
    ];
    for editor in editors {
        let setup = |f: &mut Fixture| {
            f.settings.page = Page::Organizations;
            f.settings.editing = Some(editor);
        };
        let h = shown_at(Shown::Settings, demo_state(), setup, SIZES[0]);
        for dialog in h.query_all_by_role(Role::Dialog) {
            let first = dialog
                .children_recursive()
                .find(|n| n.accesskit_node().bounding_box().is_some())
                .expect("the dialog shows something");
            let label = first.accesskit_node().label();
            assert_ne!(label.as_deref(), Some(copy::CLOSE), "{editor:?}");
        }
    }
}

/// At 360 pt the tabs wrapped onto a second line, the selected tab's underline left floating
/// above the baseline.
#[test]
fn the_org_tabs_stay_on_one_line() {
    for size in SIZES {
        let h = settings_at(Page::Organizations, size);
        let line = h.get_by_role_and_label(Role::Tab, "contoso").rect();
        for (role, name) in [(Role::Tab, "fabrikam"), (Role::Button, copy::ADD_ORG)] {
            let node = h.get_by_role_and_label(role, name);
            assert_clickable(&h, &node, size);
            assert_eq!(node.rect().top(), line.top(), "{size:?}: {name}");
        }
    }
}

/// More orgs than fit scroll sideways, with the selected one (here the last) in view.
#[test]
fn many_org_tabs_scroll_to_the_selected_one() {
    for size in SIZES {
        let setup = |f: &mut Fixture| {
            f.settings.page = Page::Organizations;
            for org in ["northwind", "tailspin", "fourthcoffee", "woodgrove"] {
                f.settings.draft.orgs.extend(new_profile(org));
            }
            f.settings.org = f.settings.draft.orgs.len() - 1;
        };
        let h = shown_at(Shown::Settings, demo_state(), setup, size);
        assert_overlays_fit(&h, size);
        let selected = h.get_by_role_and_label(Role::Tab, "woodgrove");
        assert_clickable(&h, &selected, size);
        let first = h.get_by_role_and_label(Role::Tab, "contoso").rect();
        assert_eq!(selected.rect().top(), first.top(), "{size:?}: one line");
        // Add organization stays at the end of the line while the tabs scroll before it.
        let add = h.get_by_role_and_label(Role::Button, copy::ADD_ORG);
        assert_clickable(&h, &add, size);
        assert!(add.rect().left() >= selected.rect().right(), "{size:?}");
    }
}
