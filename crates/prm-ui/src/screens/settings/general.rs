//! General: refresh interval, tray behaviour, launch at login, columns, times and proxy.

use eframe::egui;
use prm_config::{
    Config, limits,
    schema::{OrgColumn, ProxyConfig},
};

use crate::{
    copy,
    frame::FrameCx,
    widgets::{self, card::section},
};

const ORG_COLUMN: [(OrgColumn, &str); 3] = [
    (OrgColumn::Auto, "When watching more than one organization"),
    (OrgColumn::Always, "Always"),
    (OrgColumn::Never, "Never"),
];

pub(crate) fn page(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, draft: &mut Config) {
    let theme = cx.theme.clone();
    section(ui, &theme, copy::SECTION_REFRESH, None, |ui| {
        let label = copy::refresh_every(draft.refresh.interval_minutes);
        ui.add(
            egui::Slider::new(
                &mut draft.refresh.interval_minutes,
                limits::INTERVAL_MINUTES,
            )
            .text(label.trim()),
        );
        widgets::muted(ui, &theme, copy::REFRESH_HELP);
    });
    let prefs = &mut draft.ui;
    section(ui, &theme, copy::SECTION_WINDOW, None, |ui| {
        ui.checkbox(&mut prefs.start_minimized, copy::START_MINIMIZED);
        ui.checkbox(&mut prefs.close_to_tray, copy::CLOSE_TO_TRAY);
        ui.checkbox(&mut prefs.minimize_to_tray, copy::MINIMIZE_TO_TRAY);
        ui.checkbox(&mut prefs.launch_at_login, copy::LAUNCH_AT_LOGIN);
        if let Some(error) = &cx.state.autostart.error {
            widgets::error_text(ui, &theme, error);
        }
    });
    section(ui, &theme, copy::SECTION_TABLE, None, |ui| {
        widgets::field_label(ui, &theme, copy::ORG_COLUMN);
        for (value, label) in ORG_COLUMN {
            ui.radio_value(&mut prefs.show_org_column, value, label);
        }
        ui.separator();
        ui.checkbox(&mut prefs.relative_times, copy::RELATIVE_TIMES);
    });
    section(ui, &theme, copy::SECTION_NETWORK, None, |ui| {
        proxy(ui, &theme, &mut draft.network.proxy);
    });
}

fn proxy(ui: &mut egui::Ui, theme: &crate::theme::UiTheme, proxy: &mut ProxyConfig) {
    widgets::field_label(ui, theme, "Proxy");
    let id = ui.make_persistent_id("proxy_text");
    let mut text: String = ui
        .data_mut(|d| d.get_temp(id))
        .unwrap_or_else(|| proxy.to_string());
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .hint_text("system, none, pac or http://proxy:8080")
            .desired_width(320.0),
    );
    if response.changed() {
        if let Ok(parsed) = text.parse() {
            *proxy = parsed;
        }
        ui.data_mut(|d| d.insert_temp(id, text));
    }
    widgets::muted(ui, theme, copy::RESTART_HELP);
}
