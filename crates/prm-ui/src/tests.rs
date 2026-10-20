//! Screens rendered headlessly with `egui_kittest`: what a user sees and what clicking does.
//! Accessibility checks are on and a missing glyph fails the test.

use std::sync::Arc;

use eframe::egui;
use egui_kittest::{Harness, kittest::Queryable as _};
use prm_config::UiState;
use prm_domain::{auth::SettingsSection, view::StatusFilter};
use prm_service::{AppState, AuthStateView, Command, UiError};

use crate::{
    actions::{Actions, UiAction},
    commands::{
        CommandId,
        keymap::Keymap,
        palette::{PaletteItem, PaletteSources, PaletteState},
    },
    copy,
    deps::BuildInfo,
    frame::FrameCx,
    screens::{
        dashboard::{DashboardUi, dashboard},
        first_run::{FirstRun, first_run},
        settings::{PAGES, Page, SettingsEnv, SettingsState, settings_screen},
    },
    testing::{org_status, sample_rows, state_with},
    theme::{ThemeRuntime, fonts},
    ui_state::PeekLayout,
    view::{DashboardView, changes::ChangeLog},
    widgets::palette_view::{PaletteOutcome, palette_view},
};

enum Shown {
    Dashboard,
    Settings,
    FirstRun,
    Palette,
}

struct Fixture {
    shown: Shown,
    state: AppState,
    dash: DashboardUi,
    settings: SettingsState,
    first_run: FirstRun,
    palette: PaletteState,
    palette_outcome: Option<PaletteItem>,
    keymap: Keymap,
    changes: ChangeLog,
    actions: Actions,
    theme: ThemeRuntime,
}

impl Fixture {
    fn new(shown: Shown, state: AppState) -> Self {
        let view = DashboardView::new(&UiState::default(), state.rows.clone(), 1, 24.0);
        let policy = crate::render::policy::RenderPolicy {
            low_power: false,
            animations: false,
        };
        let theme = ThemeRuntime::new(
            std::path::Path::new("no-such-themes-dir"),
            &state.config.ui,
            policy,
        );
        Self {
            shown,
            dash: DashboardUi::new(
                view,
                PeekLayout {
                    open: false,
                    width: None,
                },
            ),
            settings: SettingsState::new(state.config.clone(), SettingsSection::General),
            first_run: FirstRun::new(state.config.clone()),
            palette: PaletteState::default(),
            palette_outcome: None,
            keymap: Keymap::defaults(),
            changes: ChangeLog::default(),
            actions: Actions::default(),
            theme,
            state,
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.theme.apply(&ctx);
        self.theme.install_frame(&ctx);
        let mut cx = FrameCx {
            theme: self.theme.current(&ctx).clone(),
            state: &self.state,
            now: prm_domain::testing::ts(1_792_411_500),
            offset: time::UtcOffset::UTC,
            keymap: &self.keymap,
            changes: &self.changes,
            actions: &mut self.actions,
        };
        match self.shown {
            Shown::Dashboard => dashboard(ui, &mut cx, &mut self.dash, true),
            Shown::FirstRun => first_run(ui, &mut cx, &mut self.first_run),
            Shown::Settings => settings_screen(ui, &mut cx, &mut self.settings, &env()),
            Shown::Palette => {
                let rows = self.state.rows.clone();
                let sources = PaletteSources {
                    sign_in: Vec::new(),
                    themes: Vec::new(),
                    rows: &rows,
                };
                if let PaletteOutcome::Run(item) =
                    palette_view(&ctx, &cx, &mut self.palette, &sources)
                {
                    self.palette_outcome = Some(item);
                }
            }
        }
    }

    fn take_actions(&mut self) -> Vec<UiAction> {
        self.actions.take()
    }
}

fn env() -> SettingsEnv<'static> {
    SettingsEnv {
        themes: vec![
            ("graphite".to_owned(), "Graphite".to_owned()),
            ("vellum".to_owned(), "Vellum".to_owned()),
        ],
        themes_dir: std::path::PathBuf::from("themes"),
        theme_issues: vec!["midnight: text.muted fails contrast".to_owned()],
        build: BuildInfo {
            version: "0.1.0",
            git_sha: "0123456789ab",
            target: "x86_64-pc-windows-msvc",
        },
        licenses: "Third-party license text.",
        renderer_line: "Using OpenGL on Test GPU",
    }
}

fn harness(fixture: Fixture) -> Harness<'static, Fixture> {
    Harness::builder()
        .with_size(egui::vec2(1280.0, 760.0))
        .with_context_setup(fonts::install)
        .build_ui_state(|ui, f: &mut Fixture| f.draw(ui), fixture)
}

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
fn clicking_a_count_chip_asks_for_that_status_filter() {
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
            .any(|a| matches!(a, UiAction::Sort(prm_domain::view::SortKey::Title))),
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

#[test]
fn first_run_rejects_a_bad_url_and_connects_a_good_one() {
    let mut h = harness(Fixture::new(Shown::FirstRun, AppState::default()));
    h.run();
    h.get_by_label(copy::FIRST_RUN_HEADLINE);
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
