//! Appearance: theme (previewed live), density, text size, animations, low power, renderer,
//! and helpers for writing your own theme.

use eframe::egui;
use prm_config::{
    Config, limits,
    schema::{Density, RendererPref, Tristate},
};
use prm_theme::registry::SYSTEM;

use crate::{actions::UiAction, copy, frame::FrameCx, screens::settings::SettingsEnv, widgets};

const TRISTATE: [(Tristate, &str); 3] = [
    (Tristate::Auto, "Automatic"),
    (Tristate::On, "On"),
    (Tristate::Off, "Off"),
];
const RENDERERS: [(RendererPref, &str); 3] = [
    (RendererPref::Auto, "Automatic"),
    (RendererPref::Glow, "OpenGL"),
    (RendererPref::Wgpu, "Direct3D 12 / Vulkan / Metal"),
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
    let space = theme.tokens.metrics.space.lg;
    let prefs = &mut draft.ui;
    widgets::field_label(ui, &theme, copy::THEME);
    let before = prefs.theme.clone();
    let system = copy::match_windows(
        theme_name(env, &prefs.theme_dark),
        theme_name(env, &prefs.theme_light),
    );
    ui.radio_value(&mut prefs.theme, SYSTEM.to_owned(), system);
    for (id, name) in &env.themes {
        ui.radio_value(&mut prefs.theme, id.clone(), name.as_str());
    }
    if prefs.theme != before {
        cx.push(UiAction::PreviewTheme(Some(prefs.theme.clone())));
    }
    ui.add_space(space);
    widgets::field_label(ui, &theme, copy::DENSITY);
    ui.horizontal(|ui| {
        ui.radio_value(&mut prefs.density, Density::Compact, "Compact");
        ui.radio_value(&mut prefs.density, Density::Comfortable, "Comfortable");
    });
    ui.add(egui::Slider::new(&mut prefs.font_scale, limits::FONT_SCALE).text(copy::TEXT_SIZE));
    ui.add_space(space);
    tristate(ui, &theme, copy::ANIMATIONS, &mut prefs.animations);
    widgets::muted(ui, &theme, copy::ANIMATIONS_HELP);
    tristate(ui, &theme, copy::LOW_POWER, &mut prefs.low_power);
    ui.add_space(space);
    widgets::field_label(ui, &theme, copy::RENDERER);
    for (value, label) in RENDERERS {
        ui.radio_value(&mut prefs.renderer, value, label);
    }
    widgets::muted(ui, &theme, env.renderer_line);
    widgets::muted(ui, &theme, copy::RESTART_HELP);
    ui.add_space(space);
    authoring(ui, cx, env);
}

fn tristate(ui: &mut egui::Ui, theme: &crate::theme::UiTheme, label: &str, value: &mut Tristate) {
    widgets::field_label(ui, theme, label);
    ui.horizontal(|ui| {
        for (option, text) in TRISTATE {
            ui.radio_value(value, option, text);
        }
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
