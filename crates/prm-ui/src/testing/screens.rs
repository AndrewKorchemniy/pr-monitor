//! One harness for every screen: the state a screen draws from, the theme, and the action
//! queue. Shared by the kittest tests and the screenshot gallery.

use eframe::egui;
use egui_kittest::Harness;
use prm_config::UiState;
use prm_domain::{Timestamp, auth::SettingsSection};
use prm_service::AppState;

use crate::{
    actions::{Actions, UiAction},
    commands::{
        keymap::Keymap,
        palette::{PaletteItem, PaletteSources, PaletteState},
    },
    deps::BuildInfo,
    frame::FrameCx,
    screens::{
        dashboard::{DashboardUi, dashboard},
        first_run::{FirstRun, first_run},
        settings::{SettingsEnv, SettingsState, settings_modal},
        shortcuts::shortcuts,
    },
    theme::{ThemeRuntime, fonts},
    ui_state::PeekLayout,
    view::{DashboardView, changes::ChangeLog},
    widgets::palette_view::{PaletteOutcome, palette_view},
};

/// The window size every screen is drawn at.
pub(crate) const WINDOW: egui::Vec2 = egui::vec2(1280.0, 760.0);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Shown {
    Dashboard,
    Settings,
    FirstRun,
    Palette,
    Shortcuts,
}

pub(crate) struct Fixture {
    pub shown: Shown,
    pub state: AppState,
    pub dash: DashboardUi,
    pub settings: SettingsState,
    pub first_run: FirstRun,
    pub palette: PaletteState,
    pub palette_outcome: Option<PaletteItem>,
    pub keymap: Keymap,
    pub changes: ChangeLog,
    pub actions: Actions,
    pub theme: ThemeRuntime,
    pub now: Timestamp,
}

impl Fixture {
    pub(crate) fn new(shown: Shown, state: AppState) -> Self {
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
        let peek = PeekLayout {
            open: false,
            width: None,
        };
        Self {
            shown,
            dash: DashboardUi::new(view, peek),
            settings: SettingsState::new(state.config.clone(), SettingsSection::General),
            first_run: FirstRun::new(state.config.clone()),
            palette: PaletteState::default(),
            palette_outcome: None,
            keymap: Keymap::defaults(),
            changes: ChangeLog::default(),
            actions: Actions::default(),
            theme,
            now: prm_domain::testing::ts(1_792_411_500),
            state,
        }
    }

    pub(crate) fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.theme.apply(&ctx);
        let mut cx = FrameCx {
            theme: self.theme.current(&ctx).clone(),
            state: &self.state,
            now: self.now,
            offset: time::UtcOffset::UTC,
            keymap: &self.keymap,
            changes: &self.changes,
            actions: &mut self.actions,
        };
        match self.shown {
            Shown::Dashboard => dashboard(ui, &mut cx, &mut self.dash, true),
            Shown::FirstRun => first_run(ui, &mut cx, &mut self.first_run),
            Shown::Settings => {
                dashboard(ui, &mut cx, &mut self.dash, false);
                settings_modal(&ctx, &mut cx, &mut self.settings, &env());
            }
            Shown::Palette => {
                dashboard(ui, &mut cx, &mut self.dash, false);
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
            Shown::Shortcuts => {
                dashboard(ui, &mut cx, &mut self.dash, false);
                let _ = shortcuts(&ctx, &cx);
            }
        }
    }

    pub(crate) fn take_actions(&mut self) -> Vec<UiAction> {
        self.actions.take()
    }
}

pub(crate) fn env() -> SettingsEnv<'static> {
    SettingsEnv {
        themes: vec![
            ("graphite".to_owned(), "Graphite".to_owned()),
            ("vellum".to_owned(), "Vellum".to_owned()),
            ("cyanotype".to_owned(), "Cyanotype".to_owned()),
            ("paper".to_owned(), "Paper".to_owned()),
        ],
        themes_dir: std::path::PathBuf::from("themes"),
        theme_issues: Vec::new(),
        build: BuildInfo {
            version: "0.2.0",
            git_sha: "0123456789ab",
            target: "x86_64-pc-windows-msvc",
        },
        licenses: "Third-party license text.",
        renderer_line: "Using OpenGL on Test GPU",
    }
}

/// A headless harness (accessibility checks on, missing glyphs fail).
pub(crate) fn harness(fixture: Fixture) -> Harness<'static, Fixture> {
    Harness::builder()
        .with_size(WINDOW)
        .with_context_setup(fonts::install)
        .build_ui_state(|ui, f: &mut Fixture| f.draw(ui), fixture)
}
