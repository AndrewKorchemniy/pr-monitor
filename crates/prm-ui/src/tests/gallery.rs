//! Screenshot gallery: every screen with the fictional demo data, in a dark and a light theme,
//! rendered on the GPU (or WARP) to PNG files for design review. Ignored by default:
//! `cargo test -p prm-ui gallery -- --ignored`, output in `$PRM_GALLERY_DIR` or
//! `target/gallery`.

use std::path::PathBuf;

use egui_kittest::Harness;
use prm_domain::view::StatusFilter;
use prm_service::AppState;

use crate::{
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
        scene(
            "dashboard-status-filter",
            Shown::Dashboard,
            demo_state,
            |f| {
                f.dash.view.set_status(StatusFilter::ActiveComments);
            },
        ),
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

fn render(scene: &Scene, theme: &str, dir: &std::path::Path) {
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
    for scene in scenes() {
        for theme in ["graphite", "vellum", "cyanotype", "paper"] {
            render(&scene, theme, &dir);
        }
    }
    println!("gallery written to {}", dir.display());
}
