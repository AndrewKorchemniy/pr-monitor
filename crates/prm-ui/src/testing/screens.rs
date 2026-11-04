//! One harness for every screen: the state a screen draws from, the theme, and the action
//! queue. Shared by the kittest tests and the screenshot gallery.

use std::sync::Arc;

use eframe::egui;
use egui_kittest::Harness;
use prm_config::UiState;
use prm_domain::{Timestamp, auth::SettingsSection};
use prm_service::{AppState, LoadedAvatar};

use crate::{
    actions::{Actions, UiAction},
    avatars::AvatarStore,
    commands::{
        keymap::Keymap,
        palette::{PaletteItem, PaletteSources, PaletteState},
    },
    deps::BuildInfo,
    frame::FrameCx,
    render::MIN_SIZE,
    screens::{
        dashboard::{DashboardUi, dashboard},
        first_run::{FirstRun, first_run},
        settings::{SettingsEnv, SettingsState, settings_modal},
        shortcuts::shortcuts,
        token::token_dialog,
    },
    testing::demo::demo_avatars,
    theme::{ThemeRuntime, fonts},
    ui_state::PeekLayout,
    view::{
        DashboardView,
        changes::ChangeLog,
        columns::{Choices, Picks},
    },
    widgets::{
        modal::ModalOutcome,
        palette_view::{PaletteOutcome, palette_view},
    },
};

/// The window size every screen is drawn at.
pub(crate) const WINDOW: egui::Vec2 = egui::vec2(1280.0, 760.0);
/// The smallest window the app allows.
pub(crate) const NARROW: egui::Vec2 = egui::vec2(MIN_SIZE[0], MIN_SIZE[1]);
/// `egui_kittest` draws the ui inside an outer margin this wide on every side (points).
pub(crate) const HARNESS_MARGIN: f32 = 8.0;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Shown {
    Dashboard,
    Settings,
    FirstRun,
    Palette,
    Shortcuts,
    /// The personal access token dialog over the dashboard.
    Token,
}

pub(crate) struct Fixture {
    pub shown: Shown,
    pub state: AppState,
    pub dash: DashboardUi,
    pub settings: SettingsState,
    pub first_run: FirstRun,
    pub palette: PaletteState,
    pub palette_outcome: Option<PaletteItem>,
    /// The token dialog's text and its last error.
    pub token: (String, Option<Arc<str>>),
    /// Whether the token dialog was dismissed (Esc or the close button).
    pub token_dismissed: bool,
    pub keymap: Keymap,
    pub changes: ChangeLog,
    /// Pictures loaded into the fixture; empty shows initials everywhere.
    pub avatars: AvatarStore,
    /// Pictures still to upload on the first frame (they need the egui context).
    pub pictures: Vec<LoadedAvatar>,
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
            token: (String::new(), None),
            token_dismissed: false,
            keymap: Keymap::defaults(),
            changes: ChangeLog::default(),
            avatars: AvatarStore::default(),
            pictures: demo_avatars(),
            actions: Actions::default(),
            theme,
            now: prm_domain::testing::ts(1_792_411_500),
            state,
        }
    }

    pub(crate) fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let draft = (self.shown == Shown::Settings).then_some(self.settings.draft.ui.corners);
        self.theme.set_corners_preview(draft);
        self.theme.apply(&ctx);
        for picture in self.pictures.drain(..) {
            self.avatars.insert(&ctx, &picture);
        }
        let mut cx = FrameCx {
            theme: self.theme.current(&ctx).clone(),
            state: &self.state,
            avatars: &self.avatars,
            now: self.now,
            offset: time::UtcOffset::UTC,
            keymap: &self.keymap,
            changes: &self.changes,
            actions: &mut self.actions,
        };
        match self.shown {
            Shown::Dashboard => dashboard(ui, &mut cx, &mut self.dash, true),
            Shown::FirstRun => {
                let stopped = self.dash.service_stopped.as_ref();
                first_run(ui, &mut cx, &mut self.first_run, stopped);
            }
            Shown::Settings => {
                dashboard(ui, &mut cx, &mut self.dash, false);
                let env = SettingsEnv {
                    columns: self.dash.view.column_choices(),
                    ..env()
                };
                settings_modal(&ctx, &mut cx, &mut self.settings, &env);
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
            Shown::Token => {
                dashboard(ui, &mut cx, &mut self.dash, false);
                let (input, error) = &mut self.token;
                let (outcome, _) =
                    token_dialog(&ctx, &cx.theme, "contoso", (input, error.as_deref()));
                self.token_dismissed |= outcome == ModalOutcome::Dismissed;
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
            ("andromeda".to_owned(), "Andromeda".to_owned()),
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
        columns: Choices {
            picks: Picks::default(),
            squeezed: &[],
        },
    }
}

/// A headless harness (accessibility checks on, missing glyphs fail).
pub(crate) fn harness(fixture: Fixture) -> Harness<'static, Fixture> {
    harness_at(fixture, WINDOW)
}

/// A headless harness with the window at `size`.
pub(crate) fn harness_at(fixture: Fixture, size: egui::Vec2) -> Harness<'static, Fixture> {
    Harness::builder()
        .with_size(size)
        .with_context_setup(fonts::install)
        .build_ui_state(|ui, f: &mut Fixture| f.draw(ui), fixture)
}
