//! Screenshot gallery: every screen with the fictional demo data, in a dark and a light theme,
//! rendered on the GPU (or WARP) to PNG files for design review. Ignored by default:
//! `cargo test -p prm-ui gallery -- --ignored`, output in `$PRM_GALLERY_DIR` or
//! `target/gallery`.

use std::path::PathBuf;

use eframe::egui::Role;
use egui_kittest::{Harness, kittest::Queryable as _};
use prm_domain::view::{GroupBy, SortKey};
use prm_service::AppState;

use crate::{
    commands::CommandId,
    copy,
    screens::{
        first_run::Step,
        settings::{Editor, PAGES},
    },
    testing::{
        demo::{demo_state, empty_state, now},
        screens::{Fixture, Shown, WINDOW},
    },
    theme::fonts,
};

/// What the pointer does once a scene has settled (hover a control, open a menu).
type Pointer = fn(&mut Harness<'_, Fixture>);

/// Scenes that need the pointer: hover states, the column menu, and the settings column
/// checklist scrolled into view.
fn pointer_scenes() -> Vec<(Scene, Pointer)> {
    vec![
        (
            scene(
                "dashboard-header-hover",
                Shown::Dashboard,
                demo_state,
                |f| {
                    f.dash.view.set_group(GroupBy::None);
                    f.dash.view.sort_by(SortKey::Age);
                    f.dash.view.sort_by(SortKey::Age);
                },
            ),
            hover_repo_header as Pointer,
        ),
        (
            scene("dashboard-column-menu", Shown::Dashboard, demo_state, |f| {
                f.dash.view.hidden_columns.push("reviews".to_owned());
            }),
            open_column_menu as Pointer,
        ),
        (
            scene(
                "dashboard-toolbar-hover",
                Shown::Dashboard,
                demo_state,
                nothing,
            ),
            hover_refresh as Pointer,
        ),
        (
            scene(
                "settings-general-columns",
                Shown::Settings,
                demo_state,
                |f| {
                    f.settings.page = PAGES[0];
                    f.dash.view.hidden_columns.push("reviews".to_owned());
                },
            ),
            show_column_checklist as Pointer,
        ),
    ]
}

fn show_column_checklist(h: &mut Harness<'_, Fixture>) {
    h.get_by_role_and_label(Role::CheckBox, copy::COLUMN_FLAGS)
        .scroll_to_me();
}

fn hover(h: &Harness<'_, Fixture>, label: &str) {
    let at = h.get_by_label(label).rect().center();
    h.hover_at(at);
}

fn hover_repo_header(h: &mut Harness<'_, Fixture>) {
    hover(h, copy::COLUMN_REPO);
}

fn hover_refresh(h: &mut Harness<'_, Fixture>) {
    hover(h, CommandId::RefreshNow.title());
}

fn open_column_menu(h: &mut Harness<'_, Fixture>) {
    h.get_by_label(copy::COLUMN_AUTHOR).click_secondary();
}

/// One screen, set up from a fresh fixture.
struct Scene {
    name: &'static str,
    shown: Shown,
    state: fn() -> AppState,
    setup: fn(&mut Fixture),
}

const fn scene(
    name: &'static str,
    shown: Shown,
    state: fn() -> AppState,
    setup: fn(&mut Fixture),
) -> Scene {
    Scene {
        name,
        shown,
        state,
        setup,
    }
}

/// A scene's name and how it sets up the fixture.
type Named = (&'static str, fn(&mut Fixture));

fn nothing(_: &mut Fixture) {}

fn first_run_step(f: &mut Fixture, step: Step) {
    f.first_run.step = step;
    f.first_run.settings.draft.orgs = vec![f.state.config.orgs[0].clone()];
}

fn editing(f: &mut Fixture, editor: Editor) {
    f.settings.page = PAGES[1];
    f.settings.editing = Some(editor);
}

fn scenes() -> Vec<Scene> {
    let mut all = vec![
        scene("first-run-connect", Shown::FirstRun, empty_state, nothing),
        scene("first-run-repositories", Shown::FirstRun, demo_state, |f| {
            first_run_step(f, Step::Repositories);
        }),
        scene("first-run-people", Shown::FirstRun, demo_state, |f| {
            first_run_step(f, Step::People);
        }),
        scene("dashboard", Shown::Dashboard, demo_state, nothing),
        scene("dashboard-peek", Shown::Dashboard, demo_state, |f| {
            f.dash.peek.open = true;
            if let Some(key) = f.state.rows.first().map(|r| r.key.clone()) {
                f.dash.view.select_key(&key);
            }
        }),
        scene("dashboard-filtered", Shown::Dashboard, demo_state, |f| {
            f.dash.filter = "token".to_owned();
            f.dash.view.set_text("token");
        }),
        scene("dashboard-empty", Shown::Dashboard, empty_state, nothing),
        scene("palette", Shown::Palette, demo_state, |f| {
            f.palette.query = "sign".to_owned();
        }),
        scene("shortcuts", Shown::Shortcuts, demo_state, nothing),
    ];
    let pages: [Named; 4] = [
        ("settings-general", |f| f.settings.page = PAGES[0]),
        ("settings-organizations", |f| f.settings.page = PAGES[1]),
        ("settings-notifications", |f| f.settings.page = PAGES[2]),
        ("settings-appearance", |f| f.settings.page = PAGES[3]),
    ];
    all.extend(
        pages
            .into_iter()
            .map(|(name, setup)| scene(name, Shown::Settings, demo_state, setup)),
    );
    let dialogs: [Named; 3] = [
        ("settings-add-org", |f| editing(f, Editor::AddOrg)),
        ("settings-choose-repositories", |f| {
            editing(f, Editor::Repositories);
        }),
        ("settings-edit-people", |f| editing(f, Editor::Tracked)),
    ];
    all.extend(
        dialogs
            .into_iter()
            .map(|(name, setup)| scene(name, Shown::Settings, demo_state, setup)),
    );
    all
}

fn out_dir() -> PathBuf {
    std::env::var_os("PRM_GALLERY_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/gallery"),
        PathBuf::from,
    )
}

fn render(scene: &Scene, theme: &str, dir: &std::path::Path, pointer: Option<Pointer>) {
    let mut state = (scene.state)();
    let mut config = (*state.config).clone();
    config.ui.theme = theme.to_owned();
    state.config = std::sync::Arc::new(config);
    let mut fixture = Fixture::new(scene.shown, state);
    fixture.now = now();
    (scene.setup)(&mut fixture);
    let mut harness = Harness::builder()
        .with_size(WINDOW)
        .with_pixels_per_point(1.25)
        .with_accessibility_check(false) // the kittest tests enforce accessibility; this only renders
        .with_context_setup(fonts::install)
        .wgpu()
        .build_ui_state(|ui, f: &mut Fixture| f.draw(ui), fixture);
    harness.run();
    if let Some(pointer) = pointer {
        pointer(&mut harness);
        harness.run();
    }
    let image = harness
        .render()
        .unwrap_or_else(|e| panic!("{}: render failed: {e}", scene.name));
    let path = dir.join(format!("{}-{theme}.png", scene.name));
    image
        .save(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

#[test]
#[ignore = "renders on the GPU and writes PNG files; run on demand for design review"]
fn gallery() {
    let dir = out_dir();
    // why: scenes come and go; stale images from older runs would read as current.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("gallery dir");
    let still = scenes().into_iter().map(|scene| (scene, None));
    let pointed = pointer_scenes()
        .into_iter()
        .map(|(scene, pointer)| (scene, Some(pointer)));
    for (scene, pointer) in still.chain(pointed) {
        for theme in ["graphite", "vellum", "cyanotype", "paper"] {
            render(&scene, theme, &dir, pointer);
        }
    }
    println!("gallery written to {}", dir.display());
}
