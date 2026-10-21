//! General: refresh interval, tray behaviour, launch at login, columns, times and proxy.

use eframe::egui;
use prm_config::{
    Config, limits,
    schema::{OrgColumn, ProxyConfig},
};

use crate::{
    copy,
    frame::FrameCx,
    theme::UiTheme,
    widgets::{
        self,
        card::section,
        controls::{self, setting_row, toggle_row},
        fields::{self, Field},
        slider::{SLIDER_WIDTH, SliderSpec, slider_field},
    },
};

const ORG_COLUMN: [(OrgColumn, &str); 3] = [
    (OrgColumn::Auto, "Automatic"),
    (OrgColumn::Always, "Always"),
    (OrgColumn::Never, "Never"),
];

pub(crate) fn page(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, draft: &mut Config) {
    let theme = cx.theme.clone();
    section(ui, &theme, copy::SECTION_REFRESH, None, |ui| {
        refresh_row(ui, &theme, &mut draft.refresh.interval_minutes);
    });
    let prefs = &mut draft.ui;
    section(ui, &theme, copy::SECTION_WINDOW, None, |ui| {
        let error = cx.state.autostart.error.as_deref();
        window_rows(ui, &theme, prefs, error);
    });
    section(ui, &theme, copy::SECTION_TABLE, None, |ui| {
        let help = Some(copy::ORG_COLUMN_HELP);
        setting_row(ui, &theme, (copy::ORG_COLUMN, help), |ui| {
            controls::segmented(ui, &theme, &mut prefs.show_org_column, &ORG_COLUMN);
        });
        controls::row_divider(ui, &theme);
        toggle_row(
            ui,
            &theme,
            &mut prefs.relative_times,
            (copy::RELATIVE_TIMES, None),
        );
    });
    section(ui, &theme, copy::SECTION_NETWORK, None, |ui| {
        setting_row(ui, &theme, (copy::PROXY, Some(copy::RESTART_HELP)), |ui| {
            proxy(ui, &theme, &mut draft.network.proxy);
        });
    });
    section(ui, &theme, copy::SECTION_PRIVACY, None, |ui| {
        widgets::muted(ui, &theme, copy::PRIVACY);
    });
}

fn refresh_row(ui: &mut egui::Ui, theme: &UiTheme, interval: &mut u32) {
    let text = (copy::REFRESH_INTERVAL, Some(copy::REFRESH_HELP));
    setting_row(ui, theme, text, |ui| {
        let spec = SliderSpec {
            id: egui::Id::unique("refresh_interval"),
            name: copy::REFRESH_INTERVAL,
            scale: 1.0,
            unit: copy::UNIT_MINUTES,
            width: SLIDER_WIDTH,
        };
        slider_field(ui, theme, interval, limits::INTERVAL_MINUTES, &spec);
    });
}

/// Tray and startup switches, one row each with dividers between.
fn window_rows(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    prefs: &mut prm_config::UiPrefs,
    autostart_error: Option<&str>,
) {
    let rows = [
        (&mut prefs.start_minimized, copy::START_MINIMIZED, None),
        (&mut prefs.close_to_tray, copy::CLOSE_TO_TRAY, None),
        (&mut prefs.minimize_to_tray, copy::MINIMIZE_TO_TRAY, None),
        (
            &mut prefs.launch_at_login,
            copy::LAUNCH_AT_LOGIN,
            autostart_error,
        ),
    ];
    for (index, (value, label, help)) in rows.into_iter().enumerate() {
        if index > 0 {
            controls::row_divider(ui, theme);
        }
        toggle_row(ui, theme, value, (label, help));
    }
}

fn proxy(ui: &mut egui::Ui, theme: &UiTheme, proxy: &mut ProxyConfig) {
    let id = ui.make_persistent_id("proxy_text");
    let mut text: String = ui
        .data_mut(|d| d.get_temp(id))
        .unwrap_or_else(|| proxy.to_string());
    let field = Field::new(egui::Id::unique("proxy_field"), copy::PROXY_HINT).width(280.0);
    let response = fields::text_field(ui, theme, &mut text, &field);
    if response.changed() {
        if let Ok(parsed) = text.parse() {
            *proxy = parsed;
        }
        ui.data_mut(|d| d.insert_temp(id, text));
    }
}
