//! Appearance: theme (previewed live), text size, animations, low power, renderer,
//! and helpers for writing your own theme.

use eframe::egui;
use prm_config::{
    Config, UiPrefs, limits,
    schema::{RendererPref, Tristate},
};
use prm_theme::{Corners, IssueLevel, registry::SYSTEM};

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    screens::settings::SettingsEnv,
    widgets::{
        card::section,
        controls::{self, setting_row, toggle_row},
        slider::{SLIDER_WIDTH, SliderSpec, slider_field},
    },
};

const TRISTATE: [(Tristate, &str); 3] = [
    (Tristate::Auto, "Automatic"),
    (Tristate::On, "On"),
    (Tristate::Off, "Off"),
];
const CORNER_SHAPES: [(Corners, &str); 2] = [
    (Corners::Rounded, copy::ROUNDED),
    (Corners::Square, copy::SQUARE),
];
const RENDERERS: [(RendererPref, &str); 3] = [
    (RendererPref::Auto, "Automatic"),
    (RendererPref::Glow, "OpenGL"),
    (RendererPref::Wgpu, "Direct3D"),
];

fn theme_name<'a>(env: &'a SettingsEnv<'_>, id: &'a str) -> &'a str {
    env.themes
        .iter()
        .find(|(i, _)| i == id)
        .map_or(id, |(_, name)| name.as_str())
}

/// Corners and striped rows. The next frame's `logic` previews the draft's choices, so there is
/// nothing to send.
fn look_rows(ui: &mut egui::Ui, theme: &crate::theme::UiTheme, prefs: &mut UiPrefs) {
    controls::row_divider(ui, theme);
    setting_row(ui, theme, (copy::CORNERS, Some(copy::CORNERS_HELP)), |ui| {
        if controls::segmented(ui, theme, &mut prefs.corners, &CORNER_SHAPES) {
            ui.ctx().request_repaint();
        }
    });
    controls::row_divider(ui, theme);
    toggle_row(
        ui,
        theme,
        &mut prefs.striped_rows,
        (copy::STRIPED_ROWS, Some(copy::STRIPED_ROWS_HELP)),
    );
}

pub(crate) fn page(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    draft: &mut Config,
    env: &SettingsEnv<'_>,
) {
    let theme = cx.theme.clone();
    let prefs = &mut draft.ui;
    section(ui, &theme, copy::THEME, None, |ui| {
        let before = prefs.theme.clone();
        let system = copy::match_windows(
            theme_name(env, &prefs.theme_dark),
            theme_name(env, &prefs.theme_light),
        );
        controls::radio(ui, &theme, &mut prefs.theme, SYSTEM.to_owned(), system);
        for (id, name) in &env.themes {
            controls::radio(ui, &theme, &mut prefs.theme, id.clone(), name.as_str());
        }
        if prefs.theme != before {
            cx.push(UiAction::PreviewTheme(Some(prefs.theme.clone())));
        }
        look_rows(ui, &theme, prefs);
        issues(ui, &theme, &env.theme_issues);
    });
    section(ui, &theme, copy::SECTION_LAYOUT, None, |ui| {
        setting_row(ui, &theme, (copy::TEXT_SIZE, None), |ui| {
            let spec = SliderSpec {
                id: egui::Id::unique("font_scale"),
                name: copy::TEXT_SIZE,
                scale: 100.0,
                unit: copy::UNIT_PERCENT,
                width: SLIDER_WIDTH,
            };
            slider_field(ui, &theme, &mut prefs.font_scale, limits::FONT_SCALE, &spec);
        });
    });
    section(ui, &theme, copy::SECTION_PERFORMANCE, None, |ui| {
        tristate(
            ui,
            &theme,
            (copy::ANIMATIONS, Some(copy::ANIMATIONS_HELP)),
            &mut prefs.animations,
        );
        controls::row_divider(ui, &theme);
        tristate(ui, &theme, (copy::LOW_POWER, None), &mut prefs.low_power);
        controls::row_divider(ui, &theme);
        let help = format!("{}. {}", env.renderer_line, copy::RESTART_HELP);
        setting_row(ui, &theme, (copy::RENDERER, Some(&help)), |ui| {
            controls::segmented(ui, &theme, &mut prefs.renderer, &RENDERERS);
        });
    });
    section(ui, &theme, copy::SECTION_CUSTOM_THEMES, None, |ui| {
        authoring(ui, cx, env);
    });
}

/// Theme problems (files that failed to load, failed contrast checks) and notes, each in its
/// level's colour.
fn issues(ui: &mut egui::Ui, theme: &crate::theme::UiTheme, issues: &[(IssueLevel, String)]) {
    for (level, issue) in issues {
        // why: an Info note (a retired key, say) is not a problem, so it stays quiet.
        let color = match level {
            IssueLevel::Info => theme.colors.text_muted,
            IssueLevel::Warning | IssueLevel::Error => {
                theme.colors.tone(crate::theme::Tone::Warning).fg
            }
        };
        ui.label(egui::RichText::new(issue).color(color));
    }
}

fn tristate(
    ui: &mut egui::Ui,
    theme: &crate::theme::UiTheme,
    text: (&str, Option<&str>),
    value: &mut Tristate,
) {
    setting_row(ui, theme, text, |ui| {
        controls::segmented(ui, theme, value, &TRISTATE);
    });
}

/// Reloading and copying themes, on as many lines as the page needs.
fn authoring(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, env: &SettingsEnv<'_>) {
    ui.horizontal_wrapped(|ui| {
        if ui.button(copy::RELOAD_THEMES).clicked() {
            cx.push(UiAction::ReloadThemes);
        }
        if ui.button(copy::COPY_THEME_TOML).clicked() {
            cx.push(UiAction::Copy(cx.theme.tokens.file_toml.to_string()));
        }
        if ui.button(copy::COPY_THEMES_FOLDER).clicked() {
            cx.push(UiAction::Copy(env.themes_dir.display().to_string()));
        }
    });
}
