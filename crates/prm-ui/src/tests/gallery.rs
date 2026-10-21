//! Screenshot gallery: every screen with the fictional demo data, in a dark and a light theme,
//! rendered on the GPU (or WARP) to PNG files for design review. Ignored by default:
//! `cargo test -p prm-ui gallery -- --ignored`, output in `$PRM_GALLERY_DIR` or
//! `target/gallery`.

use std::{path::PathBuf, sync::Arc};

use egui_kittest::Harness;
use prm_config::Severity;
use prm_domain::{
    Person,
    auth::{SettingsSection, UserAction},
    person::{CandidateSource, PersonCandidate},
    testing::{org, person},
};
use prm_service::{AppState, SearchPhase, UiError, state::Banner};

use crate::{
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
    widgets::people_picker::Step as Highlight,
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

/// Fictional matches for "pri": one person seen in pull requests, two from the directory.
fn matches() -> [PersonCandidate; 3] {
    let directory = |descriptor: &str, name: &str, unique: &str| PersonCandidate {
        id: None,
        descriptor: Some(Arc::from(descriptor)),
        name: Arc::from(name),
        unique_name: Some(Arc::from(unique)),
        group: false,
        source: CandidateSource::Directory,
    };
    let harvested = Person {
        unique_name: Some(Arc::from("prisha.nair@contoso.com")),
        ..person(11, "Prisha Nair")
    };
    [
        harvested.to_candidate(CandidateSource::Harvested),
        directory("aad.pd", "Priyanka Das", "priyanka.das@contoso.com"),
        directory("aad.pw", "Prince Walker", "prince.walker@contoso.com"),
    ]
}

/// The tracked-people dialog with its dropdown open on `phase`.
fn people_search(f: &mut Fixture, phase: &SearchPhase, found: &[PersonCandidate]) {
    editing(f, Editor::Tracked);
    f.settings.pickers.tracked.show_matches("pri", phase, found);
}

fn people_scenes() -> [Named; 5] {
    [
        ("settings-organizations-people", |f| {
            f.settings.page = PAGES[1];
            f.settings.scroll_to = Some(SettingsSection::People);
        }),
        ("settings-edit-people-matches", |f| {
            people_search(f, &SearchPhase::Final, &matches());
            f.settings.pickers.tracked.step(Highlight::Down, 3);
        }),
        ("settings-edit-people-searching", |f| {
            people_search(f, &SearchPhase::Local, &matches()[..1]);
        }),
        ("settings-edit-people-failed", |f| {
            let failed = UiError::new("Couldn't search the directory; showing known people only");
            people_search(f, &SearchPhase::Failed(failed), &matches()[..1]);
        }),
        ("settings-edit-people-signed-out", |f| {
            people_search(f, &SearchPhase::LocalOnly, &[]);
        }),
    ]
}

/// Banners of every shape: one line, a title with a detail and actions, and a dismissible one.
fn banner_shapes(f: &mut Fixture) {
    let plain = |id: &str, severity, title: &str| Banner {
        id: Arc::from(id),
        severity,
        org: None,
        title: Arc::from(title),
        detail: None,
        action: None,
        secondary: None,
        dismissible: false,
    };
    let sign_in = Banner {
        org: Some(org("contoso")),
        detail: Some(Arc::from("Your sign-in expired.")),
        action: Some(UserAction::SignInWithMicrosoft),
        secondary: Some(UserAction::ChooseSignIn),
        ..plain("auth:contoso", Severity::Error, "Sign in to contoso")
    };
    let offline = Banner {
        dismissible: true,
        ..plain(
            "offline",
            Severity::Warning,
            "Can't reach Azure DevOps. Showing the last data.",
        )
    };
    let unsaved = plain(
        "state-not-saved",
        Severity::Warning,
        "Couldn't save PR Monitor's data. Changes may be lost on exit.",
    );
    f.state.banners = Arc::from([sign_in, offline, unsaved]);
    f.dash.banners_expanded = true;
}

/// What used to be a passing message, now shown where the user is looking.
fn failure_scenes() -> [Named; 2] {
    [
        ("settings-save-failed", |f| {
            let reason = copy::changes_not_saved(copy::SAVE_FAILED);
            f.settings.save_error = Some(Arc::from(reason));
        }),
        ("settings-choose-repositories-failed", |f| {
            let mut orgs = f.state.orgs.to_vec();
            orgs[0].catalog = None;
            orgs[0].catalog_error = Some(UiError::new("Can't reach Azure DevOps"));
            f.state.orgs = orgs.into();
            editing(f, Editor::Repositories);
        }),
    ]
}

/// The stopped service (on the dashboard and during first run) and published banners.
fn banner_scenes() -> [Scene; 3] {
    let stopped = |f: &mut Fixture| f.dash.show_service_stopped();
    [
        scene(
            "dashboard-service-stopped",
            Shown::Dashboard,
            demo_state,
            stopped,
        ),
        scene(
            "dashboard-banners",
            Shown::Dashboard,
            demo_state,
            banner_shapes,
        ),
        scene(
            "first-run-service-stopped",
            Shown::FirstRun,
            empty_state,
            stopped,
        ),
    ]
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
    all.extend(banner_scenes());
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
            .chain(people_scenes())
            .chain(failure_scenes())
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
    state.config = Arc::new(config);
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
