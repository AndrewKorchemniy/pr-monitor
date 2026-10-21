//! The overlays and first run in small windows: a phone-sized one (360 pt wide), one 600 pt
//! wide, and the shortest the app allows. Settings folds its sidebar into a page menu there, rows
//! stack, and dialogs fit.

use std::sync::Arc;

use eframe::egui::{self, Role, vec2};
use egui_kittest::{
    Harness,
    kittest::{AccessKitNode, Queryable as _},
};
use prm_domain::auth::{AuthMode, SettingsSection};
use prm_service::{AppState, SearchPhase};

use super::{Pointer, Scene, editing, first_run_step, matches, nothing, scene};
use crate::{
    copy,
    screens::{
        first_run::Step,
        settings::{Editor, PAGES, SettingsState, orgs::new_profile},
    },
    testing::{
        demo::{demo_state, empty_state},
        screens::{Fixture, Shown},
    },
};

/// A phone held upright.
const PHONE: egui::Vec2 = vec2(360.0, 640.0);
/// A small window, still too narrow for the settings sidebar.
const SMALL: egui::Vec2 = vec2(600.0, 800.0);
/// The shortest window the app allows, at phone width.
const SHORT: egui::Vec2 = vec2(360.0, 420.0);

/// A scene's name at each size.
type Names = (&'static str, &'static str);
/// A scene's names, and how it is set up.
type Pair = (Names, Shown, fn() -> AppState, fn(&mut Fixture));
/// A settings scene's names, its setup and what the pointer does.
type Pointed = (Names, fn(&mut Fixture), Pointer);

/// The four pages, General opened for About, more orgs than fit on a line, and the Save bar.
fn settings_pages() -> [Pair; 8] {
    let settings = Shown::Settings;
    [
        (
            ("settings-general-360", "settings-general-600"),
            settings,
            demo_state,
            |f| f.settings.page = PAGES[0],
        ),
        (
            ("settings-organizations-360", "settings-organizations-600"),
            settings,
            demo_state,
            |f| f.settings.page = PAGES[1],
        ),
        (
            ("settings-about-360", "settings-about-600"),
            settings,
            demo_state,
            opened_for_about,
        ),
        (
            ("settings-many-orgs-360", "settings-many-orgs-600"),
            settings,
            demo_state,
            many_orgs,
        ),
        (
            ("settings-notifications-360", "settings-notifications-600"),
            settings,
            demo_state,
            |f| {
                f.settings.page = PAGES[2];
                let failed = copy::test_failed("Notifications are off for PR Monitor in Windows");
                f.settings.test_result = Some(Err(Arc::from(failed)));
                f.settings.reveal_test = true;
            },
        ),
        (
            ("settings-appearance-360", "settings-appearance-600"),
            settings,
            demo_state,
            |f| f.settings.page = PAGES[3],
        ),
        (
            ("settings-unsaved-360", "settings-unsaved-600"),
            settings,
            demo_state,
            |f| f.settings.draft.ui.start_minimized = !f.settings.draft.ui.start_minimized,
        ),
        (
            ("settings-save-failed-360", "settings-save-failed-600"),
            settings,
            demo_state,
            |f| {
                let reason = copy::changes_not_saved(copy::SAVE_FAILED);
                f.settings.save_error = Some(Arc::from(reason));
            },
        ),
    ]
}

/// The settings dialogs.
fn settings_dialogs() -> [Pair; 4] {
    [
        (
            (
                "settings-edit-people-matches-360",
                "settings-edit-people-matches-600",
            ),
            Shown::Settings,
            demo_state,
            |f| {
                editing(f, Editor::Tracked);
                let found = matches();
                let phase = SearchPhase::Final;
                f.settings
                    .pickers
                    .tracked
                    .show_matches("pri", &phase, &found);
            },
        ),
        (
            (
                "settings-choose-repositories-360",
                "settings-choose-repositories-600",
            ),
            Shown::Settings,
            demo_state,
            |f| editing(f, Editor::Repositories),
        ),
        (
            ("settings-add-org-360", "settings-add-org-600"),
            Shown::Settings,
            demo_state,
            |f| editing(f, Editor::AddOrg),
        ),
        (
            ("settings-licenses-360", "settings-licenses-600"),
            Shown::Settings,
            demo_state,
            |f| editing(f, Editor::Licenses),
        ),
    ]
}

/// The token dialog, first run, the palette and the shortcut sheet.
fn other_pairs() -> [Pair; 7] {
    [
        (("token-360", "token-600"), Shown::Token, demo_state, |f| {
            f.token.1 = Some(Arc::from("The token was rejected: it has expired"));
        }),
        (
            ("first-run-connect-360", "first-run-connect-600"),
            Shown::FirstRun,
            empty_state,
            nothing,
        ),
        (
            ("first-run-repositories-360", "first-run-repositories-600"),
            Shown::FirstRun,
            demo_state,
            |f| first_run_step(f, Step::Repositories),
        ),
        (
            ("first-run-people-360", "first-run-people-600"),
            Shown::FirstRun,
            demo_state,
            |f| first_run_step(f, Step::People),
        ),
        (
            ("palette-360", "palette-600"),
            Shown::Palette,
            demo_state,
            |f| f.palette.query = "sign".to_owned(),
        ),
        (
            ("palette-all-360", "palette-all-600"),
            Shown::Palette,
            demo_state,
            nothing,
        ),
        (
            ("shortcuts-360", "shortcuts-600"),
            Shown::Shortcuts,
            demo_state,
            nothing,
        ),
    ]
}

/// Every pair at both sizes, then the repositories dialog in the shortest window, searched so
/// its list is longer than the dialog.
pub(super) fn scenes() -> Vec<Scene> {
    let mut all: Vec<Scene> = settings_pages()
        .into_iter()
        .chain(settings_dialogs())
        .chain(other_pairs())
        .flat_map(|((phone, small), shown, state, setup)| {
            [
                scene(phone, shown, state, setup).at(PHONE),
                scene(small, shown, state, setup).at(SMALL),
            ]
        })
        .collect();
    let searched = |f: &mut Fixture| {
        editing(f, Editor::Repositories);
        "e".clone_into(&mut f.settings.scope_filter.text);
    };
    all.push(
        scene(
            "settings-choose-repositories-short",
            Shown::Settings,
            demo_state,
            searched,
        )
        .at(SHORT),
    );
    all
}

/// Settings as the About command opens it.
fn opened_for_about(f: &mut Fixture) {
    let config = f.state.config.clone();
    f.settings = SettingsState::new(config, SettingsSection::About);
}

/// Organizations with six orgs, the last selected: more tabs than fit on one line.
fn many_orgs(f: &mut Fixture) {
    f.settings.page = PAGES[1];
    for org in ["northwind", "tailspin", "fourthcoffee", "woodgrove"] {
        f.settings.draft.orgs.extend(new_profile(org));
    }
    f.settings.org = f.settings.draft.orgs.len() - 1;
}

/// The page menu open, the about links at the end of General, and the Advanced sign-in fields.
pub(super) fn pointer_scenes() -> Vec<(Scene, Pointer)> {
    let entra = |f: &mut Fixture| {
        f.settings.page = PAGES[1];
        f.settings.draft.orgs[0].auth = AuthMode::Entra;
    };
    let general = |f: &mut Fixture| f.settings.page = PAGES[0];
    let pairs: [Pointed; 3] = [
        (
            ("settings-page-menu-360", "settings-page-menu-600"),
            general,
            open_page_menu,
        ),
        (
            ("settings-general-about-360", "settings-general-about-600"),
            general,
            show_about,
        ),
        (
            (
                "settings-sign-in-advanced-360",
                "settings-sign-in-advanced-600",
            ),
            entra,
            open_advanced,
        ),
    ];
    pairs
        .into_iter()
        .flat_map(|((phone, small), setup, pointer)| {
            [
                (
                    scene(phone, Shown::Settings, demo_state, setup).at(PHONE),
                    pointer,
                ),
                (
                    scene(small, Shown::Settings, demo_state, setup).at(SMALL),
                    pointer,
                ),
            ]
        })
        .collect()
}

fn open_page_menu(h: &mut Harness<'_, Fixture>) {
    let name = format!("{}: {}", copy::SETTINGS, PAGES[0].title());
    h.get_by_role_and_label(Role::ComboBox, &name).click();
}

fn show_about(h: &mut Harness<'_, Fixture>) {
    h.get_by_label(copy::REPORT_PROBLEM).scroll_to_me();
}

fn open_advanced(h: &mut Harness<'_, Fixture>) {
    h.get_by_label(copy::ADVANCED).click();
    h.run();
    let client_id = |n: &AccessKitNode<'_>| n.data().placeholder() == Some(copy::ENTRA_CLIENT_ID);
    h.get_by(client_id).scroll_to_me();
}
