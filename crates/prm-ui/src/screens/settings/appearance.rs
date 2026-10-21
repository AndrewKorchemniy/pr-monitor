//! Appearance: theme (previewed live), text size, animations, low power, renderer,
//! and helpers for writing your own theme.

use eframe::egui;
use prm_config::{
    Config, limits,
    schema::{RendererPref, Tristate},
};
use prm_theme::registry::SYSTEM;

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    screens::settings::SettingsEnv,
    widgets::{
        card::section,
        controls::{self, setting_row},
        slider::{SLIDER_WIDTH, SliderSpec, slider_field},
    },
};

const TRISTATE: [(Tristate, &str); 3] = [
    (Tristate::Auto, "Automatic"),
    (Tristate::On, "On"),
    (Tristate::Off, "Off"),
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

/// Theme files that failed to load or to pass the contrast checks.
fn issues(ui: &mut egui::Ui, theme: &crate::theme::UiTheme, issues: &[String]) {
    let color = theme.colors.tone(crate::theme::Tone::Warning).fg;
    for issue in issues {
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

fn authoring(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, env: &SettingsEnv<'_>) {
    ui.horizontal(|ui| {
        if ui.button(copy::RELOAD_THEMES).clicked() {
            cx.push(UiAction::ReloadThemes);
        }
        if ui.button(copy::COPY_THEME_TOML).clicked() {
            cx.push(UiAction::Copy {
                text: cx.theme.tokens.file_toml.to_string(),
                notice: copy::COPY_THEME_TOML,
            });
        }
        if ui.button(copy::COPY_THEMES_FOLDER).clicked() {
            cx.push(UiAction::Copy {
                text: env.themes_dir.display().to_string(),
                notice: copy::COPY_THEMES_FOLDER,
            });
        }
    });
}
